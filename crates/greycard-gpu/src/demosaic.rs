//! The Bayer demosaics on the GPU: RCD, VNG4, and the dual demosaic of
//! the two, each the same arithmetic as its reference in
//! `greycard_core::develop` (`rcd`, `vng4`, `dual`). Those are ports
//! of Luis Sanz Rodríguez's RCD (release 2.3, GPL-3.0-or-later) and of
//! RawTherapee's `vng4_demosaic_RT.cc` and `dual_demosaic_RT.cc` with
//! the blend mask from `rt_algo.cc` (Ingo Weyrich, GPL-3.0-or-later),
//! VNG4 itself after dcraw's VNG by Dave Coffin; the algorithms and
//! their constants are theirs.
//!
//! The AMaZE dual takes its detail half from AMaZE's own op
//! (`amaze.rs`), whose tiles are its own. Its result stays on the
//! device in a [`Frame`], whole rows in buffers the device's largest
//! takes, and each band of the blend starts from a copy of its rows
//! there, halo and all: the frame's own values, so the band's interior
//! is what the whole frame gives. Only the blend's result comes back.
//! The RCD dual under the automatic threshold keeps RCD's result the
//! same way: the threshold's run reads it back for the search and
//! leaves it on the device, and the blend starts from it instead of
//! running RCD again. The frame is 12 bytes a pixel on the device,
//! 545 MB at 45 MP, beside the bands' 256 MB and their staging.
//!
//! The kernels are a pass each, in `rcd.wgsl`, `vng4.wgsl` and
//! `dual.wgsl` after the bindings and the bilinear they share in
//! `demosaic.wgsl`. The mosaic goes up in full floats and the result
//! comes back interleaved in full floats, as the lens correction's
//! picture does: the rest of the develop is the CPU's.
//!
//! The frame is worked in bands of whole rows, each with a halo of
//! [`HALO`] rows above and below that is computed and thrown away.
//! Every value of a band's interior reads the mosaic no more than 19
//! rows away (RCD's result reads 11, and the blend mask's contrast
//! and blur another 8), and what a band's own edges do differently
//! from the frame's (RCD's bilinear border, the mask copied out to
//! its edge, the blur's clamp) reaches no further than 23 rows in:
//! the halo covers both, so the bands are the frame's to the bit. A
//! band starts on an even row, so its CFA phases are the frame's. The
//! band's planes are storage buffers, eight floats a site in all, its
//! height chosen to keep them to [`BAND_BYTES`] and the result's
//! buffer within the device's binding limit, and they are made for
//! the run and dropped at its end. Two staging buffers a band's
//! interior, made once a run, take its result back in turn, so a band
//! runs while the one before it is unpacked.
//!
//! The thresholds are the CPU's code. The noise model's table by
//! lightness or the fixed one is handed to the mask as a table of 101,
//! the reference's to the bit. The automatic one searches the detail
//! result for its flattest tile, and that result is the GPU's: RCD's
//! for the RCD dual, AMaZE's for the AMaZE dual, each read back for
//! the search alone. A few ULP there can move the tile the
//! search settles on, so on a real card the automatic threshold can be
//! a hundredth (one of its steps) from the reference's; a test bounds
//! it to that. The largest sample, where both demosaics clamp, is the
//! CPU's too.
//!
//! Where the GPU can differ. Every kernel takes the reference's
//! operations in the reference's order, so what is left is the
//! compiler's freedom to fuse a multiply and an add, and WGSL's
//! division and square root, which need not round as the CPU's do
//! (2.5 ULP for a division). RCD is all ratios and weighted means of
//! them, so it differs by a few ULP of its values. VNG4's gradient
//! sums are exact on both (each term an absolute difference times a
//! power of two, added in the same order), so the directions it
//! chooses are the reference's; its green divides by the number of
//! them, which can round apart. The mask's cube root is the power's
//! with a Newton step, within an ULP or two of the CPU's `cbrt`, and a
//! lightness that sits on a half unit of L* can round to the
//! neighboring entry of the threshold table, a step in the mask the
//! size of the table's step there. The tests bound each.

use greycard_core::develop::dual::{
    DualContrast, MASK_BLUR_SIGMA, Threshold, blur_kernel, threshold_for, threshold_reads_detail,
};
use greycard_core::develop::{rcd, vng4};
use greycard_core::raw::CfaPattern;
use rayon::prelude::*;

use crate::plumbing::{STORAGE_BUFFER, STORAGE_BUFFER_READ, groups, layout_at, pipeline, uniform};
use crate::{Context, Error, Result};

/// Rows computed above and below each band's own and thrown away:
/// past the 23 a band's edges reach (see the module doc), and even.
const HALO: usize = 32;

/// The most a band's working buffers take on the device.
const BAND_BYTES: u64 = 256 << 20;

/// Floats a site across the band's buffers: the mosaic, four planes,
/// and the result's three.
const FLOATS_A_SITE: u64 = 8;

/// The smallest side worked here; a smaller frame is the CPU's.
const MIN_SIDE: usize = 16;

/// The shader's uniform, laid out as WGSL lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    size: [u32; 2],
    mode: u32,
    ceiling: f32,
    colors: [u32; 4],
    fc4: [u32; 4],
    term_start: [u32; 4],
    term_count: [u32; 4],
}

const _: () = assert!(std::mem::size_of::<Params>() == 80);

/// A VNG4 gradient term as the shader reads it: offsets `(dy, dx)`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Term {
    d1: [i32; 2],
    d2: [i32; 2],
    weight: f32,
    grads: u32,
    pad: [u32; 2],
}

const _: () = assert!(std::mem::size_of::<Term>() == 32);

/// Taps of the mask's blur, the center first.
const KERNEL_TAPS: usize = 7;

/// What a run makes.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Rcd,
    Vng4,
    Dual,
}

impl Mode {
    fn code(self) -> u32 {
        match self {
            Mode::Rcd => 0,
            Mode::Vng4 => 1,
            Mode::Dual => 2,
        }
    }
}

/// The op's pipelines, built once per device.
pub(crate) struct Pipelines {
    layout: wgpu::BindGroupLayout,
    rcd: [wgpu::ComputePipeline; 6],
    mask: [wgpu::ComputePipeline; 4],
    vng4: [wgpu::ComputePipeline; 3],
}

/// The module: the shared bindings, then each algorithm's kernels.
fn source() -> String {
    [
        include_str!("demosaic.wgsl"),
        include_str!("rcd.wgsl"),
        include_str!("vng4.wgsl"),
        include_str!("dual.wgsl"),
    ]
    .join("\n")
}

impl Pipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("demosaic"),
            source: wgpu::ShaderSource::Wgsl(source().into()),
        });
        let layout = layout_at(
            device,
            "demosaic",
            &[
                (0, uniform(false)),
                (1, STORAGE_BUFFER_READ),
                (2, STORAGE_BUFFER),
                (3, STORAGE_BUFFER),
                (4, STORAGE_BUFFER),
                (5, STORAGE_BUFFER),
                (6, STORAGE_BUFFER),
                (7, STORAGE_BUFFER_READ),
                (8, STORAGE_BUFFER_READ),
            ],
        );
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("demosaic"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let make = |entry: &str| pipeline(device, &module, &pl, entry);
        Self {
            rcd: [
                "rcd_vh",
                "rcd_lpf",
                "rcd_green",
                "rcd_pq",
                "rcd_other",
                "rcd_rgb",
            ]
            .map(make),
            mask: [
                "dual_lightness",
                "dual_mask",
                "dual_blur_rows",
                "dual_blur_columns",
            ]
            .map(make),
            vng4: ["vng_planes", "vng_green", "vng_rgb"].map(make),
            layout,
        }
    }
}

impl Context {
    /// Bands of at most `rows` rows, their halo with them (none: the
    /// device's own size), for a test of the bands on a frame small
    /// enough to check against the reference. Fewer than the halo
    /// needs is the device's own size.
    #[doc(hidden)]
    pub fn limit_demosaic_bands(&self, rows: Option<usize>) {
        self.demosaic_rows
            .store(rows.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
    }

    /// Buffers of at most `rows` rows for a detail result kept on the
    /// device (none: as many as the device's largest buffer takes), for
    /// a test of the copies across them on a small frame.
    #[doc(hidden)]
    pub fn limit_demosaic_frame_buffers(&self, rows: Option<usize>) {
        self.demosaic_frame_rows
            .store(rows.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
    }

    /// A frame's RGB to keep on the device, in buffers as
    /// [`Context::limit_demosaic_frame_buffers`] has them.
    fn frame(&self, width: usize, height: usize) -> Frame {
        let most = match self
            .demosaic_frame_rows
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => usize::MAX,
            r => r,
        };
        Frame::new(&self.device, width, height, most)
    }

    /// [`greycard_core::develop::rcd::demosaic_rcd`] on the GPU: the
    /// same contract, interleaved RGB back. A frame under 16 a side,
    /// or one whose bands the device cannot hold, is
    /// [`Error::Unsupported`], for the caller to run the reference.
    pub fn demosaic_rcd(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
    ) -> Result<Vec<f32>> {
        self.demosaic_bands(samples, width, height, pattern, Mode::Rcd, None)
    }

    /// [`greycard_core::develop::vng4::demosaic_vng4`] on the GPU, as
    /// [`Context::demosaic_rcd`] is RCD.
    pub fn demosaic_vng4(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
    ) -> Result<Vec<f32>> {
        self.demosaic_bands(samples, width, height, pattern, Mode::Vng4, None)
    }

    /// [`greycard_core::develop::dual::demosaic_dual`] with RCD as its
    /// detail half, on the GPU: RCD where there is detail, VNG4 where
    /// there is only noise, by `contrast` as the reference takes it.
    /// The threshold is the reference's, made on the CPU (see the
    /// module doc).
    pub fn demosaic_rcd_dual(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
        contrast: DualContrast,
    ) -> Result<Vec<f32>> {
        if !threshold_reads_detail(contrast) {
            let (threshold, _) = threshold_for(contrast, &[], width, height);
            if threshold.is_zero() {
                return self.demosaic_rcd(samples, width, height, pattern);
            }
            return self.demosaic_bands(
                samples,
                width,
                height,
                pattern,
                Mode::Dual,
                Some(&threshold),
            );
        }
        // The search reads RCD's result on the host; the blend starts
        // from the same result kept on the device, so RCD runs once.
        let rows = self.checked_rows(samples, width, height, pattern)?;
        self.scoped("demosaicing", || {
            let frame = self.frame(width, height);
            let detail = self.demosaic_bands_unscoped(
                &Bands {
                    samples,
                    width,
                    height,
                    pattern,
                    mode: Mode::Rcd,
                    threshold: None,
                    start: None,
                    sink: Some(&frame),
                    rows,
                },
                None,
            )?;
            let (threshold, _) = threshold_for(contrast, &detail, width, height);
            if threshold.is_zero() {
                return Ok(detail);
            }
            self.demosaic_bands_unscoped(
                &Bands {
                    samples,
                    width,
                    height,
                    pattern,
                    mode: Mode::Dual,
                    threshold: Some(&threshold),
                    start: Some(&frame),
                    sink: None,
                    rows,
                },
                Some(detail),
            )
        })
    }

    /// [`greycard_core::develop::dual::demosaic_dual`] with AMaZE as its
    /// detail half, on the GPU: AMaZE ([`Context::demosaic_amaze`]),
    /// its result kept on the device, then the mask and VNG4 blended
    /// into it band by band, each band starting from that result. The
    /// threshold is the reference's, read off AMaZE's result where it
    /// searches, which alone brings that result back to the host.
    pub fn demosaic_amaze_dual(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
        contrast: DualContrast,
    ) -> Result<Vec<f32>> {
        let reads = threshold_reads_detail(contrast);
        let fixed = (!reads).then(|| threshold_for(contrast, &[], width, height).0);
        if fixed.as_ref().is_some_and(Threshold::is_zero) {
            return self.demosaic_amaze(samples, width, height, pattern);
        }
        let rows = self.checked_rows(samples, width, height, pattern)?;
        self.scoped("demosaicing", || {
            let frame = self.frame(width, height);
            let detail = self.amaze_into_frame(samples, width, height, pattern, &frame, reads)?;
            let threshold = match fixed {
                Some(t) => t,
                None => {
                    let detail = detail.as_deref().expect("read back for the search");
                    threshold_for(contrast, detail, width, height).0
                }
            };
            if threshold.is_zero() {
                return Ok(detail.expect("only the search finds no threshold"));
            }
            self.demosaic_bands_unscoped(
                &Bands {
                    samples,
                    width,
                    height,
                    pattern,
                    mode: Mode::Dual,
                    threshold: Some(&threshold),
                    start: Some(&frame),
                    sink: None,
                    rows,
                },
                detail,
            )
        })
    }

    /// The rows a band holds, its halo with it: as many as keep its
    /// buffers to [`BAND_BYTES`] and its result within one binding,
    /// even, and the frame's height when that is fewer.
    fn band_rows(&self, width: usize, height: usize) -> Result<usize> {
        let limits = self.device.limits();
        let binding = limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size);
        let w = width as u64;
        let most = match self
            .demosaic_rows
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            r if r >= 2 * HALO + 2 => r as u64,
            _ => u64::MAX,
        };
        let rows = (binding / (w * 12))
            .min(BAND_BYTES / (w * 4 * FLOATS_A_SITE))
            .min(most)
            .min(height as u64) as usize;
        let rows = if rows == height { rows } else { rows & !1 };
        if rows < height && rows < 2 * HALO + 2 {
            return Err(Error::Unsupported(format!(
                "a {width}x{height} mosaic: {rows} rows a band is too few for the halo"
            )));
        }
        Ok(rows)
    }

    /// What the bands take, checked, and the rows a band holds.
    fn checked_rows(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
    ) -> Result<usize> {
        if !rcd::is_bayer(pattern) {
            return Err(Error::Unsupported(format!(
                "the demosaic on the GPU needs a 2x2 Bayer pattern, got {pattern}"
            )));
        }
        if samples.len() != width * height {
            return Err(Error::Unsupported(format!(
                "{width}x{height} mosaic with {} samples",
                samples.len()
            )));
        }
        if width < MIN_SIDE || height < MIN_SIDE {
            return Err(Error::Unsupported(format!(
                "a {width}x{height} mosaic is the CPU's"
            )));
        }
        self.band_rows(width, height)
    }

    #[allow(clippy::too_many_arguments)]
    fn demosaic_bands(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
        mode: Mode,
        threshold: Option<&Threshold>,
    ) -> Result<Vec<f32>> {
        let rows = self.checked_rows(samples, width, height, pattern)?;
        self.scoped("demosaicing", || {
            self.demosaic_bands_unscoped(
                &Bands {
                    samples,
                    width,
                    height,
                    pattern,
                    mode,
                    threshold,
                    start: None,
                    sink: None,
                    rows,
                },
                None,
            )
        })
    }

    /// The bands of one run, read back into `out` (a fresh frame when
    /// none is given; the detail result's own buffer otherwise, which
    /// the blend overwrites row by row, since every band's start comes
    /// from the device).
    fn demosaic_bands_unscoped(&self, run: &Bands, out: Option<Vec<f32>>) -> Result<Vec<f32>> {
        let &Bands {
            samples,
            width,
            height,
            pattern,
            mode,
            threshold,
            start,
            sink,
            rows,
        } = run;
        let p = self.demosaic_pipelines()?;
        let (fc4, phase_terms) =
            vng4::gradient_terms(pattern).map_err(|e| Error::Unsupported(e.to_string()))?;
        let rgb_index = |y: usize, x: usize| {
            pattern
                .color_at(y, x)
                .rgb_index()
                .expect("a Bayer pattern is RGB") as u32
        };
        let mut terms: Vec<Term> = Vec::new();
        let mut term_start = [0u32; 4];
        let mut term_count = [0u32; 4];
        for (phase, (start, count)) in term_start.iter_mut().zip(&mut term_count).enumerate() {
            *start = terms.len() as u32;
            for t in &phase_terms[phase >> 1][phase & 1] {
                terms.push(Term {
                    d1: [t.d1.0, t.d1.1],
                    d2: [t.d2.0, t.d2.1],
                    weight: t.weight,
                    grads: u32::from(t.grads),
                    pad: [0; 2],
                });
            }
            *count = terms.len() as u32 - *start;
        }
        let kernel = blur_kernel(MASK_BLUR_SIGMA);
        debug_assert_eq!(kernel.len(), KERNEL_TAPS);
        let table = threshold.map_or([0.0; 101], Threshold::table);
        let consts: Vec<f32> = kernel.iter().chain(&table).copied().collect();
        // Where both demosaics clamp: the largest sample, and at least
        // one, as the references take it.
        let ceiling = samples
            .par_iter()
            .copied()
            .fold(|| 1.0f32, f32::max)
            .reduce(|| 1.0f32, f32::max);

        let n = (width * rows) as u64;
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(4),
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        let params = buffer(
            "demosaic params",
            std::mem::size_of::<Params>() as u64,
            U::UNIFORM | U::COPY_DST,
        );
        let mosaic = buffer("demosaic mosaic", n * 4, U::STORAGE | U::COPY_DST);
        let planes = ["demosaic a", "demosaic b", "demosaic c", "demosaic d"]
            .map(|label| buffer(label, n * 4, U::STORAGE));
        let rgb = buffer(
            "demosaic rgb",
            n * 12,
            U::STORAGE | U::COPY_SRC | U::COPY_DST,
        );
        let consts_buf = buffer(
            "demosaic consts",
            (consts.len() * 4) as u64,
            U::STORAGE | U::COPY_DST,
        );
        self.queue
            .write_buffer(&consts_buf, 0, bytemuck::cast_slice(&consts));
        let terms_buf = buffer(
            "demosaic terms",
            (terms.len() * std::mem::size_of::<Term>()) as u64,
            U::STORAGE | U::COPY_DST,
        );
        self.queue
            .write_buffer(&terms_buf, 0, bytemuck::cast_slice(&terms));
        fn whole(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            }
        }
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("demosaic"),
            layout: &p.layout,
            entries: &[
                whole(0, &params),
                whole(1, &mosaic),
                whole(2, &planes[0]),
                whole(3, &planes[1]),
                whole(4, &planes[2]),
                whole(5, &planes[3]),
                whole(6, &rgb),
                whole(7, &consts_buf),
                whole(8, &terms_buf),
            ],
        });

        let row = width * 3;
        let row_bytes = (row * 4) as u64;
        let step = if rows == height {
            height
        } else {
            rows - 2 * HALO
        };
        let bands: Vec<(usize, usize)> = (0..height)
            .step_by(step)
            .map(|y0| (y0, (y0 + step).min(height)))
            .collect();
        // Two staging buffers for a band's interior, made once for the
        // run, so a band runs while the one before it is unpacked.
        let staging: [wgpu::Buffer; 2] = std::array::from_fn(|_| {
            buffer(
                "demosaic read back",
                step as u64 * row_bytes,
                U::COPY_DST | U::MAP_READ,
            )
        });
        let mut up = 0u64;
        let mut submit = |k: usize| {
            let (y0, y1) = bands[k];
            let (top, bottom) = (y0.saturating_sub(HALO), (y1 + HALO).min(height));
            let band = bottom - top;
            self.queue.write_buffer(
                &mosaic,
                0,
                bytemuck::cast_slice(&samples[top * width..bottom * width]),
            );
            up += (band * width * 4) as u64;
            self.queue.write_buffer(
                &params,
                0,
                bytemuck::bytes_of(&Params {
                    size: [width as u32, band as u32],
                    mode: mode.code(),
                    ceiling,
                    colors: [
                        rgb_index(0, 0),
                        rgb_index(0, 1),
                        rgb_index(1, 0),
                        rgb_index(1, 1),
                    ],
                    fc4: [fc4[0][0], fc4[0][1], fc4[1][0], fc4[1][1]].map(|c| c as u32),
                    term_start,
                    term_count,
                }),
            );
            let mut encoder = self.encoder("demosaic");
            // A detail result kept on the device (AMaZE's, or RCD's
            // from the threshold's run) is the band's start in place
            // of RCD's, its halo rows with it.
            if let Some(start) = start {
                start.copy_out(&mut encoder, top, bottom, &rgb, 0);
            }
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_bind_group(0, &bind, &[]);
                let (gx, gy) = (groups(width as u32, 16), groups(band as u32, 16));
                let mut run = |kernels: &[wgpu::ComputePipeline]| {
                    for k in kernels {
                        pass.set_pipeline(k);
                        pass.dispatch_workgroups(gx, gy, 1);
                    }
                };
                if mode != Mode::Vng4 && start.is_none() {
                    run(&p.rcd);
                }
                if mode == Mode::Dual {
                    run(&p.mask);
                }
                if mode != Mode::Rcd {
                    run(&p.vng4);
                }
            }
            let (offset, bytes) = ((y0 - top) as u64 * row_bytes, (y1 - y0) as u64 * row_bytes);
            if let Some(sink) = sink {
                sink.copy_in(&mut encoder, &rgb, offset, y0, y1);
            }
            encoder.copy_buffer_to_buffer(&rgb, offset, &staging[k % 2], 0, bytes);
            self.queue.submit(Some(encoder.finish()));
            let (tx, rx) = std::sync::mpsc::channel();
            staging[k % 2]
                .slice(..bytes)
                .map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
            rx
        };

        let mut out = out.unwrap_or_else(|| vec![0.0f32; width * height * 3]);
        assert_eq!(out.len(), width * height * 3);
        let mut pending = submit(0);
        for k in 0..bands.len() {
            self.wait()?;
            pending
                .recv()
                .map_err(|_| Error::ReadBack("the map callback never came".into()))?
                .map_err(|e| Error::ReadBack(e.to_string()))?;
            let next = (k + 1 < bands.len()).then(|| submit(k + 1));
            let (y0, y1) = bands[k];
            let staged = &staging[k % 2];
            let bytes = (y1 - y0) as u64 * row_bytes;
            let data = staged
                .slice(..bytes)
                .get_mapped_range()
                .map_err(|e| Error::ReadBack(e.to_string()))?;
            out[y0 * row..y1 * row].copy_from_slice(bytemuck::cast_slice(&data));
            drop(data);
            staged.unmap();
            if let Some(rx) = next {
                pending = rx;
            }
        }
        drop(submit);
        self.count_demosaic_traffic(false, up);
        self.count_demosaic_traffic(true, (out.len() * 4) as u64);
        Ok(out)
    }
}

/// One run of the bands.
struct Bands<'a> {
    samples: &'a [f32],
    width: usize,
    height: usize,
    pattern: &'a CfaPattern,
    mode: Mode,
    threshold: Option<&'a Threshold>,
    /// A detail result on the device each band starts from, in place
    /// of RCD's own.
    start: Option<&'a Frame>,
    /// Where the bands' results go on the device as well, for a run
    /// after this one to start from.
    sink: Option<&'a Frame>,
    rows: usize,
}

/// A frame's interleaved RGB kept on the device between runs: a
/// detail result for the dual's blend to start from. In buffers of
/// whole rows, as many as the device's largest buffer takes; it is
/// copied to and from, never bound.
pub(crate) struct Frame {
    chunks: Vec<wgpu::Buffer>,
    chunk_rows: usize,
    row_bytes: u64,
}

impl Frame {
    /// At most `most` rows a buffer.
    pub(crate) fn new(device: &wgpu::Device, width: usize, height: usize, most: usize) -> Self {
        let row_bytes = width as u64 * 12;
        let chunk_rows = ((device.limits().max_buffer_size / row_bytes) as usize)
            .min(most)
            .clamp(1, height.max(1));
        let chunks = (0..height.div_ceil(chunk_rows))
            .map(|k| {
                let rows = chunk_rows.min(height - k * chunk_rows);
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("demosaic detail"),
                    size: rows as u64 * row_bytes,
                    usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Self {
            chunks,
            chunk_rows,
            row_bytes,
        }
    }

    /// Its bytes on the device.
    pub(crate) fn bytes(&self) -> u64 {
        self.chunks.iter().map(wgpu::Buffer::size).sum()
    }

    /// Rows `y0..y1` as runs within its buffers: the buffer, the
    /// offset in it, the offset from row `y0`, and the bytes.
    fn runs(
        &self,
        y0: usize,
        y1: usize,
    ) -> impl Iterator<Item = (&wgpu::Buffer, u64, u64, u64)> + '_ {
        let r = self.chunk_rows;
        (y0 / r..y1.div_ceil(r)).map(move |k| {
            let (lo, hi) = (y0.max(k * r), y1.min((k + 1) * r));
            (
                &self.chunks[k],
                (lo - k * r) as u64 * self.row_bytes,
                (lo - y0) as u64 * self.row_bytes,
                (hi - lo) as u64 * self.row_bytes,
            )
        })
    }

    /// Rows `y0..y1` in from `src` at `offset`.
    pub(crate) fn copy_in(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Buffer,
        offset: u64,
        y0: usize,
        y1: usize,
    ) {
        for (chunk, at, from, bytes) in self.runs(y0, y1) {
            encoder.copy_buffer_to_buffer(src, offset + from, chunk, at, bytes);
        }
    }

    /// Rows `y0..y1` out to `dst` at `offset`.
    pub(crate) fn copy_out(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        y0: usize,
        y1: usize,
        dst: &wgpu::Buffer,
        offset: u64,
    ) {
        for (chunk, at, from, bytes) in self.runs(y0, y1) {
            encoder.copy_buffer_to_buffer(chunk, at, dst, offset + from, bytes);
        }
    }

    /// The whole frame up from the host.
    pub(crate) fn upload(&self, queue: &wgpu::Queue, rgb: &[f32]) {
        let floats = self.chunk_rows * self.row_bytes as usize / 4;
        for (chunk, part) in self.chunks.iter().zip(rgb.chunks(floats)) {
            queue.write_buffer(chunk, 0, bytemuck::cast_slice(part));
            queue.submit([]);
        }
    }
}

#[cfg(test)]
mod tests {
    /// The module parses and validates without a device.
    #[test]
    fn the_shader_parses_and_validates() {
        let source = super::source();
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("demosaic:\n{}", e.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("demosaic: {e:?}"));
    }

    /// The uniform and the term sit where the WGSL puts them.
    #[test]
    fn the_params_are_laid_out_as_the_shader_reads_them() {
        use super::{Params, Term};
        use std::mem::offset_of;
        assert_eq!(offset_of!(Params, size), 0);
        assert_eq!(offset_of!(Params, mode), 8);
        assert_eq!(offset_of!(Params, ceiling), 12);
        assert_eq!(offset_of!(Params, colors), 16);
        assert_eq!(offset_of!(Params, fc4), 32);
        assert_eq!(offset_of!(Params, term_start), 48);
        assert_eq!(offset_of!(Params, term_count), 64);
        assert_eq!(offset_of!(Term, d2), 8);
        assert_eq!(offset_of!(Term, weight), 16);
        assert_eq!(offset_of!(Term, grads), 20);
    }

    /// The blur's taps are what the shader loops over.
    #[test]
    fn the_mask_s_blur_has_the_shader_s_taps() {
        let kernel = super::blur_kernel(super::MASK_BLUR_SIGMA);
        assert_eq!(kernel.len(), super::KERNEL_TAPS);
    }
}
