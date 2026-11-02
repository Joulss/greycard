//! Where two frames of a panorama meet, from their pictures alone:
//! Harris corners on the log luminance, a normalized patch about each,
//! the mutual nearest neighbors that pass Lowe's ratio test, and a
//! homography fitted to them by RANSAC.
//!
//! Written from Harris and Stephens, "A Combined Corner and Edge
//! Detector" (Alvey Vision Conference, 1988); Lowe, "Distinctive Image
//! Features from Scale-Invariant Keypoints" (IJCV 60(2), 2004), for the
//! ratio test; Fischler and Bolles, "Random Sample Consensus" (CACM
//! 24(6), 1981); and Hartley, "In Defense of the Eight-Point Algorithm"
//! (PAMI 19(6), 1997), for the normalized linear fit. Nothing ported.
//!
//! The plane is the log of the luminance, so an exposure step between
//! two frames is an offset, which each descriptor's mean takes off.
//! Coordinates are continuous: a pixel's center sits at its index plus
//! a half, so a plane at a quarter of the size is the same coordinates
//! divided by four.

use rayon::prelude::*;

use crate::register::Homography;

/// The side of the square of samples a descriptor holds.
const PATCH: usize = 8;
/// The spacing of a descriptor's samples, in the plane's pixels.
const SPACING: usize = 4;
/// How far a corner must be from the edge, or from a sample the plane
/// does not have, for its descriptor to fit.
const BORDER: usize = PATCH / 2 * SPACING + 4;
/// Harris's constant.
const HARRIS_K: f32 = 0.04;

/// A corner and its descriptor.
#[derive(Debug, Clone)]
pub struct Keypoint {
    /// Its place, continuous.
    pub x: f32,
    pub y: f32,
    /// The blurred plane sampled about it, its mean off and unit length.
    pub descriptor: [f32; PATCH * PATCH],
}

/// How corners are found.
#[derive(Debug, Clone, Copy)]
pub struct Corners {
    /// The plane is cut into this many cells a side, and each keeps its
    /// strongest corners, so one textured corner of the frame does not
    /// take them all.
    pub grid: usize,
    /// Corners a cell keeps.
    pub per_cell: usize,
}

impl Default for Corners {
    fn default() -> Self {
        Self {
            grid: 16,
            per_cell: 10,
        }
    }
}

/// `src` (`width` by `height`) blurred by a Gaussian of `sigma`,
/// separably, the taps clamped at the edge.
pub fn blur(src: &[f32], width: usize, height: usize, sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as isize;
    let taps: Vec<f32> = (-r..=r)
        .map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f32 = taps.iter().sum();
    let taps: Vec<f32> = taps.iter().map(|v| v / total).collect();
    let (w, h) = (width as isize, height as isize);
    let mut rows = vec![0.0f32; width * height];
    rows.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let line = &src[y * width..(y + 1) * width];
        for (x, o) in row.iter_mut().enumerate() {
            *o = taps
                .iter()
                .enumerate()
                .map(|(j, t)| t * line[(x as isize + j as isize - r).clamp(0, w - 1) as usize])
                .sum();
        }
    });
    let mut out = vec![0.0f32; width * height];
    out.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            *o = taps
                .iter()
                .enumerate()
                .map(|(j, t)| {
                    t * rows[(y as isize + j as isize - r).clamp(0, h - 1) as usize * width + x]
                })
                .sum();
        }
    });
    out
}

/// The corners of `plane` (`width` by `height`, the log luminance; a
/// sample that is not finite is one the frame does not have), each with
/// its descriptor. A corner whose descriptor would reach a sample the
/// frame does not have is left out.
pub fn keypoints(plane: &[f32], width: usize, height: usize, how: &Corners) -> Vec<Keypoint> {
    let (w, h) = (width, height);
    if w <= 2 * BORDER || h <= 2 * BORDER {
        return Vec::new();
    }
    // What the frame does not have takes the plane's mean, and the
    // corners near it are refused below.
    let valid: Vec<bool> = plane.iter().map(|v| v.is_finite()).collect();
    let (sum, n) = plane
        .iter()
        .filter(|v| v.is_finite())
        .fold((0.0f64, 0usize), |(s, n), v| (s + f64::from(*v), n + 1));
    if n == 0 {
        return Vec::new();
    }
    let mean = (sum / n as f64) as f32;
    let filled: Vec<f32> = plane
        .iter()
        .map(|v| if v.is_finite() { *v } else { mean })
        .collect();
    // Far from anything missing: a corner's whole patch is there.
    let missing_near = near_missing(&valid, w, h, BORDER);

    let mut xx = vec![0.0f32; w * h];
    let mut yy = vec![0.0f32; w * h];
    let mut xy = vec![0.0f32; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let gx = 0.5 * (filled[i + 1] - filled[i - 1]);
            let gy = 0.5 * (filled[i + w] - filled[i - w]);
            xx[i] = gx * gx;
            yy[i] = gy * gy;
            xy[i] = gx * gy;
        }
    }
    let (xx, yy, xy) = (
        blur(&xx, w, h, 1.5),
        blur(&yy, w, h, 1.5),
        blur(&xy, w, h, 1.5),
    );
    let response: Vec<f32> = (0..w * h)
        .map(|i| xx[i] * yy[i] - xy[i] * xy[i] - HARRIS_K * (xx[i] + yy[i]).powi(2))
        .collect();
    let top = response.iter().copied().fold(0.0f32, f32::max);
    let floor = top * 1e-4;
    let grid = how.grid.max(1);
    let suppress = 3isize;
    let mut cells: Vec<Vec<(f32, usize, usize)>> = vec![Vec::new(); grid * grid];
    for y in BORDER..h - BORDER {
        for x in BORDER..w - BORDER {
            let v = response[y * w + x];
            if v <= floor || missing_near[y * w + x] {
                continue;
            }
            let peak = (-suppress..=suppress).all(|dy| {
                (-suppress..=suppress).all(|dx| {
                    (dx, dy) == (0, 0)
                        || response[(y as isize + dy) as usize * w + (x as isize + dx) as usize] < v
                })
            });
            if peak {
                cells[(y * grid / h) * grid + x * grid / w].push((v, x, y));
            }
        }
    }
    let smooth = blur(&filled, w, h, 2.0);
    let mut out = Vec::new();
    for mut cell in cells {
        cell.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(_, x, y) in cell.iter().take(how.per_cell) {
            let mut d = [0.0f32; PATCH * PATCH];
            for j in 0..PATCH {
                for i in 0..PATCH {
                    let sx = x as isize
                        + (i as isize - (PATCH / 2) as isize) * SPACING as isize
                        + (SPACING / 2) as isize;
                    let sy = y as isize
                        + (j as isize - (PATCH / 2) as isize) * SPACING as isize
                        + (SPACING / 2) as isize;
                    d[j * PATCH + i] = smooth[sy as usize * w + sx as usize];
                }
            }
            let m = d.iter().sum::<f32>() / d.len() as f32;
            d.iter_mut().for_each(|v| *v -= m);
            let norm = d.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm < 1e-3 {
                continue;
            }
            d.iter_mut().for_each(|v| *v /= norm);
            out.push(Keypoint {
                x: x as f32 + 0.5,
                y: y as f32 + 0.5,
                descriptor: d,
            });
        }
    }
    out
}

/// Whether each pixel is within `r` of a sample the frame lacks.
fn near_missing(valid: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let missing: Vec<bool> = valid.iter().map(|v| !v).collect();
    if !missing.iter().any(|m| *m) {
        return missing;
    }
    super::grow(&missing, w, h, r)
}

/// The nearest and second-nearest of `set` to `d`: the first's index
/// and both squared distances.
fn nearest_two(d: &[f32; PATCH * PATCH], set: &[Keypoint]) -> (usize, f32, f32) {
    let (mut best, mut d1, mut d2) = (0, f32::MAX, f32::MAX);
    for (j, k) in set.iter().enumerate() {
        let s: f32 = d
            .iter()
            .zip(&k.descriptor)
            .map(|(a, b)| (a - b) * (a - b))
            .sum();
        if s < d1 {
            d2 = d1;
            d1 = s;
            best = j;
        } else if s < d2 {
            d2 = s;
        }
    }
    (best, d1, d2)
}

/// Lowe's ratio: a match's distance at most this share of the next
/// best's.
pub const RATIO: f32 = 0.8;

/// The pairs `(i, j)` of `a[i]` and `b[j]` each the other's nearest,
/// the nearest closer than [`RATIO`] of the second.
pub fn matches(a: &[Keypoint], b: &[Keypoint]) -> Vec<(usize, usize)> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let ab: Vec<(usize, f32, f32)> = a
        .par_iter()
        .map(|k| nearest_two(&k.descriptor, b))
        .collect();
    let ba: Vec<usize> = b
        .par_iter()
        .map(|k| nearest_two(&k.descriptor, a).0)
        .collect();
    ab.iter()
        .enumerate()
        .filter(|(i, (j, d1, d2))| ba[*j] == *i && d1.sqrt() < RATIO * d2.sqrt())
        .map(|(i, (j, _, _))| (i, *j))
        .collect()
}

/// The shift and scale that put `points` about their centroid at a
/// mean distance of the square root of two, as Hartley's normalization
/// has it.
fn normalizer(points: &[(f64, f64)]) -> Homography {
    let n = points.len().max(1) as f64;
    let (mx, my) = points
        .iter()
        .fold((0.0, 0.0), |a, q| (a.0 + q.0 / n, a.1 + q.1 / n));
    let d = points
        .iter()
        .map(|q| (q.0 - mx).hypot(q.1 - my))
        .sum::<f64>()
        / n;
    let s = std::f64::consts::SQRT_2 / d.max(1e-9);
    Homography {
        m: [s, 0.0, -s * mx, 0.0, s, -s * my, 0.0, 0.0, 1.0],
    }
}

/// The least-squares homography taking `src` to `dst` (four pairs or
/// more), its last entry one, on normalized points; none when they
/// do not pin one down.
pub fn fit_points(src: &[(f64, f64)], dst: &[(f64, f64)]) -> Option<Homography> {
    if src.len() < 4 || src.len() != dst.len() {
        return None;
    }
    let (ts, td) = (normalizer(src), normalizer(dst));
    let mut ata = [[0.0f64; 8]; 8];
    let mut atb = [0.0f64; 8];
    for (s, d) in src.iter().zip(dst) {
        let (x, y) = ts.apply(s.0, s.1);
        let (u, v) = td.apply(d.0, d.1);
        let rows = [
            ([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u),
            ([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v),
        ];
        for (r, b) in rows {
            for i in 0..8 {
                atb[i] += r[i] * b;
                for j in 0..8 {
                    ata[i][j] += r[i] * r[j];
                }
            }
        }
    }
    let h = solve(ata, atb)?;
    let normalized = Homography {
        m: [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0],
    };
    let m = td.inverse()?.compose(&normalized).compose(&ts);
    m.m.iter().all(|v| v.is_finite()).then_some(m)
}

/// `a x = b` by Gaussian elimination with partial pivoting.
// Matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
pub(crate) fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for c in 0..N {
        let p = (c..N).max_by(|i, j| a[*i][c].abs().total_cmp(&a[*j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..N {
            let f = a[r][c] / a[c][c];
            for k in c..N {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; N];
    for r in (0..N).rev() {
        let s: f64 = (r + 1..N).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    Some(x)
}

/// What RANSAC found.
#[derive(Debug, Clone)]
pub struct Consensus {
    /// The homography from the first set's coordinates to the second's.
    pub homography: Homography,
    /// The pairs it holds, by their index among those given.
    pub inliers: Vec<usize>,
    /// The inliers' root-mean-square distance from where it puts them.
    pub rms: f64,
}

/// RANSAC over four-point samples, `iterations` of them, a pair an
/// inlier within `threshold`; then two least-squares fits on the
/// inliers. Deterministic: the samples are drawn from a fixed seed.
pub fn ransac(
    src: &[(f64, f64)],
    dst: &[(f64, f64)],
    iterations: usize,
    threshold: f64,
) -> Option<Consensus> {
    let n = src.len();
    if n < 4 || dst.len() != n {
        return None;
    }
    let held = |h: &Homography| -> Vec<usize> {
        (0..n)
            .filter(|&i| {
                if h.depth(src[i].0, src[i].1) <= 0.0 {
                    return false;
                }
                let (u, v) = h.apply(src[i].0, src[i].1);
                (u - dst[i].0).hypot(v - dst[i].1) < threshold
            })
            .collect()
    };
    let best = (0..iterations)
        .into_par_iter()
        .filter_map(|it| {
            let mut s =
                0x9E37_79B9_7F4A_7C15u64 ^ (it as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let mut next = || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s % n as u64) as usize
            };
            let mut pick = [0usize; 4];
            let mut k = 0;
            let mut tries = 0;
            while k < 4 && tries < 64 {
                tries += 1;
                let c = next();
                if !pick[..k].contains(&c) {
                    pick[k] = c;
                    k += 1;
                }
            }
            if k < 4 {
                return None;
            }
            let ps: Vec<_> = pick.iter().map(|&i| src[i]).collect();
            let pd: Vec<_> = pick.iter().map(|&i| dst[i]).collect();
            let h = fit_points(&ps, &pd)?;
            Some((held(&h).len(), it, h))
        })
        // The most inliers; of a tie, the first drawn, so the answer
        // does not depend on the threads.
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))?;
    let mut h = best.2;
    let mut inliers = held(&h);
    for _ in 0..2 {
        let ps: Vec<_> = inliers.iter().map(|&i| src[i]).collect();
        let pd: Vec<_> = inliers.iter().map(|&i| dst[i]).collect();
        let Some(refit) = fit_points(&ps, &pd) else {
            break;
        };
        let again = held(&refit);
        if again.len() < inliers.len() {
            break;
        }
        h = refit;
        inliers = again;
    }
    if inliers.len() < 4 {
        return None;
    }
    let rms = (inliers
        .iter()
        .map(|&i| {
            let (u, v) = h.apply(src[i].0, src[i].1);
            (u - dst[i].0).powi(2) + (v - dst[i].1).powi(2)
        })
        .sum::<f64>()
        / inliers.len() as f64)
        .sqrt();
    Some(Consensus {
        homography: h,
        inliers,
        rms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A textured plane: value noise at two scales, in the log.
    fn texture(_w: usize, _h: usize, seed: u64) -> impl Fn(f64, f64) -> f32 {
        let hash = move |i: i64, j: i64, o: u64| {
            let mut s = seed
                ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ o.wrapping_mul(0x1656_67B1_9E37_79F9);
            s ^= s >> 29;
            s = s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            s ^= s >> 32;
            (s >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        move |x, y| {
            [(7.0, 1.0), (17.0, 1.5)]
                .iter()
                .enumerate()
                .map(|(o, &(cell, amp))| {
                    let (u, v) = (x / cell, y / cell);
                    let (i, j) = (u.floor() as i64, v.floor() as i64);
                    let (fu, fv) = (u - i as f64, v - j as f64);
                    let s = |t: f64| t * t * (3.0 - 2.0 * t);
                    let (su, sv) = (s(fu), s(fv));
                    let o = o as u64;
                    let top = hash(i, j, o) * (1.0 - su) + hash(i + 1, j, o) * su;
                    let bottom = hash(i, j + 1, o) * (1.0 - su) + hash(i + 1, j + 1, o) * su;
                    amp * (top * (1.0 - sv) + bottom * sv)
                })
                .sum::<f64>() as f32
        }
    }

    /// Two views of one textured plane, the second through a known
    /// homography with a keystone and an exposure step: the corners
    /// match, and RANSAC finds the homography to a tenth of a pixel
    /// over the overlap.
    #[test]
    fn features_find_a_known_homography() {
        let (w, h) = (480usize, 360usize);
        let scene = texture(w, h, 7);
        // b's (u, v) is the scene at truth_inverse (u, v).
        let truth = Homography {
            m: [0.98, -0.03, -70.0, 0.025, 0.99, 6.0, 6e-5, -2e-5, 1.0],
        };
        let back = truth.inverse().unwrap();
        let a: Vec<f32> = (0..w * h)
            .map(|i| scene((i % w) as f64 + 0.5, (i / w) as f64 + 0.5))
            .collect();
        let b: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = back.apply((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
                // A stop darker: an offset in the log.
                scene(x, y) - 1.0
            })
            .collect();
        let how = Corners::default();
        let (ka, kb) = (keypoints(&a, w, h, &how), keypoints(&b, w, h, &how));
        assert!(
            ka.len() > 100 && kb.len() > 100,
            "{} {}",
            ka.len(),
            kb.len()
        );
        let m = matches(&ka, &kb);
        assert!(m.len() > 40, "{}", m.len());
        let src: Vec<(f64, f64)> = m
            .iter()
            .map(|(i, _)| (f64::from(ka[*i].x), f64::from(ka[*i].y)))
            .collect();
        let dst: Vec<(f64, f64)> = m
            .iter()
            .map(|(_, j)| (f64::from(kb[*j].x), f64::from(kb[*j].y)))
            .collect();
        let found = ransac(&src, &dst, 2000, 1.5).expect("a consensus");
        assert!(
            found.inliers.len() * 10 > m.len() * 7,
            "{}",
            found.inliers.len()
        );
        // Over the overlap: a's right part.
        let worst = [(150.0, 30.0), (450.0, 30.0), (150.0, 330.0), (450.0, 330.0)]
            .iter()
            .map(|&(x, y)| {
                let (p, q) = found.homography.apply(x, y);
                let (r, s) = truth.apply(x, y);
                (p - r).hypot(q - s)
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.5, "{worst}");
    }

    /// Four exact pairs pin the homography; a pair off by a pixel among
    /// many is left out.
    #[test]
    fn the_linear_fit_and_its_consensus() {
        let truth = Homography {
            m: [1.1, 0.05, 3.0, -0.02, 0.95, -4.0, 1e-4, 2e-4, 1.0],
        };
        let src: Vec<(f64, f64)> = (0..50)
            .map(|i| ((i * 37 % 200) as f64, (i * 53 % 150) as f64))
            .collect();
        let mut dst: Vec<(f64, f64)> = src.iter().map(|&(x, y)| truth.apply(x, y)).collect();
        let four = [0, 1, 3, 4];
        let h = fit_points(&four.map(|i| src[i]), &four.map(|i| dst[i])).unwrap();
        for (a, b) in h.m.iter().zip(&truth.m) {
            assert!((a - b).abs() < 1e-6, "{:?}", h.m);
        }
        dst[10].0 += 30.0;
        dst[20].1 -= 25.0;
        let c = ransac(&src, &dst, 300, 1.0).unwrap();
        assert_eq!(c.inliers.len(), 48);
        assert!(!c.inliers.contains(&10) && !c.inliers.contains(&20));
        assert!(c.rms < 1e-6, "{}", c.rms);
    }
}
