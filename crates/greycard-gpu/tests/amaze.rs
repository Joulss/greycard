//! The GPU AMaZE against its CPU reference, on the same mosaics.
//!
//! Without a GPU adapter each test says so and returns; it does not
//! pass in silence. (`cargo test` shows the line with `--nocapture`.)

use greycard_core::develop::amaze::demosaic_amaze;
use greycard_core::raw::{CfaColor, CfaPattern};
use greycard_gpu::Context;

fn context() -> Option<Context> {
    match Context::own() {
        Ok(c) => Some(c),
        Err(e) => {
            assert!(
                std::env::var_os("GREYCARD_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
                "the GPU AMaZE has nothing to run on ({e}) and GREYCARD_REQUIRE_GPU is set"
            );
            eprintln!("SKIPPED: the GPU AMaZE has nothing to run on ({e})");
            println!("SKIPPED: the GPU AMaZE has nothing to run on ({e})");
            None
        }
    }
}

fn bayer(name: &str) -> CfaPattern {
    let colors = name
        .chars()
        .map(|c| match c {
            'R' => CfaColor::Red,
            'G' => CfaColor::Green,
            _ => CfaColor::Blue,
        })
        .collect();
    CfaPattern::new(2, 2, colors).unwrap()
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A scene sampled through the pattern: `rgb(x, y)` in linear light,
/// green-normalized as the demosaic takes it.
fn mosaic(
    w: usize,
    h: usize,
    pattern: &CfaPattern,
    rgb: impl Fn(f32, f32) -> [f32; 3],
) -> Vec<f32> {
    (0..w * h)
        .map(|i| {
            let (y, x) = (i / w, i % w);
            rgb(x as f32, y as f32)[pattern.color_at(y, x).rgb_index().unwrap()]
        })
        .collect()
}

/// The scenes, by name: smooth gradients, edges at several angles, a
/// noisy field, a zone plate (fine lines at every angle and frequency,
/// the resolution chart's job), and the reference's own busy mosaic
/// with a clipped patch and a chroma checker.
fn scenes(w: usize, h: usize, pattern: &CfaPattern) -> Vec<(&'static str, Vec<f32>)> {
    let (wf, hf) = (w as f32, h as f32);
    let gradient = mosaic(w, h, pattern, |x, y| {
        [
            0.05 + 0.9 * x / wf,
            0.1 + 0.5 * y / hf,
            0.8 - 0.6 * (x + y) / (wf + hf),
        ]
    });
    let edges = mosaic(w, h, pattern, |x, y| {
        // A star of sixteen wedges about the middle, every edge at its
        // own angle, colored differently either side.
        let a = (y - hf / 2.0).atan2(x - wf / 2.0);
        let k = ((a / std::f32::consts::PI * 8.0).floor() as i32).rem_euclid(2);
        if k == 0 {
            [0.8, 0.6, 0.3]
        } else {
            [0.1, 0.2, 0.35]
        }
    });
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let noise: Vec<f32> = (0..w * h).map(|_| 0.05 + 0.9 * rng.next()).collect();
    let zone = mosaic(w, h, pattern, |x, y| {
        let (dx, dy) = (x - wf / 2.0, y - hf / 2.0);
        let r2 = dx * dx + dy * dy;
        let k = std::f32::consts::PI / wf.max(hf);
        let v = 0.5 + 0.4 * (k * r2).cos();
        [v * 0.9, v, v * 0.7]
    });
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let busy: Vec<f32> = (0..w * h)
        .map(|i| {
            let noise = rng.next();
            let (y, x) = (i / w, i % w);
            let c = pattern.color_at(y, x).rgb_index().unwrap() as f32;
            let fine = if (x + y) % 2 == 0 { 0.15 } else { -0.15 };
            let v = 0.35 + 0.2 * (x as f32 / 9.0).sin() + 0.05 * c + 0.1 * noise;
            let v = if x < w / 3 && y < h / 3 { v + fine } else { v };
            if (w / 2..w / 2 + 40).contains(&x) && (h / 2..h / 2 + 30).contains(&y) {
                1.3
            } else {
                v
            }
        })
        .collect();
    vec![
        ("gradient", gradient),
        ("edges", edges),
        ("noise", noise),
        ("zone plate", zone),
        ("busy", busy),
    ]
}

/// How far apart two demosaics are: the largest difference, and the
/// share of samples past 1e-4 and past 1e-3 of the reference's scale
/// (its largest value).
struct Apart {
    max: f32,
    scale: f32,
    over_4: usize,
    over_3: usize,
    len: usize,
    worst_at: usize,
}

fn apart(cpu: &[f32], gpu: &[f32]) -> Apart {
    assert_eq!(cpu.len(), gpu.len());
    let scale = cpu.iter().copied().fold(0f32, f32::max).max(1e-6);
    let mut a = Apart {
        max: 0.0,
        scale,
        over_4: 0,
        over_3: 0,
        len: cpu.len(),
        worst_at: 0,
    };
    for (k, (c, g)) in cpu.iter().zip(gpu).enumerate() {
        let d = (c - g).abs();
        assert!(g.is_finite(), "a sample that is not finite at {k}");
        if d > a.max {
            a.max = d;
            a.worst_at = k;
        }
        if d > 1e-4 * scale {
            a.over_4 += 1;
        }
        if d > 1e-3 * scale {
            a.over_3 += 1;
        }
    }
    a
}

impl std::fmt::Display for Apart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "max {:.3e} (scale {:.3}), {:.5}% past 1e-4, {:.5}% past 1e-3",
            self.max,
            self.scale,
            100.0 * self.over_4 as f64 / self.len as f64,
            100.0 * self.over_3 as f64 / self.len as f64
        )
    }
}

/// The share of samples that may be past 1e-3 of the scale: where a
/// threshold in the algorithm (the variance choices on a smooth ramp,
/// where both variances are rounding, the adaptive ratio, the Nyquist
/// test) falls the other way on a rounding difference of the GPU's (a
/// fused multiply-add, a division within its ULPs), and the recursions
/// carry it a few cells on. The reference against itself with its
/// input nudged a ULP (the ignored test below) moves 2.4e-3 of an edge
/// scene's samples past 1e-3 and by as much as 3e-2 relative. Measured:
/// at most 8e-6 on lavapipe, 1.4e-4 on the zone plate on an RTX 5070 Ti.
const SHARE_PAST_1E3: f64 = 1e-3;
/// On scenes this small a share is a handful of samples, too few for a
/// flip or two: this many may be past 1e-3 whatever the share.
const FEW_PAST_1E3: usize = 300;
/// The largest difference allowed anywhere, relative to the scale: over
/// the one-ULP yardstick's 3.3e-2 and anything measured (1.06e-3 on
/// lavapipe; 4.2e-3 on the 5070 Ti, 4.4e-2 at 24 and 45 MP there).
const MAX_RELATIVE: f32 = 1e-1;
/// The share on a real frame: 3.6e-6 on lavapipe, 8.7e-6 on the 5070
/// Ti; at 45 MP a share of 1e-3 would be 135k samples, enough to hide
/// a seam row.
const REAL_SHARE_PAST_1E3: f64 = 1e-4;

#[test]
fn synthetic_mosaics_match_the_reference() {
    let Some(ctx) = context() else {
        return;
    };
    // Sizes that cut the last row and column of tiles short, one that
    // does not, and one under a tile's interior (128) and just over the
    // size that falls back to bilinear (34).
    for (w, h) in [(300usize, 270usize), (256, 256), (420, 333), (40, 37)] {
        for name in ["RGGB", "BGGR", "GRBG", "GBRG"] {
            let pattern = bayer(name);
            for (scene, samples) in scenes(w, h, &pattern) {
                let cpu = demosaic_amaze(&samples, w, h, &pattern).unwrap();
                let gpu = ctx.demosaic_amaze(&samples, w, h, &pattern).unwrap();
                let a = apart(&cpu, &gpu);
                let px = a.worst_at / 3;
                println!(
                    "{w}x{h} {name} {scene}: {a}; worst at ({}, {}) channel {}",
                    px % w,
                    px / w,
                    a.worst_at % 3
                );
                assert!(
                    a.max <= MAX_RELATIVE * a.scale,
                    "{w}x{h} {name} {scene}: {a}"
                );
                assert!(
                    a.over_3 as f64 <= (SHARE_PAST_1E3 * a.len as f64).max(FEW_PAST_1E3 as f64),
                    "{w}x{h} {name} {scene}: {a}"
                );
            }
        }
    }
}

#[test]
fn the_batches_meet_without_a_seam() {
    let Some(ctx) = context() else {
        return;
    };
    // Four rows of tiles: one batch, a batch a row, and batches of
    // three, whose last has fewer tiles than the buffers are made for.
    let (w, h) = (300, 420);
    let pattern = bayer("GRBG");
    for (scene, samples) in scenes(w, h, &pattern) {
        let whole = ctx.demosaic_amaze(&samples, w, h, &pattern).unwrap();
        for rows in [1, 3] {
            let batched = ctx
                .demosaic_amaze_in_batches(&samples, w, h, &pattern, Some(rows))
                .unwrap();
            assert!(
                whole
                    .iter()
                    .zip(&batched)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{scene}: batches of {rows} tile rows differ from one batch"
            );
        }
    }
}

#[test]
fn small_frames_and_other_patterns_are_the_references() {
    let Some(ctx) = context() else {
        return;
    };
    let pattern = bayer("RGGB");
    let samples = vec![0.5f32; 20 * 20];
    assert_eq!(
        ctx.demosaic_amaze(&samples, 20, 20, &pattern).unwrap(),
        demosaic_amaze(&samples, 20, 20, &pattern).unwrap()
    );
    let xtrans = CfaPattern::xtrans();
    assert!(
        ctx.demosaic_amaze(&vec![0.5; 36 * 36], 36, 36, &xtrans)
            .is_err()
    );
}

/// The balanced mosaic of every Bayer raw in `GREYCARD_SAMPLES`, as
/// `prepare` hands it to the demosaic, against the reference. Run it in
/// release: the reference takes long a frame unoptimized.
#[test]
#[ignore]
fn the_real_frames_match_the_reference() {
    let Some(dir) = std::env::var_os("GREYCARD_SAMPLES") else {
        println!("SKIPPED: GREYCARD_SAMPLES is not set");
        return;
    };
    let Some(ctx) = context() else {
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("read GREYCARD_SAMPLES")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| greycard_core::decode::is_raw_path(p))
        .collect();
    files.sort();
    let mut checked = 0;
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let frame = match greycard_core::decode::decode_path(&path) {
            Ok(f) => f,
            Err(e) => {
                println!("{name}: passed over, it does not decode ({e})");
                continue;
            }
        };
        let settings = greycard_core::develop::DevelopSettings::default();
        let prepared = greycard_core::develop::prepare(&frame, &settings, false).expect("prepares");
        let Some(pattern) = prepared
            .pattern
            .as_ref()
            .filter(|p| greycard_core::develop::rcd::is_bayer(p))
        else {
            println!("{name}: passed over, not a Bayer mosaic");
            continue;
        };
        let (w, h, samples) = (prepared.width, prepared.height, &prepared.samples);
        let cpu = demosaic_amaze(samples, w, h, pattern).unwrap();
        let gpu = ctx.demosaic_amaze(samples, w, h, pattern).unwrap();
        let a = apart(&cpu, &gpu);
        let px = a.worst_at / 3;
        println!(
            "{name}: {w}x{h}, {a}; worst at ({}, {}) channel {}",
            px % w,
            px / w,
            a.worst_at % 3
        );
        assert!(a.max <= MAX_RELATIVE * a.scale, "{name}: {a}");
        assert!(
            a.over_3 as f64 <= REAL_SHARE_PAST_1E3 * a.len as f64,
            "{name}: {a}"
        );
        checked += 1;
    }
    assert!(checked > 0, "no Bayer raw in GREYCARD_SAMPLES");
}

/// The time of the GPU AMaZE and of the reference on a synthetic
/// mosaic at 24 and 45 MP, the GPU warm (its second run). For the real
/// GPU, in release:
///
/// `cargo test --release -p greycard-gpu --test amaze -- --ignored
/// timing --nocapture`
#[test]
#[ignore]
fn timing() {
    let Some(ctx) = context() else {
        return;
    };
    println!("on {}", ctx.name());
    let pattern = bayer("RGGB");
    for (w, h) in [(6000usize, 4000usize), (8256, 5504)] {
        let (_, samples) = scenes(w, h, &pattern).swap_remove(4);
        let cold = std::time::Instant::now();
        let gpu = ctx.demosaic_amaze(&samples, w, h, &pattern).unwrap();
        let cold = cold.elapsed();
        let mut warm = std::time::Duration::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let again = ctx.demosaic_amaze(&samples, w, h, &pattern).unwrap();
            warm = warm.min(t.elapsed());
            assert_eq!(again.len(), gpu.len());
        }
        let t = std::time::Instant::now();
        let cpu = demosaic_amaze(&samples, w, h, &pattern).unwrap();
        let cpu_time = t.elapsed();
        println!(
            "{w}x{h} ({:.1} MP): GPU {:.1} ms warm (best of 3), {:.1} ms cold; CPU {:.1} ms on {} \
             threads; {}",
            (w * h) as f64 / 1e6,
            warm.as_secs_f64() * 1e3,
            cold.as_secs_f64() * 1e3,
            cpu_time.as_secs_f64() * 1e3,
            rayon::current_num_threads(),
            apart(&cpu, &gpu)
        );
    }
}

/// How far the reference moves from itself when every input sample is
/// nudged by one ULP: the algorithm's own conditioning, the yardstick
/// for the GPU's differences.
#[test]
#[ignore]
fn the_reference_against_itself_nudged() {
    for (w, h) in [(300usize, 270usize), (420, 333)] {
        for name in ["RGGB", "GBRG"] {
            let pattern = bayer(name);
            for (scene, samples) in scenes(w, h, &pattern) {
                let nudged: Vec<f32> = samples
                    .iter()
                    .enumerate()
                    .map(|(k, &v)| {
                        if k % 3 == 0 {
                            f32::from_bits(v.to_bits() + 1)
                        } else {
                            v
                        }
                    })
                    .collect();
                let a = demosaic_amaze(&samples, w, h, &pattern).unwrap();
                let b = demosaic_amaze(&nudged, w, h, &pattern).unwrap();
                println!("{w}x{h} {name} {scene}, nudged: {}", apart(&a, &b));
            }
        }
    }
}
