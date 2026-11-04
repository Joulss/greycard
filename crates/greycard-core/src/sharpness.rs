//! How sharp a picture is at each pixel: the local energy of the
//! Laplacian of its log luminance, smoothed. The focus stacker weighs
//! its frames by it ([`crate::stack`]); the viewport's focus peaking
//! paints where it is high.
//!
//! On the log, so a sharp edge in the shadows counts as much as one in
//! the light, and the picture's own exposure cannot tilt it.
//!
//! [`focus_score`] makes one number of a frame from the same map: how
//! sharp its sharpest part is, comparable across the frames of a burst
//! and, roughly, of a shoot.

use rayon::prelude::*;

use crate::register;

/// The local energy of the Laplacian of the log luminance, smoothed
/// over a `2r+1` square: the focus stacker's map. The floor under the
/// log is `floor_stops` under the plane's median
/// ([`crate::register`]'s).
pub fn map(lum: &[f32], width: usize, height: usize, floor_stops: f32, radius: usize) -> Vec<f32> {
    let log = register::log_plane(lum, floor_stops);
    of_plane(&log, width, height, radius)
}

/// The same of a plane already in the measure wanted: the energy of
/// its Laplacian, smoothed.
pub fn of_plane(plane: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let energy = laplacian_energy(plane, width, height);
    box_blur(&energy, width, height, radius)
}

/// The square of the four-neighbor Laplacian, the taps clamped at the
/// border.
pub fn laplacian_energy(plane: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; width * height];
    out.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let mid = &plane[y * width..(y + 1) * width];
        let up = &plane[y.saturating_sub(1) * width..][..width];
        let down = &plane[(y + 1).min(height - 1) * width..][..width];
        for (x, o) in row.iter_mut().enumerate() {
            let left = mid[x.saturating_sub(1)];
            let right = mid[(x + 1).min(width - 1)];
            let lap = 4.0 * mid[x] - left - right - up[x] - down[x];
            *o = lap * lap;
        }
    });
    out
}

/// The mean over a `2r+1` square, the window cut down at the border
/// rather than padded, by running sums. The vertical pass is the
/// horizontal one over a transposed copy: two blocked transposes cost
/// less than a strided traversal of a 24-megapixel plane.
pub fn box_blur(src: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    if radius == 0 {
        return src.to_vec();
    }
    let mut rows = vec![0.0f32; width * height];
    rows.par_chunks_mut(width)
        .zip(src.par_chunks(width))
        .for_each(|(out, row)| run_1d(row, out, radius));
    let turned = transpose(&rows, width, height);
    let mut blurred = vec![0.0f32; width * height];
    blurred
        .par_chunks_mut(height)
        .zip(turned.par_chunks(height))
        .for_each(|(out, row)| run_1d(row, out, radius));
    transpose(&blurred, height, width)
}

/// One line's running mean.
fn run_1d(src: &[f32], out: &mut [f32], radius: usize) {
    let n = src.len();
    let mut sum = 0.0f64;
    let mut hi = radius.min(n - 1);
    for v in src.iter().take(hi + 1) {
        sum += *v as f64;
    }
    let mut lo = 0usize;
    for (x, o) in out.iter_mut().enumerate() {
        let want_hi = (x + radius).min(n - 1);
        let want_lo = x.saturating_sub(radius);
        while hi < want_hi {
            hi += 1;
            sum += src[hi] as f64;
        }
        while lo < want_lo {
            sum -= src[lo] as f64;
            lo += 1;
        }
        *o = (sum / (want_hi - want_lo + 1) as f64) as f32;
    }
}

/// `src` turned on its side: the result is `height` wide and `width`
/// tall. In blocks, so neither side of the copy runs down a column.
fn transpose(src: &[f32], width: usize, height: usize) -> Vec<f32> {
    const BLOCK: usize = 64;
    let mut out = vec![0.0f32; width * height];
    out.par_chunks_mut(height * BLOCK)
        .enumerate()
        .for_each(|(block_index, block)| {
            let x0 = block_index * BLOCK;
            let columns = block.len() / height;
            for y0 in (0..height).step_by(BLOCK) {
                for y in y0..(y0 + BLOCK).min(height) {
                    let row = &src[y * width..];
                    for i in 0..columns {
                        block[i * height + y] = row[x0 + i];
                    }
                }
            }
        });
    out
}

/// The longest edge a focus score is measured at: the preview box
/// downscaled by a whole factor to no more than this, so a camera's
/// full-size preview and another's small one are of about one scale,
/// and a high ISO's grain is averaged down before the Laplacian.
pub const SCORE_EDGE: usize = 2048;

/// The share of the map at or under the level a score reads: the
/// peaking threshold's own quantile.
pub const SCORE_QUANTILE: f32 = 0.995;

/// The light added under the log, in stops: the peaking measure's, so
/// a dark ground's eight-bit steps are not taken for detail.
pub const SCORE_SHADOW: f32 = -5.0;

/// The smoothing's radius at [`SCORE_EDGE`]: the peaking's 5 by 5.
pub const SCORE_RADIUS: usize = 2;

/// The score's scale: the log2 energy read, [`SCORE_LOW`] to
/// [`SCORE_LOW`] plus [`SCORE_SPAN`], put on nought to one, so a filter
/// reads `focus>0.5`. The samples read -1.1 to 4.6, but for one
/// frame with nothing sharp in it, and a preview of 1024 blurred by a
/// box of 4 near -8.
pub const SCORE_LOW: f32 = -8.0;
pub const SCORE_SPAN: f32 = 16.0;

/// A frame's focus score from its preview, sRGB eight bits a channel,
/// row by row: the log2 of the sharpness map's [`SCORE_QUANTILE`],
/// the map taken on the log luminance at [`SCORE_EDGE`], put on a
/// scale of nought to one ([`SCORE_LOW`]). A sharper frame has more
/// of its pixels at a high energy, so its top is higher; the log
/// removes the exposure's gain, and the fixed edge the preview's size.
/// `None` for a picture too small to have a Laplacian.
pub fn focus_score(rgb: &[u8], width: usize, height: usize) -> Option<f32> {
    let level = focus_level(rgb, width, height)?;
    Some(((level - SCORE_LOW) / SCORE_SPAN).clamp(0.0, 1.0))
}

/// [`focus_score`] before its scale: the log2 energy itself, a flat
/// picture's at the bottom of `f32`.
pub fn focus_level(rgb: &[u8], width: usize, height: usize) -> Option<f32> {
    level_at(rgb, width, height, SCORE_EDGE)
}

/// [`focus_level`] measured at a long edge of no more than `edge`.
fn level_at(rgb: &[u8], width: usize, height: usize, edge: usize) -> Option<f32> {
    if rgb.len() < width * height * 3 {
        return None;
    }
    let f = width.max(height).div_ceil(edge).max(1);
    let (w, h) = (width / f, height / f);
    if w < 3 || h < 3 {
        return None;
    }
    let linear: Vec<f32> = (0..=255u8)
        .map(|v| crate::color::srgb_decode(f32::from(v) / 255.0))
        .collect();
    let floor = SCORE_SHADOW.exp2();
    let n = (f * f) as f32;
    let mut log = vec![0.0f32; w * h];
    log.par_chunks_mut(w).enumerate().for_each(|(ty, out)| {
        for y in ty * f..(ty + 1) * f {
            let row = &rgb[y * width * 3..][..w * f * 3];
            for (x, px) in row.as_chunks::<3>().0.iter().enumerate() {
                let [r, g, b] = [0, 1, 2].map(|c| linear[px[c] as usize]);
                out[x / f] += 0.2126 * r + 0.7152 * g + 0.0722 * b;
            }
        }
        out.iter_mut().for_each(|v| *v = (*v / n + floor).log2());
    });
    let mut map = of_plane(&log, w, h, SCORE_RADIUS);
    let k = ((map.len() - 1) as f32 * SCORE_QUANTILE).round() as usize;
    let (_, top, _) = map.select_nth_unstable_by(k, f32::total_cmp);
    Some(if *top > 0.0 { top.log2() } else { f32::MIN })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A step edge, and the same edge after a few passes of a box blur.
    fn edges(w: usize, h: usize) -> (Vec<f32>, Vec<f32>) {
        let sharp: Vec<f32> = (0..w * h)
            .map(|i| if i % w < w / 2 { 0.05 } else { 0.5 })
            .collect();
        let mut soft = sharp.clone();
        for _ in 0..3 {
            soft = box_blur(&soft, w, h, 3);
        }
        (sharp, soft)
    }

    #[test]
    fn a_sharp_edge_scores_above_its_blurred_copy() {
        let (w, h) = (64, 32);
        let (sharp, soft) = edges(w, h);
        let a = map(&sharp, w, h, 14.0, 2);
        let b = map(&soft, w, h, 14.0, 2);
        let peak = |m: &[f32]| m.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            peak(&a) > 4.0 * peak(&b),
            "{} against {}",
            peak(&a),
            peak(&b)
        );
        // Away from the edge, flat: nothing either way.
        assert_eq!(a[h / 2 * w + 4], 0.0);
    }

    #[test]
    fn a_box_blur_keeps_a_constant_and_its_sum_away_from_the_border() {
        let (w, h) = (40, 30);
        let flat = vec![0.25f32; w * h];
        assert!(
            box_blur(&flat, w, h, 3)
                .iter()
                .all(|v| (v - 0.25).abs() < 1e-6)
        );
        let mut spike = vec![0.0f32; w * h];
        spike[15 * w + 20] = 49.0;
        let spread = box_blur(&spike, w, h, 3);
        assert!((spread[15 * w + 20] - 1.0).abs() < 1e-5);
        assert!((spread.iter().sum::<f32>() - 49.0).abs() < 1e-3);
    }

    /// A picture with detail at every scale, rings and a checker, as a
    /// preview holds it.
    fn scene(w: usize, h: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32, y as f32);
                let r = ((fx - w as f32 * 0.4).hypot(fy - h as f32 * 0.5) * 0.15).sin();
                let check = if (x / 8 + y / 8) % 2 == 0 {
                    40.0
                } else {
                    -40.0
                };
                let v = (128.0 + 70.0 * r + check).clamp(0.0, 255.0);
                out.extend([v as u8, (v - 10.0).max(0.0) as u8, (v / 2.0 + 60.0) as u8]);
            }
        }
        out
    }

    /// The scene blurred by a box of `r` twice, a channel at a time.
    fn soften(rgb: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
        let mut out = rgb.to_vec();
        for c in 0..3 {
            let mut plane: Vec<f32> = rgb.iter().skip(c).step_by(3).map(|&v| v as f32).collect();
            for _ in 0..2 {
                plane = box_blur(&plane, w, h, r);
            }
            for (i, v) in plane.iter().enumerate() {
                out[i * 3 + c] = v.round() as u8;
            }
        }
        out
    }

    #[test]
    fn a_frame_scores_by_its_focus_whatever_its_size_and_exposure() {
        let (w, h) = (1200, 800);
        let sharp = scene(w, h);
        let score = |rgb: &[u8], w, h| focus_score(rgb, w, h).unwrap();
        let s = score(&sharp, w, h);
        let blurs: Vec<f32> = [1, 2, 4]
            .iter()
            .map(|&r| score(&soften(&sharp, w, h, r), w, h))
            .collect();
        assert!(
            s > blurs[0] && blurs[0] > blurs[1] && blurs[1] > blurs[2],
            "{s} {blurs:?}"
        );
        assert!(s - blurs[1] > 0.1, "{s} against {}", blurs[1]);
        // Darker by a stop or so: much the same score.
        let dark: Vec<u8> = sharp
            .iter()
            .map(|&v| {
                (crate::color::srgb_encode(crate::color::srgb_decode(v as f32 / 255.0) * 0.5)
                    * 255.0)
                    .round() as u8
            })
            .collect();
        assert!(
            (score(&dark, w, h) - s).abs() < 0.05,
            "{} against {s}",
            score(&dark, w, h)
        );
        // The same frame stored at twice the size: brought down to the
        // one scale, the one score.
        let (w, h) = (1000, 640);
        let small = scene(w, h);
        let mut big = Vec::with_capacity(small.len() * 4);
        for y in 0..2 * h {
            for x in 0..2 * w {
                big.extend_from_slice(&small[((y / 2) * w + x / 2) * 3..][..3]);
            }
        }
        let (a, b) = (score(&small, w, h), score(&big, 2 * w, 2 * h));
        assert!((a - b).abs() < 0.01, "{a} against {b}");
        // Nothing to measure: flat scores nought, tiny scores none.
        assert_eq!(focus_score(&vec![90u8; 64 * 48 * 3], 64, 48), Some(0.0));
        assert_eq!(focus_score(&[0u8; 12], 2, 2), None);
        assert_eq!(focus_score(&[0u8; 5], 4, 4), None);
    }

    /// Each frame's level in a folder, from its preview, and its
    /// preview blurred: for setting the score's scale on real frames.
    /// `GREYCARD_FOCUS=<folder> cargo test -p greycard-core --release
    /// measure_focus -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_focus() {
        let Ok(dir) = std::env::var("GREYCARD_FOCUS") else {
            return;
        };
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect();
        paths.sort();
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let started = std::time::Instant::now();
            let rgb = if crate::picture::is_picture_path(&path) {
                match image::open(&path) {
                    Ok(i) => i.to_rgb8(),
                    Err(_) => continue,
                }
            } else {
                match crate::decode::preview_path(&path) {
                    Ok(Some((p, _))) => p,
                    _ => continue,
                }
            };
            let read = started.elapsed();
            let (w, h) = (rgb.width() as usize, rgb.height() as usize);
            let started = std::time::Instant::now();
            let level = focus_level(rgb.as_raw(), w, h).unwrap();
            let took = started.elapsed();
            let blurred: Vec<String> = [1, 2, 4]
                .iter()
                .map(|&r| {
                    let soft = soften(rgb.as_raw(), w, h, r);
                    let at = |edge| level_at(&soft, w, h, edge).unwrap();
                    format!("{:.2}/{:.2}", at(SCORE_EDGE / 2), at(SCORE_EDGE))
                })
                .collect();
            let half = level_at(rgb.as_raw(), w, h, SCORE_EDGE / 2).unwrap();
            println!(
                "{name}: {w}x{h} level {half:.2}/{level:.2} score {:.3} blurred {} (read {:.0} ms, score {:.1} ms)",
                focus_score(rgb.as_raw(), w, h).unwrap(),
                blurred.join(" "),
                read.as_secs_f64() * 1e3,
                took.as_secs_f64() * 1e3
            );
        }
    }
}
