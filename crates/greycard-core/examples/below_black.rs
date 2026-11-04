//! What clamping the mosaic at zero after the black does to a frame's
//! deepest shadows, so the table in the notes can be taken again.
//!
//! The levels are normalized both ways here (clamped at zero, and
//! kept below it), and each goes through the noise estimate, the
//! gains, the default demosaic, the profiled denoise, and the matrix.
//! The kept one is clamped at zero before the denoise, which cannot
//! take a value below zero: with a read noise fitted at zero, its
//! transform sends one past its inverse's pole. The darkest
//! blocks of the frame are read at each stage. Then [`develop`] under
//! its defaults, with and without the denoise, is read in the same
//! blocks: what the library does as built.
//!
//! ```text
//! cargo run --release -p greycard-core --example below_black -- FILE
//! ```

use greycard_core::color;
use greycard_core::decode::decode_path;
use greycard_core::develop::denoise::{DenoiseOptions, denoise_profiled};
use greycard_core::develop::noise::{NoiseModel, estimate_noise};
use greycard_core::develop::{
    DemosaicMethod, DevelopSettings, apply_gains_cfa, apply_matrix, crop_samples,
    demosaic_cfa_with, develop,
};
use greycard_core::raw::{CfaPattern, Orientation, RawFrame, SensorLayout};

/// Blocks a multiple of both mosaics' periods.
const BLOCK: usize = 24;

fn normalize(frame: &RawFrame, clamp: bool) -> Vec<f32> {
    assert!(
        frame.levels.black_deltas.is_none(),
        "black deltas not read here"
    );
    let (w, cpp) = (frame.width, frame.channels);
    let mut out = frame.samples.to_f32();
    for (i, s) in out.iter_mut().enumerate() {
        let (y, x, ch) = (i / (w * cpp), (i / cpp) % w, i % cpp);
        let b = frame.levels.black.at(y, x, ch);
        let range = (frame.levels.white.at(y, x, ch) - b).max(f32::EPSILON);
        let v = (*s - b) / range;
        *s = if clamp { v.clamp(0.0, 1.0) } else { v.min(1.0) };
    }
    out
}

/// Per channel mean and standard deviation of the mosaic's photosites
/// in the blocks.
fn mosaic_stats(m: &[f32], w: usize, p: &CfaPattern, blocks: &[(usize, usize)]) -> [(f64, f64); 3] {
    let mut acc = [(0f64, 0f64, 0usize); 3];
    for &(by, bx) in blocks {
        for y in by..by + BLOCK {
            for x in bx..bx + BLOCK {
                let c = p.color_at(y, x).rgb_index().unwrap();
                let v = m[y * w + x] as f64;
                acc[c].0 += v;
                acc[c].1 += v * v;
                acc[c].2 += 1;
            }
        }
    }
    acc.map(finish)
}

fn rgb_stats(rgb: &[f32], w: usize, blocks: &[(usize, usize)]) -> [(f64, f64); 3] {
    let mut acc = [(0f64, 0f64, 0usize); 3];
    for &(by, bx) in blocks {
        for y in by..by + BLOCK {
            for x in bx..bx + BLOCK {
                for (c, a) in acc.iter_mut().enumerate() {
                    let v = rgb[(y * w + x) * 3 + c] as f64;
                    a.0 += v;
                    a.1 += v * v;
                    a.2 += 1;
                }
            }
        }
    }
    acc.map(finish)
}

fn finish((s, ss, n): (f64, f64, usize)) -> (f64, f64) {
    let n = n.max(1) as f64;
    let mean = s / n;
    (mean, (ss / n - mean * mean).max(0.0).sqrt())
}

fn row(label: &str, s: [(f64, f64); 3], scale: f64) {
    let f = |(m, d): (f64, f64)| format!("{:>9.3} {:>8.3}", m * scale, d * scale);
    println!("| {label:<28} | {} | {} | {} |", f(s[0]), f(s[1]), f(s[2]));
}

fn tint(label: &str, s: [(f64, f64); 3]) {
    let (r, g, b) = (s[0].0, s[1].0, s[2].0);
    println!(
        "| {label:<28} | R/G {:>6.3} | B/G {:>6.3} | Y {:>9.3} |",
        r / g,
        b / g,
        (0.2627 * r + 0.6780 * g + 0.0593 * b) * 1e3
    );
}

fn model(label: &str, m: &NoiseModel) {
    println!(
        "| {label:<28} | a {:.3e} {:.3e} {:.3e} | b {:.3e} {:.3e} {:.3e} |",
        m.a[0], m.a[1], m.a[2], m.b[0], m.b[1], m.b[2]
    );
}

fn main() {
    let file = std::env::args().nth(1).expect("want: FILE");
    let frame = decode_path(&file).expect("decode");
    let SensorLayout::Cfa(pattern) = &frame.layout else {
        panic!("not a CFA frame");
    };
    let crop = frame.effective_crop();
    let pattern = pattern.shifted(crop.x, crop.y);
    let cut = |m: &[f32]| crop_samples(m, frame.width, frame.channels, crop);
    let (clamped, w, h) = cut(&normalize(&frame, true));
    let (kept, _, _) = cut(&normalize(&frame, false));
    let below = kept.iter().filter(|v| **v < 0.0).count();
    let at_black = kept.iter().filter(|v| **v == 0.0).count();
    let lowest = kept.iter().copied().fold(f32::INFINITY, f32::min);
    println!(
        "{w}x{h} {pattern}, black {:?}, white {:?}; photosites below black {below} ({:.4}%), at black {at_black}, lowest {lowest:.5}",
        frame.levels.black.at(0, 0, 0),
        frame.levels.white.at(0, 0, 0),
        100.0 * below as f64 / kept.len() as f64
    );

    // The darkest 1% of the blocks, by the kept mosaic's mean, and the
    // band from 5% to 10% for where the bias fades.
    let mut blocks: Vec<(f64, (usize, usize))> = Vec::new();
    for by in (BLOCK..h - 2 * BLOCK).step_by(BLOCK) {
        for bx in (BLOCK..w - 2 * BLOCK).step_by(BLOCK) {
            let mut s = 0f64;
            for y in by..by + BLOCK {
                for x in bx..bx + BLOCK {
                    s += kept[y * w + x] as f64;
                }
            }
            blocks.push((s, (by, bx)));
        }
    }
    blocks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = blocks.len();
    let pick = |from: f64, to: f64| -> Vec<(usize, usize)> {
        blocks[(n as f64 * from) as usize..(n as f64 * to) as usize]
            .iter()
            .map(|b| b.1)
            .collect()
    };
    let bands = [
        ("darkest 1%", pick(0.0, 0.01)),
        ("5% to 10%", pick(0.05, 0.10)),
    ];

    let profile = color::profile_from_frame(&frame).expect("profile");
    let wb = color::resolve_white_balance(&frame, &profile, DevelopSettings::default().white_point)
        .expect("wb");
    let gains = wb.coefficients_f32();
    let method = DemosaicMethod::default().for_mosaic(&pattern);
    let contrast = DevelopSettings::default().dual_contrast;

    struct Run {
        mosaic: Vec<f32>,
        model: NoiseModel,
        demosaiced: Vec<f32>,
        denoised: Vec<f32>,
        developed: Vec<f32>,
    }
    let run = |mosaic: &[f32], clamp_before: bool| -> Run {
        let model = estimate_noise(mosaic, w, h, &pattern).expect("noise").model;
        let mut balanced = mosaic.to_vec();
        apply_gains_cfa(&mut balanced, w, &pattern, gains);
        let (demosaiced, _) =
            demosaic_cfa_with(&balanced, w, h, &pattern, method, contrast).expect("demosaic");
        let mut denoised = demosaiced.clone();
        if clamp_before {
            denoised.iter_mut().for_each(|v| *v = v.max(0.0));
        }
        denoise_profiled(
            &mut denoised,
            w,
            h,
            &model.after_gains(gains),
            &DenoiseOptions::default(),
        )
        .expect("denoise");
        let mut developed = denoised.clone();
        apply_matrix(&mut developed, wb.matrix_f32());
        Run {
            mosaic: mosaic.to_vec(),
            model,
            demosaiced,
            denoised,
            developed,
        }
    };
    let runs = [
        ("clamped", run(&clamped, false)),
        ("kept", run(&kept, true)),
    ];
    let normal = DevelopSettings {
        orientation: Some(Orientation::Normal),
        ..DevelopSettings::default()
    };
    let plain = develop(&frame, &normal).expect("develop");
    let denoised = develop(
        &frame,
        &DevelopSettings {
            denoise: Some(DenoiseOptions::default()),
            ..normal.clone()
        },
    )
    .expect("develop");
    assert_eq!((plain.image.width, plain.image.height), (w, h));

    for (name, r) in &runs {
        let wild = r
            .denoised
            .iter()
            .filter(|v| !(-0.5..=4.0).contains(*v))
            .count();
        let worst = r.denoised.iter().copied().fold(0f32, |m, v| m.max(v.abs()));
        println!("{name}: {wild} denoised values outside -0.5..4, largest magnitude {worst:.3e}");
    }
    for (name, d) in [("plain", &plain), ("denoised", &denoised)] {
        let wild = d
            .image
            .data
            .iter()
            .filter(|v| !(-0.5..=4.0).contains(*v))
            .count();
        println!("develop(), {name}: {wild} values outside -0.5..4");
    }
    println!("\nnoise model (before gains)");
    for (name, r) in &runs {
        model(name, &r.model);
    }
    for (band, blocks) in &bands {
        println!("\n{band} ({} blocks), mean and sd x1000", blocks.len());
        println!(
            "| stage                        |     R mean     sd |     G mean     sd |     B mean     sd |"
        );
        for (name, r) in &runs {
            row(
                &format!("mosaic, {name}"),
                mosaic_stats(&r.mosaic, w, &pattern, blocks),
                1e3,
            );
        }
        for (name, r) in &runs {
            row(
                &format!("demosaiced, {name}"),
                rgb_stats(&r.demosaiced, w, blocks),
                1e3,
            );
        }
        for (name, r) in &runs {
            row(
                &format!("denoised, {name}"),
                rgb_stats(&r.denoised, w, blocks),
                1e3,
            );
        }
        for (name, r) in &runs {
            row(
                &format!("developed, {name}"),
                rgb_stats(&r.developed, w, blocks),
                1e3,
            );
        }
        row(
            "develop(), plain",
            rgb_stats(&plain.image.data, w, blocks),
            1e3,
        );
        row(
            "develop(), denoised",
            rgb_stats(&denoised.image.data, w, blocks),
            1e3,
        );
        for (name, r) in &runs {
            tint(
                &format!("developed, {name}"),
                rgb_stats(&r.developed, w, blocks),
            );
        }
        tint("develop(), plain", rgb_stats(&plain.image.data, w, blocks));
        tint(
            "develop(), denoised",
            rgb_stats(&denoised.image.data, w, blocks),
        );
    }
}
