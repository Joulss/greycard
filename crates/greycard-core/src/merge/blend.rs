//! A panorama's frames blended band by band (Burt and Adelson, "A
//! Multiresolution Spline With Application to Image Mosaics", ACM ToG
//! 2(4), 1983): each frame's Laplacian pyramid weighted by the Gaussian
//! pyramid of its mask, summed into one pyramid of the canvas, and the
//! sum collapsed. A seam is then a hard edge only in the finest band;
//! the coarser a band, the farther the frames cross-fade, so a shade of
//! difference in brightness spreads over the canvas rather than
//! stepping at the seam.
//!
//! The canvas's pyramid is the only thing that grows: a frame is added
//! and let go, so the memory is the canvas's bands (a third over the
//! canvas, in RGB and a weight) and one frame's pyramid, whatever the
//! count. The pyramid is `stack.rs`'s: the same binomial reduce and
//! expand. Nothing ported.

use rayon::prelude::*;

use crate::stack::{expand, reduce};

/// The canvas's bands as they are summed.
pub struct Bands {
    sizes: Vec<(usize, usize)>,
    /// Each level's weighted sum, interleaved RGB.
    sum: Vec<Vec<f32>>,
    /// Each level's summed weight.
    weight: Vec<Vec<f32>>,
}

/// The sizes of a pyramid of `levels` over `width` by `height`.
pub fn sizes(width: usize, height: usize, levels: usize) -> Vec<(usize, usize)> {
    let mut out = vec![(width, height)];
    for _ in 1..levels.max(1) {
        let (w, h) = *out.last().expect("a base");
        out.push((w.div_ceil(2), h.div_ceil(2)));
    }
    out
}

/// The levels a canvas of `width` by `height` is blended over: the
/// coarsest band about sixteen pixels on its shorter side, and no more
/// than eight.
pub fn levels_for(width: usize, height: usize) -> usize {
    let short = width.min(height).max(1);
    let n = (usize::BITS - short.leading_zeros()) as usize;
    n.saturating_sub(4).clamp(1, 8)
}

/// The Laplacian pyramid of interleaved `base` over `sizes`, the
/// coarsest level its Gaussian.
fn laplacian(base: Vec<f32>, sizes: &[(usize, usize)], channels: usize) -> Vec<Vec<f32>> {
    let mut g = vec![base];
    for k in 1..sizes.len() {
        let (w, h) = sizes[k - 1];
        let next = reduce(g.last().expect("a base"), w, h, channels);
        g.push(next);
    }
    for k in 0..sizes.len() - 1 {
        let (w, h) = sizes[k];
        let (cw, ch) = sizes[k + 1];
        let up = expand(&g[k + 1], cw, ch, w, h, channels);
        g[k].par_iter_mut()
            .zip(up.par_iter())
            .for_each(|(a, b)| *a -= b);
    }
    g
}

impl Bands {
    /// Bands for a canvas `width` by `height`, of `levels`.
    pub fn new(width: usize, height: usize, levels: usize) -> Self {
        let sizes = sizes(width, height, levels);
        Self {
            sum: sizes.iter().map(|(w, h)| vec![0.0; w * h * 3]).collect(),
            weight: sizes.iter().map(|(w, h)| vec![0.0; w * h]).collect(),
            sizes,
        }
    }

    pub fn levels(&self) -> usize {
        self.sizes.len()
    }

    /// What a frame's place on the canvas must be a multiple of, so its
    /// levels fall on the canvas's.
    pub fn alignment(&self) -> usize {
        1 << (self.levels() - 1)
    }

    /// The bytes the bands hold.
    pub fn bytes(&self) -> usize {
        self.sum
            .iter()
            .chain(&self.weight)
            .map(|v| v.len() * 4)
            .sum()
    }

    /// A frame added: `rgb` (interleaved, every sample a number) and its
    /// weight `mask`, `width` by `height`, its corner at `(x, y)` of the
    /// canvas, both multiples of [`Bands::alignment`]. What reaches past
    /// the canvas is left out.
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        rgb: Vec<f32>,
        mask: Vec<f32>,
    ) {
        let a = self.alignment();
        assert!(
            x.is_multiple_of(a) && y.is_multiple_of(a),
            "a frame's corner off the levels"
        );
        assert_eq!(rgb.len(), width * height * 3);
        assert_eq!(mask.len(), width * height);
        let sizes = sizes(width, height, self.levels());
        let bands = laplacian(rgb, &sizes, 3);
        let mut weights = vec![mask];
        for k in 1..sizes.len() {
            let (w, h) = sizes[k - 1];
            let next = reduce(weights.last().expect("a base"), w, h, 1);
            weights.push(next);
        }
        for (l, ((band, weight), &(tw, th))) in bands.iter().zip(&weights).zip(&sizes).enumerate() {
            let (ox, oy) = (x >> l, y >> l);
            let (cw, ch) = self.sizes[l];
            if ox >= cw || oy >= ch {
                continue;
            }
            let span = tw.min(cw - ox);
            let rows = th.min(ch - oy);
            let sum = &mut self.sum[l];
            let total = &mut self.weight[l];
            sum.par_chunks_mut(cw * 3)
                .zip(total.par_chunks_mut(cw))
                .skip(oy)
                .take(rows)
                .enumerate()
                .for_each(|(j, (srow, wrow))| {
                    for i in 0..span {
                        let g = weight[j * tw + i];
                        if g <= 0.0 {
                            continue;
                        }
                        wrow[ox + i] += g;
                        for c in 0..3 {
                            srow[(ox + i) * 3 + c] += g * band[(j * tw + i) * 3 + c];
                        }
                    }
                });
        }
    }

    /// The blend: each band its weighted mean, the bands collapsed, the
    /// canvas's interleaved RGB.
    pub fn finish(self) -> Vec<f32> {
        let Bands {
            sizes,
            mut sum,
            weight,
        } = self;
        for (s, w) in sum.iter_mut().zip(&weight) {
            s.par_chunks_mut(3).zip(w.par_iter()).for_each(|(p, &w)| {
                if w > 1e-8 {
                    p.iter_mut().for_each(|v| *v /= w);
                } else {
                    p.fill(0.0);
                }
            });
        }
        let top = sizes.len() - 1;
        let mut image = sum.pop().expect("a level");
        for l in (0..top).rev() {
            let (cw, ch) = sizes[l + 1];
            let (w, h) = sizes[l];
            let mut up = expand(&image, cw, ch, w, h, 3);
            let band = sum.pop().expect("a level");
            up.par_iter_mut()
                .zip(band.par_iter())
                .for_each(|(u, b)| *u += b);
            image = up;
        }
        image
    }
}

/// The samples of `rgb` (interleaved, `width` by `height`) that
/// `known` does not hold filled from what it does, pulled up a pyramid
/// of averages and pushed back down, so a band taken of the frame does
/// not see an edge where the frame ends.
pub fn fill(rgb: &mut [f32], known: &[bool], width: usize, height: usize) {
    if known.iter().all(|k| *k) {
        return;
    }
    let mut sizes = vec![(width, height)];
    let mut sums: Vec<Vec<f32>> = vec![
        rgb.par_chunks(3)
            .zip(known.par_iter())
            .flat_map_iter(|(p, &k)| {
                let s = if k { 1.0 } else { 0.0 };
                p.iter()
                    .map(move |v| if k && v.is_finite() { v * s } else { 0.0 })
                    .collect::<Vec<_>>()
            })
            .collect(),
    ];
    let mut weights: Vec<Vec<f32>> = vec![known.iter().map(|&k| f32::from(u8::from(k))).collect()];
    while let Some(&(w, h)) = sizes.last() {
        if (w <= 1 && h <= 1) || weights.last().expect("a level").iter().all(|v| *v > 0.0) {
            break;
        }
        let s = reduce(sums.last().expect("a level"), w, h, 3);
        let g = reduce(weights.last().expect("a level"), w, h, 1);
        sizes.push((w.div_ceil(2), h.div_ceil(2)));
        sums.push(s);
        weights.push(g);
    }
    let top = sizes.len() - 1;
    let mean = |s: &[f32], g: &[f32]| -> Vec<f32> {
        s.par_chunks(3)
            .zip(g.par_iter())
            .flat_map_iter(|(p, &g)| {
                let k = if g > 1e-6 { 1.0 / g } else { 0.0 };
                [p[0] * k, p[1] * k, p[2] * k]
            })
            .collect()
    };
    let mut filled = mean(&sums[top], &weights[top]);
    for l in (0..top).rev() {
        let (cw, ch) = sizes[l + 1];
        let (w, h) = sizes[l];
        let up = expand(&filled, cw, ch, w, h, 3);
        let own = mean(&sums[l], &weights[l]);
        filled = own
            .par_chunks(3)
            .zip(up.par_chunks(3))
            .zip(weights[l].par_iter())
            .flat_map_iter(|((o, u), &g)| {
                if g > 1e-6 {
                    [o[0], o[1], o[2]]
                } else {
                    [u[0], u[1], u[2]]
                }
            })
            .collect();
    }
    rgb.par_chunks_mut(3)
        .zip(known.par_iter())
        .zip(filled.par_chunks(3))
        .for_each(|((p, &k), f)| {
            if !k {
                p.copy_from_slice(f);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A smooth scene with fine detail, over a canvas `w` by `h`.
    fn scene(x: usize, y: usize, c: usize) -> f32 {
        let (x, y) = (x as f32, y as f32);
        0.3 + 0.1 * (x * 0.05).sin()
            + 0.05 * (y * 0.3 + c as f32).cos()
            + 0.02 * ((x + 2.0 * y) * 0.9).sin()
    }

    /// The blend of frames that agree is the scene, however many frames
    /// it is cut into and in whatever order they come: the sums do not
    /// depend on the count.
    #[test]
    fn the_blend_does_not_depend_on_the_count() {
        let (w, h) = (256, 128);
        let levels = 5;
        let whole: Vec<f32> = (0..w * h)
            .flat_map(|i| (0..3).map(move |c| scene(i % w, i / w, c)))
            .collect();
        // Cut into `n` frames side by side, each overlapping the next by
        // 32 columns, a seam down each overlap's middle; each frame a
        // tile at a multiple of the alignment.
        let blend = |n: usize, reversed: bool| -> Vec<f32> {
            let mut bands = Bands::new(w, h, levels);
            let a = bands.alignment();
            let step = w / n;
            let mut order: Vec<usize> = (0..n).collect();
            if reversed {
                order.reverse();
            }
            for k in order {
                let x0 = (k * step).saturating_sub(64) / a * a;
                let x1 = ((k + 1) * step + 64).min(w);
                let tw = x1 - x0;
                let mut rgb = vec![0.0f32; tw * h * 3];
                let mut mask = vec![0.0f32; tw * h];
                for y in 0..h {
                    for i in 0..tw {
                        let x = x0 + i;
                        for c in 0..3 {
                            rgb[(y * tw + i) * 3 + c] = whole[(y * w + x) * 3 + c];
                        }
                        mask[y * tw + i] = f32::from(u8::from(x >= k * step && x < (k + 1) * step));
                    }
                }
                bands.add(x0, 0, tw, h, rgb, mask);
            }
            bands.finish()
        };
        let worst = |a: &[f32], b: &[f32]| {
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max)
        };
        let one = blend(1, false);
        assert!(worst(&one, &whole) < 1e-4, "{}", worst(&one, &whole));
        for n in [2, 4, 8] {
            let many = blend(n, false);
            assert!(worst(&many, &whole) < 2e-3, "{n}: {}", worst(&many, &whole));
            assert!(worst(&many, &blend(n, true)) < 1e-5);
        }
    }

    /// Two frames a shade apart in brightness: the step at the seam is
    /// spread over the coarse bands, not left at the seam.
    #[test]
    fn a_step_in_brightness_is_spread() {
        let (w, h) = (256, 64);
        let mut bands = Bands::new(w, h, 6);
        for (k, level) in [(0usize, 0.5f32), (1, 0.55)] {
            let rgb = vec![level; w * h * 3];
            let mask: Vec<f32> = (0..w * h)
                .map(|i| f32::from(u8::from(((i % w) < w / 2) == (k == 0))))
                .collect();
            bands.add(0, 0, w, h, rgb, mask);
        }
        let out = bands.finish();
        let at = |x: usize| out[(32 * w + x) * 3];
        // No step of more than a tenth of the difference between two
        // neighbors.
        let biggest = (1..w)
            .map(|x| (at(x) - at(x - 1)).abs())
            .fold(0.0f32, f32::max);
        assert!(biggest < 0.005, "{biggest}");
        assert!((at(2) - 0.5).abs() < 0.01 && (at(w - 3) - 0.55).abs() < 0.01);
    }

    #[test]
    fn a_fill_has_no_edge() {
        let (w, h) = (64, 32);
        let mut rgb: Vec<f32> = (0..w * h).flat_map(|_| [0.4, 0.5, 0.6]).collect();
        let known: Vec<bool> = (0..w * h).map(|i| i % w < 40).collect();
        for (p, k) in rgb.chunks_mut(3).zip(&known) {
            if !k {
                p.fill(f32::NAN);
            }
        }
        fill(&mut rgb, &known, w, h);
        assert!(
            rgb.chunks(3)
                .all(|p| (p[0] - 0.4).abs() < 1e-5 && (p[2] - 0.6).abs() < 1e-5)
        );
    }
}
