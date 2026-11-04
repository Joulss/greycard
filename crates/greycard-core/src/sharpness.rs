//! How sharp a picture is at each pixel: the local energy of the
//! Laplacian of its log luminance, smoothed. The focus stacker weighs
//! its frames by it ([`crate::stack`]); the viewport's focus peaking
//! paints where it is high.
//!
//! On the log, so a sharp edge in the shadows counts as much as one in
//! the light, and the picture's own exposure cannot tilt it.

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
}
