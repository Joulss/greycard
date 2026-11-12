//! The viewport's half floats: every op that writes the viewport's
//! texture rounds to the nearest half float, ties to even, as the CPU
//! path's `Halves` does through the `half` crate. A store to an
//! `rgba16float` texture rounds as the device likes (toward zero on
//! NVIDIA and lavapipe), so the shaders round first, in `half.wgsl`.
//!
//! Without a GPU adapter each test says so and returns; it does not
//! pass in silence.

use greycard_core::develop::dehaze::{DehazeOptions, dehaze};
use greycard_core::develop::local_contrast::{LocalContrastOptions, local_contrast};
use greycard_core::develop::sharpen::{SharpenOptions, sharpen_with_mask};
use greycard_core::image::WorkingImage;
use greycard_gpu::{Context, wgpu};
use half::f16;

fn context() -> Option<Context> {
    match Context::own() {
        Ok(c) => Some(c),
        Err(e) => {
            assert!(
                std::env::var_os("GREYCARD_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
                "the half-float tests have nothing to run on ({e}) and GREYCARD_REQUIRE_GPU is set"
            );
            println!("SKIPPED: the half-float tests have nothing to run on ({e})");
            None
        }
    }
}

/// Every half float, the point halfway to the next (a tie), and the
/// f32 either side of that point, both signs; the f32 subnormals, the
/// zeros, the infinities and the edges of the largest finite half; and
/// a spread of f32 values from 2^-30 to 2^17.
fn sweep() -> Vec<f32> {
    let mut out = Vec::new();
    for bits in 0..0x7c00u16 {
        let h = f16::from_bits(bits).to_f32();
        let next = f16::from_bits(bits + 1).to_f32();
        let mid = (f64::from(h) + f64::from(next)) / 2.0;
        // The tie is exact in f32: a half has eleven bits, an f32
        // twenty-four.
        let mid = mid as f32;
        for v in [
            h,
            mid,
            f32::from_bits(mid.to_bits() - 1),
            f32::from_bits(mid.to_bits() + 1),
        ] {
            out.push(v);
            out.push(-v);
        }
    }
    for v in [
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        f32::MIN_POSITIVE,
        1e-40,
        -1e-40,
        65504.0,
        65519.996,
        65520.0,
        65536.0,
        2f32.powi(-24),
        2f32.powi(-25),
        f32::from_bits(2f32.powi(-25).to_bits() + 1),
        2f32.powi(-14),
        f32::from_bits(2f32.powi(-14).to_bits() - 1),
    ] {
        out.push(v);
    }
    let mut seed = 0x6772_6579u64;
    for _ in 0..200_000 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let e = ((seed >> 40) % 48) as i32 - 30;
        let m = 1.0 + ((seed >> 8) & 0xffffff) as f32 / (1u32 << 24) as f32;
        let v = m * 2f32.powi(e);
        out.push(if seed & 1 == 0 { v } else { -v });
    }
    while !out.len().is_multiple_of(4) {
        out.push(0.0);
    }
    out
}

const WIDTH: u32 = 1024;

const PROBE: &str = r#"
@group(0) @binding(0) var<storage, read> input: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> rounded: array<vec4<f32>>;
@group(0) @binding(2) var stored: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var bare: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.y * 65535u * 64u + gid.x;
    if i >= arrayLength(&input) {
        return;
    }
    let v = input[i];
    let r = halves_nearest(v);
    rounded[i] = r;
    let at = vec2<u32>(i % 1024u, i / 1024u);
    textureStore(stored, at, r);
    textureStore(bare, at, v);
}
"#;

fn read_buffer(ctx: &Context, buffer: &wgpu::Buffer, size: u64) -> Vec<u8> {
    let staging = ctx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
    ctx.queue().submit(Some(encoder.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    ctx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device polls");
    rx.recv().expect("the map callback comes").expect("maps");
    slice.get_mapped_range().expect("the mapped range").to_vec()
}

fn halves_of(bytes: &[u8]) -> Vec<f16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| f16::from_bits(u16::from_le_bytes(*p)))
        .collect()
}

/// `half_nearest` against the `half` crate's `from_f32` on the sweep:
/// the f32 it hands back is that half's value to the bit, and the
/// store of it is that half to the bit, on whatever rounding the
/// device's store has. The bare store's misses are printed, to show
/// what the rounding is for.
#[test]
fn the_rounding_is_the_half_crates() {
    use wgpu::util::DeviceExt;
    let Some(ctx) = context() else {
        return;
    };
    let values = sweep();
    let texels = values.len() as u32 / 4;
    let rows = texels.div_ceil(WIDTH);
    let device = ctx.device();
    let source = format!("{}\n{PROBE}", include_str!("../src/half.wgsl"));
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("half probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("half probe"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("values"),
        contents: bytemuck::cast_slice(&values),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let size = (values.len() * 4) as u64;
    let rounded = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rounded"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let stored = ctx.viewport_texture(WIDTH, rows);
    let bare = ctx.viewport_texture(WIDTH, rows);
    let view = |t: &wgpu::Texture| t.create_view(&Default::default());
    let (stored_view, bare_view) = (view(&stored), view(&bare));
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("half probe"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: rounded.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&stored_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&bare_view),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        let groups = texels.div_ceil(64);
        pass.dispatch_workgroups(groups.min(65535), groups.div_ceil(65535), 1);
    }
    ctx.queue().submit(Some(encoder.finish()));
    let rounded: Vec<f32> = bytemuck::cast_slice(&read_buffer(&ctx, &rounded, size)).to_vec();
    let stored = halves_of(&ctx.read_viewport_rows(&stored, 0, rows).expect("reads"));
    let bare = halves_of(&ctx.read_viewport_rows(&bare, 0, rows).expect("reads"));
    let mut bare_misses = 0usize;
    for (i, &v) in values.iter().enumerate() {
        let want = f16::from_f32(v);
        if v.is_nan() {
            assert!(rounded[i].is_nan() && stored[i].is_nan(), "{v:e}");
            continue;
        }
        assert_eq!(
            rounded[i].to_bits(),
            want.to_f32().to_bits(),
            "{v:e} ({:#010x}) rounded to {:e}, the half crate's {want}",
            v.to_bits(),
            rounded[i]
        );
        assert_eq!(
            stored[i].to_bits(),
            want.to_bits(),
            "{v:e} ({:#010x}) stored as {}, the half crate's {want}",
            v.to_bits(),
            stored[i]
        );
        if bare[i].to_bits() != want.to_bits() {
            bare_misses += 1;
        }
    }
    println!(
        "{}: {} values, the bare store off the nearest half in {bare_misses}",
        ctx.name(),
        values.len()
    );
}

/// A picture for the ops: smooth color across two decades, fine
/// noise, a haze over its upper half, deep shadow, and a clipped patch.
fn scene(w: usize, h: usize) -> WorkingImage {
    let mut image = WorkingImage::new(w, h);
    let mut seed = 0x9E37_79B9u32;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 - 0.5
    };
    let (pixels, _) = image.data.as_chunks_mut::<3>();
    for (i, px) in pixels.iter_mut().enumerate() {
        let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
        let base = 0.01 * 100f32.powf(x);
        let bars = if (i % w / 9 + i / w / 7).is_multiple_of(2) {
            1.0
        } else {
            0.6
        };
        let haze = (1.0 - 2.0 * y).max(0.0) * 0.6;
        let tint = [1.0 + 0.3 * y, 1.0, 1.2 - 0.4 * y];
        let air = [0.92, 0.9, 0.85];
        for c in 0..3 {
            let v = base * bars * tint[c] * (1.0 + 0.05 * rand());
            px[c] = v * (1.0 - haze) + air[c] * haze;
        }
        if x > 0.9 && y > 0.8 {
            *px = [4.0, 3.5, 3.0];
        }
        if x < 0.05 && y > 0.9 {
            *px = [1e-4 * (1.0 + rand()), 2e-5, 0.0];
        }
    }
    image
}

/// How a texture's halves sit against a plane of f32s rounded to
/// nearest: the share off by any amount, the largest distance in half
/// ulps, and the mean signed distance, the bias.
#[derive(Debug, Default)]
struct Against {
    samples: usize,
    off: usize,
    max_ulps: u32,
    sum_ulps: i64,
}

impl Against {
    fn add(&mut self, got: f16, want: f32) {
        let want = f16::from_f32(want);
        self.samples += 1;
        // Ulps along the line of halves, the signs folded in.
        let line = |h: f16| {
            let b = i32::from(h.to_bits());
            if b & 0x8000 != 0 { -(b & 0x7fff) } else { b }
        };
        let d = line(got) - line(want);
        if d != 0 {
            self.off += 1;
            self.max_ulps = self.max_ulps.max(d.unsigned_abs());
            self.sum_ulps += i64::from(d);
        }
    }

    fn line(&self) -> String {
        format!(
            "{:.2}% off, worst {} ulps, mean {:+.4} ulps",
            100.0 * self.off as f64 / self.samples as f64,
            self.max_ulps,
            self.sum_ulps as f64 / self.samples as f64
        )
    }
}

/// One op's viewport texture against the same op's full floats on the
/// device (where only the store differs) and against the CPU
/// reference's, both rounded as the CPU path's `Halves` rounds them.
fn measure(
    op: &str,
    texture: &[f16],
    gpu: &[f32],
    gpu_alpha: Option<&[f32]>,
    cpu: &[f32],
    cpu_alpha: Option<&[f32]>,
) -> (Against, Against) {
    let (mut own, mut reference) = (Against::default(), Against::default());
    for (i, px) in texture.as_chunks::<4>().0.iter().enumerate() {
        for c in 0..3 {
            own.add(px[c], gpu[i * 3 + c]);
            reference.add(px[c], cpu[i * 3 + c]);
        }
        own.add(px[3], gpu_alpha.map_or(0.0, |a| a[i]));
        reference.add(px[3], cpu_alpha.map_or(0.0, |a| a[i]));
    }
    println!("{op}: against its full floats {}", own.line());
    println!("{op}: against the CPU's halves {}", reference.line());
    (own, reference)
}

const W: usize = 600;
const H: usize = 400;

/// Each op's viewport texture is its full floats rounded to nearest,
/// to the bit, and sits against the CPU reference's halves within the
/// op's own rounding, with no bias.
#[test]
fn the_viewport_halves_round_to_nearest() {
    let Some(ctx) = context() else {
        return;
    };
    let image = scene(W, H);
    let uploaded = ctx.upload(&image).expect("uploads");
    let out = ctx.viewport_texture(W as u32, H as u32);
    let read = |t: &wgpu::Texture| halves_of(&ctx.read_viewport_rows(t, 0, H as u32).unwrap());
    let mut results = Vec::new();

    // The sharpen, its blend mask in the alpha.
    let options = SharpenOptions::default();
    ctx.sharpen(&uploaded, None, &options, Some(1.0), 2.0, &out)
        .expect("sharpens");
    let texture = read(&out);
    let mut gpu = image.clone();
    let (_, gpu_mask) = ctx
        .sharpen_image(&mut gpu, &options, Some(1.0), 2.0)
        .expect("sharpens");
    let mut cpu = image.clone();
    let (_, cpu_mask) = sharpen_with_mask(&mut cpu, None, &options, Some(1.0), 2.0);
    results.push((
        "sharpen",
        measure(
            "sharpen",
            &texture,
            &gpu.data,
            Some(&gpu_mask),
            &cpu.data,
            Some(&cpu_mask),
        ),
    ));

    // The sharpen's delta alone: the picture as it was.
    ctx.sharpen_with_delta(&uploaded, None, &options, Some(1.0), 2.0, &out, false)
        .expect("runs");
    let texture = read(&out);
    results.push((
        "unsharpened",
        measure(
            "unsharpened",
            &texture,
            &image.data,
            None,
            &image.data,
            None,
        ),
    ));

    // The local contrast.
    let options = LocalContrastOptions {
        texture: 0.5,
        clarity: 0.5,
        ..Default::default()
    };
    ctx.local_contrast_to_viewport(&uploaded, &options, 4.0, &out)
        .expect("runs");
    let texture = read(&out);
    let mut gpu = image.clone();
    ctx.local_contrast_image(&mut gpu, &options, 4.0)
        .expect("runs");
    let mut cpu = image.clone();
    local_contrast(&mut cpu, &options, 4.0);
    results.push((
        "local contrast",
        measure("local contrast", &texture, &gpu.data, None, &cpu.data, None),
    ));

    // The dehaze.
    let options = DehazeOptions { amount: 0.6 };
    let model = ctx
        .dehaze_reduced(&uploaded)
        .expect("sums")
        .model(&options)
        .expect("a model");
    ctx.dehaze_to_viewport(&uploaded, &model, &out)
        .expect("runs");
    let texture = read(&out);
    let (full, _) = ctx.dehaze(&uploaded, &model).expect("runs");
    let gpu = ctx.download(&full).expect("reads back");
    let mut cpu = image.clone();
    dehaze(&mut cpu, &options);
    results.push((
        "dehaze",
        measure("dehaze", &texture, &gpu.data, None, &cpu.data, None),
    ));

    for (op, (own, reference)) in &results {
        assert_eq!(own.off, 0, "{op}: {}", own.line());
        // Toward zero this was a third of an ulp low on the mean,
        // a third of the samples a whole ulp.
        assert!(reference.max_ulps <= 1, "{op}: {}", reference.line());
        assert!(
            (reference.sum_ulps as f64 / reference.samples as f64).abs() < 0.01,
            "{op}: {}",
            reference.line()
        );
    }
}

/// The ops into the viewport's texture at 24 MP, the median of five
/// after a warm run, for the cost of the rounding. Run in release.
#[test]
#[ignore]
fn the_viewport_ops_at_24_mp() {
    let Some(ctx) = context() else {
        return;
    };
    let (w, h) = (6000, 4000);
    let image = scene(w, h);
    let uploaded = ctx.upload(&image).expect("uploads");
    let out = ctx.viewport_texture(w as u32, h as u32);
    let median = |run: &mut dyn FnMut()| {
        run();
        let mut t: Vec<f64> = (0..5)
            .map(|_| {
                let started = std::time::Instant::now();
                run();
                started.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        t.sort_by(f64::total_cmp);
        t[2]
    };
    let sync = |ctx: &Context| {
        ctx.read_viewport_rows(&out, 0, 1).expect("reads");
    };
    let sharpen = median(&mut || {
        ctx.sharpen(
            &uploaded,
            None,
            &SharpenOptions::default(),
            Some(1.0),
            2.0,
            &out,
        )
        .expect("sharpens");
        sync(&ctx);
    });
    let lc = LocalContrastOptions {
        texture: 0.5,
        clarity: 0.5,
        ..Default::default()
    };
    let local = median(&mut || {
        ctx.local_contrast_to_viewport(&uploaded, &lc, 4.0, &out)
            .expect("runs");
        sync(&ctx);
    });
    let model = ctx
        .dehaze_reduced(&uploaded)
        .expect("sums")
        .model(&DehazeOptions { amount: 0.6 })
        .expect("a model");
    let haze = median(&mut || {
        ctx.dehaze_to_viewport(&uploaded, &model, &out)
            .expect("runs");
        sync(&ctx);
    });
    println!(
        "{} at 24 MP: sharpen {sharpen:.1} ms, local contrast {local:.1} ms, dehaze apply {haze:.1} ms",
        ctx.name()
    );
}
