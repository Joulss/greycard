//! A mask's edge refined against the picture: the guided filter of
//! Kaiming He, Jian Sun, and Xiaoou Tang ("Guided Image Filtering",
//! ECCV 2010, and its color form in the TPAMI 2013 version) with the
//! mask as the input and the picture's three channels as the guide,
//! and its fast form of Kaiming He and Jian Sun ("Fast Guided Filter",
//! 2015), the model made on a reduced copy. Where the picture has an
//! edge inside the window, the mask is made a local linear function of
//! the picture there, so its edge moves toward the picture's; where
//! the picture is flat, the mask is smoothed by the window, so a hard
//! edge through a clear sky becomes a ramp and two masks meeting there
//! meet without a seam.
//!
//! A color guide rather than the luminance alone: a red coat against
//! green leaves of the same lightness is an edge to the eye and none
//! to the luminance. The guide's channels should be perceptual (the
//! caller hands it Oklab), so that `eps` is one size of difference
//! everywhere on the scale.

use crate::guided::box_mean;
use rayon::prelude::*;

/// `mask` (`w` by `h`, 0 to 1) filtered against the three-channel
/// `guide` with a window of radius `r` texels and regularizer `eps`,
/// held to 0 to 1. A radius of nothing leaves the mask as it is.
///
/// The covariance of the guide in each window is a 3x3 matrix; `eps`
/// is added on its diagonal before it is inverted, so a window whose
/// guide varies by less than about `eps.sqrt()` in every channel reads
/// as flat and is smoothed, and one whose guide varies by more keeps
/// the mask's edge on the guide's.
pub fn refine_edge(
    guide: &[[f32; 3]],
    mask: &[f32],
    w: usize,
    h: usize,
    r: usize,
    eps: f32,
) -> Vec<f32> {
    let n = w * h;
    assert_eq!(guide.len(), n);
    assert_eq!(mask.len(), n);
    if r == 0 {
        return mask.to_vec();
    }
    let c = coefficients(guide, mask, w, h, r, eps);
    (0..n)
        .into_par_iter()
        .map(|i| {
            let (g, c) = (guide[i], c[i]);
            (c[0] * g[0] + c[1] * g[1] + c[2] * g[2] + c[3]).clamp(0.0, 1.0)
        })
        .collect()
}

/// The window means of the linear model's coefficients at every
/// texel, `[a_r, a_g, a_b, b]`: what the output is made from, as
/// `a · I + b` with the guide `I` there.
fn coefficients(
    guide: &[[f32; 3]],
    mask: &[f32],
    w: usize,
    h: usize,
    r: usize,
    eps: f32,
) -> Vec<[f32; 4]> {
    let n = w * h;
    let mean = |f: &(dyn Fn(usize) -> f32 + Sync)| {
        let plane: Vec<f32> = (0..n).into_par_iter().map(f).collect();
        box_mean(&plane, w, h, r)
    };
    let c = |k: usize| move |i: usize| guide[i][k];
    let (mr, mg, mb) = (mean(&c(0)), mean(&c(1)), mean(&c(2)));
    let mp = mean(&|i| mask[i]);
    let prod = |a: usize, b: usize| move |i: usize| guide[i][a] * guide[i][b];
    let (rr, rg, rb) = (mean(&prod(0, 0)), mean(&prod(0, 1)), mean(&prod(0, 2)));
    let (gg, gb, bb) = (mean(&prod(1, 1)), mean(&prod(1, 2)), mean(&prod(2, 2)));
    let with_p = |k: usize| move |i: usize| guide[i][k] * mask[i];
    let (rp, gp, bp) = (mean(&with_p(0)), mean(&with_p(1)), mean(&with_p(2)));
    let ab: Vec<[f32; 4]> = (0..n)
        .into_par_iter()
        .map(|i| {
            model(
                [mr[i], mg[i], mb[i]],
                mp[i],
                [rr[i], rg[i], rb[i], gg[i], gb[i], bb[i]],
                [rp[i], gp[i], bp[i]],
                eps,
            )
        })
        .collect();
    drop((mr, mg, mb, mp, rr, rg, rb, gg, gb, bb, rp, gp, bp));
    let part = |k: usize| {
        let plane: Vec<f32> = ab.par_iter().map(|v| v[k]).collect();
        box_mean(&plane, w, h, r)
    };
    let (ar, ag, ab_, b) = (part(0), part(1), part(2), part(3));
    (0..n)
        .into_par_iter()
        .map(|i| [ar[i], ag[i], ab_[i], b[i]])
        .collect()
}

/// [`refine_edge`] with its window's model made on a copy `s` times
/// smaller each way (He and Sun, "Fast Guided Filter", 2015): the
/// guide and the mask are averaged over blocks of `s` by `s`, the
/// coefficients found there with a radius of `r / s` blocks, and then
/// read back up bilinearly and put on the guide at its own size. The
/// coefficients are means over the window already, so they carry
/// nothing finer than it, and the copy loses little while the window
/// is a few blocks wide; the edge itself still comes from the guide at
/// its own size.
///
/// The windows are summed directly, a pass along the rows and one down
/// the columns, every channel the model wants in one pass: with `r / s`
/// a few blocks that is cheaper than running sums and keeps every
/// sum's order fixed, so the answer is one answer on every machine.
pub fn refine_edge_fast(
    guide: &[[f32; 3]],
    mask: &[f32],
    w: usize,
    h: usize,
    r: usize,
    eps: f32,
    s: usize,
) -> Vec<f32> {
    assert_eq!(guide.len(), w * h);
    assert_eq!(mask.len(), w * h);
    if r == 0 {
        return mask.to_vec();
    }
    let s = s.max(1);
    let (sw, sh) = (w.div_ceil(s), h.div_ceil(s));
    let rs = (r / s).max(1);
    // Block means of the guide and the mask, short blocks at the right
    // and the bottom by their own count, and the products the model
    // reads, of the means: [I, p, I_r I, I_g (g, b), I_b b, I p].
    let mut small = vec![[0.0f32; 13]; sw * sh];
    small.par_chunks_mut(sw).enumerate().for_each(|(by, out)| {
        let (y0, y1) = (by * s, ((by + 1) * s).min(h));
        for (bx, o) in out.iter_mut().enumerate() {
            let (x0, x1) = (bx * s, ((bx + 1) * s).min(w));
            let (mut g, mut m) = ([0.0f32; 3], 0.0f32);
            for y in y0..y1 {
                for i in y * w + x0..y * w + x1 {
                    g[0] += guide[i][0];
                    g[1] += guide[i][1];
                    g[2] += guide[i][2];
                    m += mask[i];
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as f32;
            let (g, p) = (g.map(|v| v / n), m / n);
            *o = [
                g[0],
                g[1],
                g[2],
                p,
                g[0] * g[0],
                g[0] * g[1],
                g[0] * g[2],
                g[1] * g[1],
                g[1] * g[2],
                g[2] * g[2],
                g[0] * p,
                g[1] * p,
                g[2] * p,
            ];
        }
    });
    let means = box_direct(&small, sw, sh, rs);
    drop(small);
    let model: Vec<[f32; 4]> = means
        .par_iter()
        .map(|v| {
            let [m0, m1, m2, p, rr, rg, rb, gg, gb, bb, rp, gp, bp] = *v;
            model([m0, m1, m2], p, [rr, rg, rb, gg, gb, bb], [rp, gp, bp], eps)
        })
        .collect();
    drop(means);
    let coefficients = box_direct(&model, sw, sh, rs);
    // Each texel's center on the blocks' grid, bilinear between block
    // centers, clamped at the edges.
    let tap = |v: usize, len: usize| {
        let c = ((v as f32 + 0.5) / s as f32 - 0.5).max(0.0);
        let i0 = (c.floor() as usize).min(len - 1);
        let i1 = (i0 + 1).min(len - 1);
        (i0, i1, c - i0 as f32)
    };
    let across: Vec<(usize, usize, f32)> = (0..w).map(|x| tap(x, sw)).collect();
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, line)| {
        let (y0, y1, fy) = tap(y, sh);
        let (top, bottom) = (&coefficients[y0 * sw..], &coefficients[y1 * sw..]);
        let row = &guide[y * w..];
        for (x, o) in line.iter_mut().enumerate() {
            let (x0, x1, fx) = across[x];
            let c: [f32; 4] = std::array::from_fn(|k| {
                let t = top[x0][k] + (top[x1][k] - top[x0][k]) * fx;
                let b = bottom[x0][k] + (bottom[x1][k] - bottom[x0][k]) * fx;
                t + (b - t) * fy
            });
            let g = row[x];
            *o = (c[0] * g[0] + c[1] * g[1] + c[2] * g[2] + c[3]).clamp(0.0, 1.0);
        }
    });
    out
}

/// One window's linear model from its means: the guide's `m`, the
/// mask's `p`, the guide's second moments `[rr, rg, rb, gg, gb, bb]`,
/// and its products with the mask `[rp, gp, bp]`. `[a_r, a_g, a_b,
/// b]` with a = Σ⁻¹ cov(I, p) and b = p̄ - a·Ī, by the adjugate: Σ is
/// symmetric and, with `eps` on its diagonal, positive definite.
fn model(m: [f32; 3], p: f32, second: [f32; 6], with_p: [f32; 3], eps: f32) -> [f32; 4] {
    let [rr, rg, rb, gg, gb, bb] = second;
    let s00 = rr - m[0] * m[0] + eps;
    let s01 = rg - m[0] * m[1];
    let s02 = rb - m[0] * m[2];
    let s11 = gg - m[1] * m[1] + eps;
    let s12 = gb - m[1] * m[2];
    let s22 = bb - m[2] * m[2] + eps;
    let cov = [
        with_p[0] - m[0] * p,
        with_p[1] - m[1] * p,
        with_p[2] - m[2] * p,
    ];
    let i00 = s11 * s22 - s12 * s12;
    let i01 = s02 * s12 - s01 * s22;
    let i02 = s01 * s12 - s02 * s11;
    let i11 = s00 * s22 - s02 * s02;
    let i12 = s01 * s02 - s00 * s12;
    let i22 = s00 * s11 - s01 * s01;
    let det = s00 * i00 + s01 * i01 + s02 * i02;
    let a = [
        (i00 * cov[0] + i01 * cov[1] + i02 * cov[2]) / det,
        (i01 * cov[0] + i11 * cov[1] + i12 * cov[2]) / det,
        (i02 * cov[0] + i12 * cov[1] + i22 * cov[2]) / det,
    ];
    [
        a[0],
        a[1],
        a[2],
        p - a[0] * m[0] - a[1] * m[1] - a[2] * m[2],
    ]
}

/// The mean over a (2r+1)² window clamped to the plane, every channel
/// at once, by direct sums along the rows and then down the columns,
/// as [`box_mean`] means it.
fn box_direct<const N: usize>(src: &[[f32; N]], w: usize, h: usize, r: usize) -> Vec<[f32; N]> {
    let mut rows = vec![[0.0f32; N]; w * h];
    rows.par_chunks_mut(w)
        .zip(src.par_chunks(w))
        .for_each(|(out, row)| {
            for (x, o) in out.iter_mut().enumerate() {
                let (a, b) = (x.saturating_sub(r), (x + r + 1).min(w));
                let mut acc = [0.0f32; N];
                for v in &row[a..b] {
                    for k in 0..N {
                        acc[k] += v[k];
                    }
                }
                let n = (b - a) as f32;
                *o = acc.map(|v| v / n);
            }
        });
    let mut out = vec![[0.0f32; N]; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, line)| {
        let (a, b) = (y.saturating_sub(r), (y + r + 1).min(h));
        let n = (b - a) as f32;
        for (x, o) in line.iter_mut().enumerate() {
            let mut acc = [0.0f32; N];
            for yy in a..b {
                let v = &rows[yy * w + x];
                for k in 0..N {
                    acc[k] += v[k];
                }
            }
            *o = acc.map(|v| v / n);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose boundary is soft, a ramp sixteen texels wide
    /// (a subject's defocused edge), under a mask whose edge is hard
    /// and in its middle: the refined mask takes the picture's ramp,
    /// following the picture across the boundary where the mask had
    /// jumped. And the same hard edge drawn four texels off the
    /// boundary moves its half-way point toward the picture's.
    #[test]
    fn a_hard_edge_follows_the_pictures_soft_boundary() {
        let (w, h) = (96usize, 24usize);
        let ramp = |x: usize| ((x as f32 + 0.5 - 40.0) / 16.0 + 0.5).clamp(0.0, 1.0);
        let guide: Vec<[f32; 3]> = (0..w * h)
            .map(|k| {
                let t = ramp(k % w);
                // Dark green to light red: an edge in every channel.
                [0.3 + 0.5 * t, -0.08 + 0.18 * t, 0.06 - 0.02 * t]
            })
            .collect();
        let step = |at: usize| -> Vec<f32> {
            (0..w * h)
                .map(|k| if k % w < at { 0.0 } else { 1.0 })
                .collect()
        };
        let out = refine_edge(&guide, &step(40), w, h, 12, 1e-4);
        let row = &out[12 * w..13 * w];
        // Within a tenth of the picture's ramp everywhere across it,
        // where the step was up to half off.
        for (x, v) in row.iter().enumerate().take(48).skip(32) {
            assert!(
                (v - ramp(x)).abs() < 0.1,
                "at {x}: {v} against the picture's {}",
                ramp(x)
            );
        }
        // Inside the picture's flat sides the mask settles on its ends.
        assert!(row[4] < 0.01 && row[90] > 0.99, "{} {}", row[4], row[90]);
        // A sharper boundary, four texels wide, and the step drawn
        // four texels short of it: its half-way point moves over.
        let sharp: Vec<[f32; 3]> = guide
            .iter()
            .enumerate()
            .map(|(k, _)| {
                let t = ((k % w) as f32 + 0.5 - 38.0) / 4.0;
                let t = t.clamp(0.0, 1.0);
                [0.3 + 0.5 * t, -0.08 + 0.18 * t, 0.06 - 0.02 * t]
            })
            .collect();
        let off = refine_edge(&sharp, &step(36), w, h, 12, 1e-4);
        let half = off[12 * w..13 * w].iter().position(|v| *v >= 0.5).unwrap();
        assert!((38..=40).contains(&half), "half way at {half}");
    }

    /// The filter is linear in the mask, and a constant comes through
    /// as itself: two masks drawn edge to edge, one the other's
    /// complement, are still each other's complement refined, so where
    /// they meet nothing is left out and nothing is taken twice.
    #[test]
    fn two_masks_edge_to_edge_still_meet_without_a_seam() {
        let (w, h) = (64usize, 32usize);
        let guide: Vec<[f32; 3]> = (0..w * h)
            .map(|k| {
                let (x, y) = ((k % w) as f32, (k / w) as f32);
                [
                    0.4 + 0.3 * (x / 9.0).sin() * (y / 7.0).cos(),
                    0.05 * (x / 5.0).cos(),
                    0.02,
                ]
            })
            .collect();
        let left: Vec<f32> = (0..w * h)
            .map(|k| if k % w < 30 { 1.0 } else { 0.0 })
            .collect();
        let right: Vec<f32> = left.iter().map(|v| 1.0 - v).collect();
        let (a, b) = (
            refine_edge(&guide, &left, w, h, 6, 1e-4),
            refine_edge(&guide, &right, w, h, 6, 1e-4),
        );
        let worst = a
            .iter()
            .zip(&b)
            .map(|(a, b)| (a + b - 1.0).abs())
            .fold(0.0, f32::max);
        assert!(worst < 1e-3, "{worst}");
    }

    /// Where the picture is flat the hard edge is smoothed into a ramp
    /// about the window wide, centered where it was: a seam between
    /// two masks drawn edge to edge is gone.
    #[test]
    fn on_a_flat_picture_the_edge_becomes_a_ramp() {
        let (w, h) = (96usize, 16usize);
        let guide = vec![[0.6f32, 0.01, 0.02]; w * h];
        let mask: Vec<f32> = (0..w * h)
            .map(|k| if k % w < 48 { 0.0 } else { 1.0 })
            .collect();
        let out = refine_edge(&guide, &mask, w, h, 8, 1e-4);
        let row = &out[8 * w..9 * w];
        assert!((row[48] - 0.5).abs() < 0.1 && (row[47] - 0.5).abs() < 0.1);
        assert!(row[44] > 0.1 && row[52] < 0.9, "{} {}", row[44], row[52]);
        assert!(row[30] < 1e-4 && row[70] > 1.0 - 1e-4);
    }

    /// An edge in hue alone, the lightness the same either side, is
    /// still an edge to the color guide.
    #[test]
    fn an_edge_in_hue_alone_holds_the_mask() {
        let (w, h) = (96usize, 16usize);
        let guide: Vec<[f32; 3]> = (0..w * h)
            .map(|k| {
                if k % w < 48 {
                    [0.6, -0.1, 0.08]
                } else {
                    [0.6, 0.12, 0.04]
                }
            })
            .collect();
        let mask: Vec<f32> = (0..w * h)
            .map(|k| if k % w < 48 { 0.0 } else { 1.0 })
            .collect();
        let out = refine_edge(&guide, &mask, w, h, 8, 1e-4);
        let row = &out[8 * w..9 * w];
        assert!(row[46] < 0.02 && row[49] > 0.98, "{} {}", row[46], row[49]);
    }

    #[test]
    fn no_radius_is_no_change() {
        let mask: Vec<f32> = (0..64).map(|k| (k % 7) as f32 / 6.0).collect();
        let guide = vec![[0.5f32, 0.0, 0.0]; 64];
        assert_eq!(refine_edge(&guide, &mask, 8, 8, 0, 1e-4), mask);
    }

    /// The fast form at four texels a block is the reference's to a
    /// few hundredths: a disc in a textured picture, and a mask that is
    /// a slightly larger disc with a hard edge.
    #[test]
    fn the_fast_filter_is_the_references_within_a_few_hundredths() {
        let (w, h) = (256usize, 192usize);
        let inside = |x: f32, y: f32, r: f32| ((x - 128.0).powi(2) + (y - 96.0).powi(2)).sqrt() < r;
        let guide: Vec<[f32; 3]> = (0..w * h)
            .map(|k| {
                let (x, y) = ((k % w) as f32 + 0.5, (k / w) as f32 + 0.5);
                let grain = 0.01 * ((x * 1.7).sin() + (y * 2.3).cos());
                if inside(x, y, 50.0) {
                    [0.75 + grain, 0.06, 0.03]
                } else {
                    [0.35 + grain, -0.04, -0.06]
                }
            })
            .collect();
        let mask: Vec<f32> = (0..w * h)
            .map(|k| inside((k % w) as f32 + 0.5, (k / w) as f32 + 0.5, 56.0) as u8 as f32)
            .collect();
        let exact = refine_edge(&guide, &mask, w, h, 16, 4e-4);
        let fast = refine_edge_fast(&guide, &mask, w, h, 16, 4e-4, 4);
        let worst = exact
            .iter()
            .zip(&fast)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        let mean = exact
            .iter()
            .zip(&fast)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / (w * h) as f32;
        assert!(worst < 0.08 && mean < 0.01, "worst {worst}, mean {mean}");
        // And both moved the mask's edge in toward the disc's: three
        // texels outside the disc, where the mask was whole, it is
        // under half.
        let row = 96 * w;
        for refined in [&exact, &fast] {
            assert!(refined[row + 128 + 53] < 0.5, "{}", refined[row + 128 + 53]);
            assert!(refined[row + 128 + 46] > 0.9, "{}", refined[row + 128 + 46]);
        }
    }
}
