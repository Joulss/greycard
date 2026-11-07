//! AMaZE on the GPU: `greycard_core::develop::amaze::demosaic_amaze`
//! with the same tiles, the same passes, and the same order of
//! operations, in `amaze.wgsl`. The reference is our port of
//! RawTherapee's `rtengine/amaze_demosaic_RT.cc` (GPL-3.0-or-later,
//! copyright (c) 2008-2010 Emil Martinec, optimized for speed by Ingo
//! Weyrich, incorporating ideas of Luis Sanz Rodríguez and Paul Lee);
//! the algorithm and its constants are theirs.
//!
//! The reference's output depends on its tiles (a pass reads as zero
//! the cells it never wrote near a tile's edge, and the Nyquist box is
//! a tile's own), so we keep them: every plane is a 160x160 block a
//! tile, a row of tiles at a time (more when the picture is narrow),
//! and the batch's interior comes back as a band of whole picture rows.
//! The planes are spread over four storage bindings so none passes the
//! default 128 MB a binding: 2.8 MB a tile in all, 130 MB a row of
//! tiles at 6000 pixels across. The ceiling, the outer ring of three
//! pixels (bilinear, as the reference's), and frames too small for a
//! tile are the reference's own functions on the CPU.
//!
//! Every buffer is made for a call and reused across its batches, as
//! the other ops here do: about 400 MB on the device at 45 MP. A
//! device that cannot make them reports it in the error scope, which
//! comes back as [`Error::Gpu`] for the caller to fall back on the CPU.

use greycard_core::develop::amaze::{MARGIN, TS, ceiling, fill_border};
use greycard_core::develop::demosaic_bilinear;
use greycard_core::develop::rcd::is_bayer;
use greycard_core::raw::CfaPattern;

use crate::plumbing::{SAMPLED, STORAGE_BUFFER, Uniforms, groups, layout, pipeline, uniform};
use crate::{Context, Error, Result};

/// The shader's uniform, laid out as WGSL lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    size: [u32; 2],
    tiles_across: u32,
    first_tile: u32,
    count: u32,
    band_y0: u32,
    ceiling: f32,
    pad: u32,
    colors: [u32; 4],
}

const _: () = assert!(std::mem::size_of::<Params>() == 48);

/// The planes the shader keeps a tile, and how many share a binding.
const PLANES: u64 = 27;
const PER_BINDING: u64 = 7;
const BINDINGS: u64 = PLANES.div_ceil(PER_BINDING);
/// Floats a tile's plane.
const CELLS: u64 = (TS * TS) as u64;
/// The tile interior's side, the step between tiles.
const STEP: usize = TS - 2 * MARGIN;

/// The kernels in the order they run, and the shape of their dispatch.
const KERNELS: [(&str, Shape); 18] = [
    ("init", Shape::Cells),
    ("gradients", Shape::Cells),
    ("differences", Shape::Cells),
    ("choose", Shape::Lines),
    ("disagreement", Shape::Cells),
    ("weights", Shape::Cells),
    ("nyquist_test", Shape::Cells),
    ("nyquist_majority", Shape::Cells),
    ("nyquist_area", Shape::Cells),
    ("green_at_chroma", Shape::Tiles),
    ("curvature", Shape::Cells),
    ("refine", Shape::Cells),
    ("diagonals", Shape::Cells),
    ("chroma_diagonal", Shape::Cells),
    ("diagonal_weight", Shape::Tiles),
    ("green_again", Shape::Cells),
    ("other_chroma", Shape::Cells),
    ("output", Shape::Cells),
];

#[derive(Clone, Copy)]
enum Shape {
    /// A thread a cell, 16x16 groups, z the tile.
    Cells,
    /// A thread a row (group x 0) or a column (group x 1), y the tile.
    Lines,
    /// A workgroup a tile.
    Tiles,
}

/// The op's pipelines, built once per device.
pub(crate) struct Pipelines {
    layout: wgpu::BindGroupLayout,
    kernels: Vec<(wgpu::ComputePipeline, Shape)>,
}

impl Pipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amaze"),
            source: wgpu::ShaderSource::Wgsl(include_str!("amaze.wgsl").into()),
        });
        let layout = layout(
            device,
            "amaze",
            &[
                uniform(true),
                SAMPLED,
                STORAGE_BUFFER,
                STORAGE_BUFFER,
                STORAGE_BUFFER,
                STORAGE_BUFFER,
                STORAGE_BUFFER,
                STORAGE_BUFFER,
            ],
        );
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("amaze"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let kernels = KERNELS
            .iter()
            .map(|&(entry, shape)| (pipeline(device, &module, &pl, entry), shape))
            .collect();
        Self { layout, kernels }
    }
}

/// The bytes of planes a binding may hold in a batch, about: one row
/// of tiles always, more while they fit.
const BATCH_BINDING_BYTES: u64 = 48 << 20;

impl Context {
    /// [`greycard_core::develop::amaze::demosaic_amaze`] on the GPU:
    /// the same signature and contract, the mosaic uploaded, demosaiced
    /// and read back, RGB interleaved. A pattern that is not 2x2 Bayer
    /// or a picture larger than the device takes is
    /// [`Error::Unsupported`]; a device error is [`Error::Gpu`].
    pub fn demosaic_amaze(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
    ) -> Result<Vec<f32>> {
        self.demosaic_amaze_in_batches(samples, width, height, pattern, None)
    }

    /// [`Context::demosaic_amaze`] a given number of tile rows a batch:
    /// for the tests, which check that the batches meet without a seam.
    #[doc(hidden)]
    pub fn demosaic_amaze_in_batches(
        &self,
        samples: &[f32],
        width: usize,
        height: usize,
        pattern: &CfaPattern,
        tile_rows: Option<usize>,
    ) -> Result<Vec<f32>> {
        if !is_bayer(pattern) {
            return Err(Error::Unsupported(format!(
                "AMaZE needs a 2x2 Bayer pattern, got {pattern}"
            )));
        }
        assert_eq!(samples.len(), width * height, "one sample a pixel");
        if width < 2 * MARGIN + 2 || height < 2 * MARGIN + 2 {
            return Ok(demosaic_bilinear(samples, width, height, pattern));
        }
        let (w, h) = self.fits(width, height)?;
        let ceiling = ceiling(samples);
        let color = |r, c| {
            pattern
                .color_at(r, c)
                .rgb_index()
                .expect("a Bayer pattern is RGB") as u32
        };
        let params = Params {
            size: [w, h],
            tiles_across: width.div_ceil(STEP) as u32,
            ceiling,
            colors: [color(0, 0), color(0, 1), color(1, 0), color(1, 1)],
            ..Default::default()
        };
        let mut out = self.scoped("demosaicing with AMaZE", || {
            self.amaze_unscoped(samples, params, tile_rows)
        })?;
        fill_border(&mut out, samples, width, height, pattern);
        Ok(out)
    }

    fn amaze_unscoped(
        &self,
        samples: &[f32],
        params: Params,
        tile_rows: Option<usize>,
    ) -> Result<Vec<f32>> {
        let pipelines = self.amaze_pipelines()?;
        let [w, h] = params.size;
        let (width, height) = (w as usize, h as usize);
        let across = params.tiles_across as u64;
        let down = height.div_ceil(STEP);
        let tile_bytes = PER_BINDING * CELLS * 4;
        let limits = self.device.limits();
        let binding_max = limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size);
        if across * tile_bytes > binding_max {
            return Err(Error::Unsupported(format!(
                "a {width}-pixel-wide mosaic; a row of AMaZE tiles takes {} MB a binding and \
                 this device binds up to {} MB",
                (across * tile_bytes) >> 20,
                binding_max >> 20
            )));
        }
        if let Some(rows) = tile_rows
            && rows as u64 * across * tile_bytes > binding_max
        {
            return Err(Error::Unsupported(format!(
                "{rows} rows of AMaZE tiles a batch, past this device's {} MB a binding",
                binding_max >> 20
            )));
        }
        let per_batch = tile_rows
            .unwrap_or((BATCH_BINDING_BYTES.min(binding_max) / (across * tile_bytes)) as usize)
            .clamp(1, down);
        let batches = down.div_ceil(per_batch);
        let max_tiles = across * per_batch as u64;

        let mosaic = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("amaze mosaic"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.upload_mosaic(&mosaic, samples, w, h);
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        // Each binding sized for the planes it holds: the last has six.
        let planes: Vec<wgpu::Buffer> = (0..BINDINGS)
            .map(|k| {
                let held = (PLANES - k * PER_BINDING).min(PER_BINDING);
                buffer(
                    "amaze planes",
                    max_tiles * held * CELLS * 4,
                    wgpu::BufferUsages::STORAGE,
                )
            })
            .collect();
        let boxes = buffer(
            "amaze nyquist boxes",
            max_tiles * 16,
            wgpu::BufferUsages::STORAGE,
        );
        let band_rows = (per_batch * STEP).min(height) as u64;
        let band_bytes = band_rows * u64::from(w) * 12;
        let band = buffer(
            "amaze band",
            band_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        // Two staging buffers, so a batch runs while the one before it
        // is unpacked.
        let staging: [wgpu::Buffer; 2] = std::array::from_fn(|_| {
            buffer(
                "amaze read back",
                band_bytes,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            )
        });
        let uniforms = Uniforms::new(&self.device, "amaze params", batches as u64);
        let view = mosaic.create_view(&Default::default());
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ];
        for (k, plane) in planes.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: 2 + k as u32,
                resource: plane.as_entire_binding(),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: boxes.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 7,
            resource: band.as_entire_binding(),
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("amaze"),
            layout: &pipelines.layout,
            entries: &entries,
        });

        let rows_of = |b: usize| {
            let y0 = b * per_batch * STEP;
            (y0, (per_batch * STEP).min(height - y0))
        };
        let submit = |b: usize| {
            let tiles = (per_batch.min(down - b * per_batch) as u64 * across) as u32;
            let (y0, rows) = rows_of(b);
            let slot = uniforms.push(
                &self.queue,
                &Params {
                    first_tile: (b * per_batch) as u32 * params.tiles_across,
                    count: tiles,
                    band_y0: y0 as u32,
                    ..params
                },
            );
            let mut encoder = self.encoder("amaze");
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_bind_group(0, &bind, &[slot]);
                let cells = groups(TS as u32, 16);
                for (kernel, shape) in &pipelines.kernels {
                    pass.set_pipeline(kernel);
                    match shape {
                        Shape::Cells => pass.dispatch_workgroups(cells, cells, tiles),
                        Shape::Lines => pass.dispatch_workgroups(2, tiles, 1),
                        Shape::Tiles => pass.dispatch_workgroups(tiles, 1, 1),
                    }
                }
            }
            let bytes = rows as u64 * u64::from(w) * 12;
            encoder.copy_buffer_to_buffer(&band, 0, &staging[b % 2], 0, bytes);
            self.queue.submit(Some(encoder.finish()));
            let (tx, rx) = std::sync::mpsc::channel();
            staging[b % 2]
                .slice(..bytes)
                .map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
            rx
        };
        let mut out = vec![0f32; width * height * 3];
        let mut pending = submit(0);
        for b in 0..batches {
            self.wait()?;
            pending
                .recv()
                .map_err(|_| Error::ReadBack("the map callback never came".into()))?
                .map_err(|e| Error::ReadBack(e.to_string()))?;
            let next = (b + 1 < batches).then(|| submit(b + 1));
            let (y0, rows) = rows_of(b);
            let bytes = rows as u64 * u64::from(w) * 12;
            let buffer = &staging[b % 2];
            let data = buffer
                .slice(..bytes)
                .get_mapped_range()
                .map_err(|e| Error::ReadBack(e.to_string()))?;
            out[y0 * width * 3..(y0 + rows) * width * 3]
                .copy_from_slice(bytemuck::cast_slice(&data));
            drop(data);
            buffer.unmap();
            if let Some(rx) = next {
                pending = rx;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    /// The shader parses and validates without a device.
    #[test]
    fn the_shader_parses_and_validates() {
        let source = include_str!("amaze.wgsl");
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("amaze.wgsl:\n{}", e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("amaze.wgsl: {e:?}"));
    }

    /// A named constant of the shader, as const evaluation left it.
    fn constant(module: &naga::Module, name: &str) -> i64 {
        let (_, c) = module
            .constants
            .iter()
            .find(|(_, c)| c.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("amaze.wgsl has no constant {name}"));
        match module.global_expressions[c.init] {
            naga::Expression::Literal(naga::Literal::I32(v)) => i64::from(v),
            naga::Expression::Literal(naga::Literal::U32(v)) => i64::from(v),
            ref e => panic!("amaze.wgsl's {name} is {e:?}, not an integer"),
        }
    }

    /// The shader's geometry and plane layout are the host's: the tile,
    /// its margin and step, the cells a plane, the planes a binding, and
    /// every plane index below the count the buffers are made for (a
    /// plane past it would read the last binding's end, clamped, in
    /// silence).
    #[test]
    fn the_shader_lays_out_the_planes_as_the_host_sizes_them() {
        use super::{CELLS, MARGIN, PER_BINDING, PLANES, STEP, TS};
        let source = include_str!("amaze.wgsl");
        let module = naga::front::wgsl::parse_str(source).expect("amaze.wgsl parses");
        let at = |name| constant(&module, name);
        assert_eq!(at("TS"), TS as i64);
        assert_eq!(at("MARGIN"), MARGIN as i64);
        assert_eq!(at("STEP"), STEP as i64);
        assert_eq!(at("CELLS"), CELLS as i64);
        assert_eq!(at("PLANES"), PLANES as i64);
        assert_eq!(at("PER_BINDING"), PER_BINDING as i64);
        let names = [
            "CFA",
            "GREEN",
            "DELHVSQSUM",
            "DIRWTS0",
            "DIRWTS1",
            "VCD",
            "HCD",
            "VCDALT",
            "HCDALT",
            "CDDIFFSQ",
            "HVWT",
            "DGRB0",
            "DGRB1",
            "DGINTV",
            "DGINTH",
            "DGRB2H",
            "DGRB2V",
            "DELP",
            "DELM",
            "DGRBSQ1P",
            "DGRBSQ1M",
            "RBM",
            "RBP",
            "PMWT",
            "RBINT",
            "NYQUIST",
            "NYQUIST2",
        ];
        let mut planes: Vec<i64> = names.iter().map(|n| at(n)).collect();
        planes.sort_unstable();
        planes.dedup();
        assert_eq!(planes.len(), names.len(), "two planes share an index");
        assert!(
            planes.iter().all(|&p| (0..PLANES as i64).contains(&p)),
            "a plane index past the {PLANES} the host makes room for: {planes:?}"
        );
        // Four bindings in the layout: the planes must fit them.
        assert!(PLANES.div_ceil(PER_BINDING) <= 4);
    }

    /// The uniform sits where the WGSL puts it.
    #[test]
    fn the_params_are_laid_out_as_the_shader_reads_them() {
        use super::Params;
        use std::mem::offset_of;
        assert_eq!(offset_of!(Params, size), 0);
        assert_eq!(offset_of!(Params, tiles_across), 8);
        assert_eq!(offset_of!(Params, first_tile), 12);
        assert_eq!(offset_of!(Params, count), 16);
        assert_eq!(offset_of!(Params, band_y0), 20);
        assert_eq!(offset_of!(Params, ceiling), 24);
        assert_eq!(offset_of!(Params, colors), 32);
    }
}
