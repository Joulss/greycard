//! Focus peaking: the sharpest parts of the picture as displayed,
//! painted over it in one color. The CPU reference the viewport's
//! peaking passes (`peaking.wgsl`) are held to.
//!
//! The map is core's ([`greycard_core::sharpness`]): the energy of the
//! Laplacian of the log luminance, smoothed. It is taken on the
//! viewport's own pixels, at the view's scale, so a picture at Fit,
//! which the downsampling hides a missed focus in, paints less than
//! the same picture at 1:1, as it looks.
//!
//! The threshold is the frame's own, never a fixed number, since the
//! Laplacian grows with the ISO and the sharpening: a pixel paints
//! when its energy is within [`BELOW`] of the map's [`QUANTILE`] over
//! the picture. A frame sharp in one place has its top there and
//! little near it; a frame sharp everywhere has most of itself near
//! its top. Both are counted in bins of the log of the energy, so
//! the GPU's comparison is the CPU's, bin for bin.

use greycard_core::sharpness;

/// The smoothing's radius, display pixels: a 5 by 5 window, small
/// enough that the paint follows an eyelash.
pub const RADIUS: usize = 2;

/// The bins of the energy's log2: from [`LOW`], [`STEPS`] an octave.
/// Zero energy, a flat patch, is the first bin.
pub const BINS: usize = 768;
pub const LOW: f32 = -32.0;
pub const STEPS: f32 = 16.0;

/// The share of the picture's pixels at or under the reference level.
pub const QUANTILE: f32 = 0.995;

/// How far under the reference level a pixel still paints, in
/// octaves of energy.
pub const BELOW: f32 = 3.0;

/// A little light added under the log, in stops: black is then a level
/// and not minus infinity, and the shadows, where a step of eight bits
/// is a stop, ease into a plain measure rather than meeting a hard
/// floor, whose knee would paint a dark ground's grain.
pub const SHADOW: f32 = -5.0;

/// A rectangle of the target the picture covers, pixels, the far
/// edges outside it: `[x0, y0, x1, y1]`.
pub type Rect = [u32; 4];

/// An encoded value, eight bits, to linear light, by the sRGB curve.
fn linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The log2 luminance of a displayed pixel, eased in the shadows.
pub fn log_luminance(px: [u8; 4]) -> f32 {
    let y = 0.2126 * linear(px[0]) + 0.7152 * linear(px[1]) + 0.0722 * linear(px[2]);
    (y + SHADOW.exp2()).log2()
}

/// The bin an energy lands in.
pub fn bin(energy: f32) -> u32 {
    // Zero, and the NaN a broken pixel would give: no log.
    if energy.is_nan() || energy <= 0.0 {
        return 0;
    }
    let b = ((energy.log2() - LOW) * STEPS).floor();
    b.clamp(0.0, (BINS - 1) as f32) as u32
}

/// The first bin that paints, from the picture's counts: the bin
/// [`QUANTILE`] of them reach, less [`BELOW`] octaves, and never the
/// first (a flat picture paints nothing).
pub fn threshold(counts: &[u32]) -> u32 {
    let total: u64 = counts.iter().map(|&c| c as u64).sum();
    // Less a thousandth, so the f32 quantile's rounding up cannot ask
    // for one count more than the share.
    let want = (total as f64 * QUANTILE as f64 - 1e-3).ceil() as u64;
    let mut seen = 0u64;
    let mut at = counts.len() as u32 - 1;
    for (k, &c) in counts.iter().enumerate() {
        seen += c as u64;
        if seen >= want.max(1) {
            at = k as u32;
            break;
        }
    }
    at.saturating_sub((BELOW * STEPS) as u32).max(1)
}

/// The sharpness map of a `width` wide target inside each of `rects`,
/// each rectangle a plane of its own, so neither the canvas nor
/// another tile's edge is taken for detail; zero outside them.
pub fn map(rgba: &[[u8; 4]], width: u32, height: u32, rects: &[Rect]) -> Vec<f32> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0.0f32; w * h];
    for &[x0, y0, x1, y1] in rects {
        let (x0, y0) = (x0 as usize, y0 as usize);
        let (x1, y1) = ((x1 as usize).min(w), (y1 as usize).min(h));
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let (rw, rh) = (x1 - x0, y1 - y0);
        let mut plane = Vec::with_capacity(rw * rh);
        for y in y0..y1 {
            plane.extend(
                rgba[y * w + x0..y * w + x1]
                    .iter()
                    .map(|&p| log_luminance(p)),
            );
        }
        let m = sharpness::of_plane(&plane, rw, rh, RADIUS);
        for (y, row) in m.chunks(rw).enumerate() {
            out[(y0 + y) * w + x0..(y0 + y) * w + x1].copy_from_slice(row);
        }
    }
    out
}

/// The first of `rects` holding a pixel, as the shader looks.
fn rect_of(rects: &[Rect], x: u32, y: u32) -> Option<usize> {
    rects
        .iter()
        .position(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
}

/// Which pixels of the target peaking paints: true where the map is
/// at or over its rectangle's threshold. Each rectangle (a compare
/// tile) has its own, so a noisy frame cannot set its neighbor's.
pub fn reference(rgba: &[[u8; 4]], width: u32, height: u32, rects: &[Rect]) -> Vec<bool> {
    let m = map(rgba, width, height, rects);
    let w = width as usize;
    let at = |i: usize| rect_of(rects, (i % w) as u32, (i / w) as u32);
    let mut counts = vec![vec![0u32; BINS]; rects.len()];
    for (i, &e) in m.iter().enumerate() {
        if let Some(k) = at(i) {
            counts[k][bin(e) as usize] += 1;
        }
    }
    let from: Vec<u32> = counts.iter().map(|c| threshold(c)).collect();
    m.iter()
        .enumerate()
        .map(|(i, &e)| at(i).is_some_and(|k| bin(e) >= from[k]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose left half is a fine checker and whose right half
    /// is the same checker smoothed out, grey on grey.
    fn halves(w: u32, h: u32) -> Vec<[u8; 4]> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let v = if x < w / 2 {
                    if (x / 2 + y / 2) % 2 == 0 { 60 } else { 190 }
                } else {
                    125
                };
                [v, v, v, 255]
            })
            .collect()
    }

    #[test]
    fn the_sharp_half_paints_and_the_flat_half_does_not() {
        let (w, h) = (96, 64);
        let marks = reference(&halves(w, h), w, h, &[[0, 0, w, h]]);
        let left = (0..h)
            .flat_map(|y| (4..w / 2 - 4).map(move |x| (y * w + x) as usize))
            .filter(|&i| marks[i])
            .count();
        let right = (0..h)
            .flat_map(|y| (w / 2 + 4..w).map(move |x| (y * w + x) as usize))
            .filter(|&i| marks[i])
            .count();
        assert!(left > (h * (w / 2 - 8)) as usize / 2, "{left}");
        assert_eq!(right, 0);
    }

    #[test]
    fn a_flat_picture_paints_nothing() {
        let (w, h) = (32, 32);
        let flat = vec![[100, 100, 100, 255]; (w * h) as usize];
        assert!(!reference(&flat, w, h, &[[0, 0, w, h]]).iter().any(|&m| m));
    }

    #[test]
    fn outside_the_rectangles_nothing_paints() {
        let (w, h) = (96, 64);
        let marks = reference(&halves(w, h), w, h, &[[10, 10, 40, 50]]);
        for (i, &m) in marks.iter().enumerate() {
            let (x, y) = (i as u32 % w, i as u32 / w);
            if m {
                assert!((10..40).contains(&x) && (10..50).contains(&y));
            }
        }
        assert!(marks.iter().any(|&m| m));
    }

    /// Two tiles, one the checker and one a soft ramp: each paints by
    /// its own top, so the soft one still paints its strongest part,
    /// which one count over both would leave bare.
    #[test]
    fn each_rectangle_has_its_own_threshold() {
        let (w, h) = (96u32, 64u32);
        let px: Vec<[u8; 4]> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let v = if x < w / 2 {
                    if (x / 2 + y / 2) % 2 == 0 { 60 } else { 190 }
                } else {
                    // A gentle wave: detail, far softer than the checker.
                    (125.0 + 6.0 * ((x as f32) * 0.7).sin()) as u8
                };
                [v, v, v, 255]
            })
            .collect();
        let soft = |rects: &[Rect]| {
            let marks = reference(&px, w, h, rects);
            (0..h)
                .flat_map(|y| (w / 2 + 4..w).map(move |x| (y * w + x) as usize))
                .filter(|&i| marks[i])
                .count()
        };
        assert!(soft(&[[0, 0, w / 2, h], [w / 2, 0, w, h]]) > 0);
        assert_eq!(soft(&[[0, 0, w, h]]), 0, "one count over both");
    }

    #[test]
    fn the_threshold_follows_the_frames_top() {
        let mut counts = vec![0u32; BINS];
        counts[0] = 500;
        counts[400] = 495;
        counts[500] = 5;
        assert_eq!(threshold(&counts), 400 - (BELOW * STEPS) as u32);
        // Ten times the energy everywhere, the same picture brighter
        // in its noise: the threshold moves with it.
        let mut louder = vec![0u32; BINS];
        louder[0] = 500;
        louder[453] = 495;
        louder[553] = 5;
        assert_eq!(threshold(&louder) - threshold(&counts), 53);
        assert_eq!(
            threshold(&vec![0u32; BINS]),
            BINS as u32 - 1 - (BELOW * STEPS) as u32
        );
    }

    /// Paints the PNGs of `GREYCARD_PEAKING_DIR` into `painted/` under
    /// it, for tuning [`QUANTILE`] and [`BELOW`] by eye.
    #[test]
    #[ignore = "a tuning aid over pictures on disk, not a test"]
    fn paint_a_folder() {
        let Some(dir) = std::env::var_os("GREYCARD_PEAKING_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let out = dir.join("painted");
        std::fs::create_dir_all(&out).unwrap();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "png") {
                continue;
            }
            let mut img = image::open(&path).unwrap().to_rgba8();
            let (w, h) = img.dimensions();
            let px: Vec<[u8; 4]> = img.pixels().map(|p| p.0).collect();
            let t = std::time::Instant::now();
            let marks = reference(&px, w, h, &[[1, 1, w - 1, h - 1]]);
            let spent = t.elapsed();
            let share = marks.iter().filter(|&&m| m).count() as f64 / marks.len() as f64;
            println!(
                "{}: {w}x{h}, {:.1}% painted, {:.0} ms",
                path.display(),
                share * 100.0,
                spent.as_secs_f64() * 1000.0
            );
            for (p, &m) in img.pixels_mut().zip(&marks) {
                if m {
                    p.0 = [
                        PAINT_FOR_TUNING[0],
                        PAINT_FOR_TUNING[1],
                        PAINT_FOR_TUNING[2],
                        255,
                    ];
                }
            }
            img.save(out.join(path.file_name().unwrap())).unwrap();
        }
    }

    const PAINT_FOR_TUNING: [u8; 3] = [60, 255, 40];
}
