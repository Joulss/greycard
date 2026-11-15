//! The scores, computed only where the input was clipped.
//!
//! In the working space (linear Rec.2020), after the one demosaic every
//! method shares, per pixel:
//!
//! - **stops**: `log2` of the reconstruction's luminance over the
//!   truth's. Signed, so a bias says whether a method under- or
//!   overshoots.
//! - **dab**: Oklab `ab` distance at matched lightness: both pixels
//!   scaled to unit luminance first, so it is a hue and chroma error
//!   alone, and a magenta or cyan cast shows in the signed means of
//!   `a` and `b`.
//! - **dE**: Oklab distance with both pixels scaled by the truth's
//!   luminance: lightness and color in one perceptual number, the one
//!   the oracles split.
//!
//! Over each regime and size class, sums for a **structure** score: the
//! gradients of log luminance (smoothed over 5x5 so the truth's noise is
//! not taken for structure), their correlation between reconstruction
//! and truth, and the ratio of their energies, how much of the truth's
//! local contrast survives.
//!
//! And at the regions' edges, the **ring**: the error just outside the
//! clipped cells, where a method's reach (or the demosaic's) leaks into
//! unclipped pixels, and the **step**, the median signed error just
//! inside less the one just outside, a jump at the clip boundary the
//! truth does not have.
//!
//! In the mosaic, for a method that returns one, the same stops per
//! clipped photosite against the truth's photosite.

use rayon::prelude::*;

use greycard_core::color::Oklab;

use super::clipmap::{Band, ClipMap};

/// Rec.2020's luminance weights.
pub const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];
/// Samples kept per bucket and frame; the rest are skipped by a hash of
/// the pixel's index, so the choice repeats.
pub const SAMPLES: usize = 4000;
/// The most pixels one frame's bucket weighs in a pooled summary.
pub const POOL_CAP: f64 = 50_000.0;
/// Smoothing radius of the structure score's log luminance.
const SMOOTH: usize = 2;
/// The gradient's step, pixels.
const STEP: usize = 2;
const TINY: f32 = 1e-6;

#[inline]
pub fn luma(px: [f32; 3]) -> f32 {
    LUMA[0] * px[0] + LUMA[1] * px[1] + LUMA[2] * px[2]
}

#[inline]
fn px(rgb: &[f32], i: usize) -> [f32; 3] {
    [rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]]
}

/// Samples of one bucket, and the sums that pool exactly.
#[derive(Debug, Clone, Default)]
pub struct Bucket {
    /// Pixels in the bucket.
    pub n: usize,
    pub stops: Vec<f32>,
    pub dab: Vec<f32>,
    pub de: Vec<f32>,
    pub sum_stops: f64,
    pub sum_da: f64,
    pub sum_db: f64,
    /// Structure: sums of truth·reconstruction, truth², reconstruction²
    /// over the gradients.
    pub g_tr: f64,
    pub g_tt: f64,
    pub g_rr: f64,
    pub g_n: usize,
}

impl Bucket {
    fn merge(&mut self, o: Bucket) {
        self.n += o.n;
        self.stops.extend(o.stops);
        self.dab.extend(o.dab);
        self.de.extend(o.de);
        self.sum_stops += o.sum_stops;
        self.sum_da += o.sum_da;
        self.sum_db += o.sum_db;
        self.g_tr += o.g_tr;
        self.g_tt += o.g_tt;
        self.g_rr += o.g_rr;
        self.g_n += o.g_n;
    }

    /// Each sample's weight: the pixels it stands for, capped at
    /// `cap` for the bucket as a whole.
    pub fn weight(&self, cap: f64) -> f64 {
        if self.stops.is_empty() {
            0.0
        } else {
            (self.n as f64).min(cap) / self.stops.len() as f64
        }
    }
}

/// The ring at the regions' edges.
#[derive(Debug, Clone, Default)]
pub struct Ring {
    pub outside: Bucket,
    /// Signed stops just inside, sampled.
    pub inside: Vec<f32>,
}

/// One image's scores: nine buckets, regime (1 to 3) by size class, and
/// the ring.
#[derive(Debug, Clone, Default)]
pub struct ImageScore {
    pub buckets: [[Bucket; 3]; 3],
    pub ring: Ring,
}

/// Stable per-pixel hash, for the subsampling.
#[inline]
fn hash(i: usize) -> u64 {
    let mut z = (i as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The keep-one-in-this-many of each bucket, from its size.
pub fn strides(counts: &[[usize; 3]; 3]) -> [[u64; 3]; 3] {
    counts.map(|row| row.map(|n| n.div_ceil(SAMPLES).max(1) as u64))
}

/// Log luminance smoothed by a box of side `2 SMOOTH + 1`: what the
/// structure score differentiates. Computed for the truth once and for
/// each reconstruction.
pub fn smooth_log_luma(rgb: &[f32], width: usize, height: usize) -> Vec<f32> {
    let log: Vec<f32> = (0..width * height)
        .into_par_iter()
        .map(|i| luma(px(rgb, i)).max(TINY).log2())
        .collect();
    let r = SMOOTH;
    let rows: Vec<f32> = log
        .par_chunks(width)
        .flat_map_iter(|row| {
            (0..width).map(move |x| {
                let (a, b) = (x.saturating_sub(r), (x + r + 1).min(width));
                row[a..b].iter().sum::<f32>() / (b - a) as f32
            })
        })
        .collect();
    (0..width * height)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % width, i / width);
            let (a, b) = (y.saturating_sub(r), (y + r + 1).min(height));
            (a..b).map(|yy| rows[yy * width + x]).sum::<f32>() / (b - a) as f32
        })
        .collect()
}

/// Score `rec` against `truth`, both working-space RGB of the map's size.
/// `truth_log` and `rec_log` are their [`smooth_log_luma`].
pub fn score_image(
    map: &ClipMap,
    truth: &[f32],
    rec: &[f32],
    truth_log: &[f32],
    rec_log: &[f32],
    stride: &[[u64; 3]; 3],
) -> ImageScore {
    let lab = Oklab::for_working_space();
    let (w, h) = (map.width, map.height);
    let ring_stride = 16u64;
    (0..h)
        .into_par_iter()
        .fold(ImageScore::default, |mut acc, y| {
            for x in 0..w {
                let i = y * w + x;
                let regime = map.regime[i];
                let band = map.band[i];
                let scored = (1..=3).contains(&regime);
                if !scored && band != Band::Outside {
                    continue;
                }
                let t = px(truth, i);
                let r = px(rec, i);
                let yt = luma(t);
                if yt <= TINY {
                    continue;
                }
                let yr = luma(r).max(TINY);
                let stops = (yr / yt).log2();
                let unit_t = lab.of(t.map(|v| v / yt));
                let unit_r = lab.of(r.map(|v| v / yr));
                let (da, db) = (unit_r[1] - unit_t[1], unit_r[2] - unit_t[2]);
                let dab = da.hypot(db);
                if band == Band::Outside {
                    let o = &mut acc.ring.outside;
                    o.n += 1;
                    o.sum_stops += f64::from(stops);
                    if hash(i).is_multiple_of(ring_stride) {
                        o.stops.push(stops);
                        o.dab.push(dab);
                    }
                    continue;
                }
                let Some(bin) = map.bin(i) else { continue };
                let (ri, bi) = (regime as usize - 1, bin.index());
                if band == Band::Inside && hash(i).is_multiple_of(ring_stride) {
                    acc.ring.inside.push(stops);
                }
                let b = &mut acc.buckets[ri][bi];
                b.n += 1;
                b.sum_stops += f64::from(stops);
                b.sum_da += f64::from(da);
                b.sum_db += f64::from(db);
                if hash(i).is_multiple_of(stride[ri][bi]) {
                    let scaled = lab.of(r.map(|v| v / yt));
                    let de = ((scaled[0] - unit_t[0]).powi(2)
                        + (scaled[1] - unit_t[1]).powi(2)
                        + (scaled[2] - unit_t[2]).powi(2))
                    .sqrt();
                    b.stops.push(stops);
                    b.dab.push(dab);
                    b.de.push(de);
                }
                // Structure: a gradient where the pixel and its four
                // neighbors a step away are all clipped, scored, and
                // clear of the edge band, so no smoothing window reaches
                // an unclipped pixel: otherwise the step at the region's
                // edge, which a flat fill has too, passes for structure.
                let interior =
                    |n: usize| (1..=3).contains(&map.regime[n]) && map.band[n] != Band::Inside;
                if x >= STEP && y >= STEP && x + STEP < w && y + STEP < h && interior(i) {
                    let near = [i - STEP, i + STEP, i - STEP * w, i + STEP * w];
                    if near.iter().all(|&n| interior(n)) {
                        let gt = [
                            truth_log[i + STEP] - truth_log[i - STEP],
                            truth_log[i + STEP * w] - truth_log[i - STEP * w],
                        ];
                        let gr = [
                            rec_log[i + STEP] - rec_log[i - STEP],
                            rec_log[i + STEP * w] - rec_log[i - STEP * w],
                        ];
                        b.g_tr += f64::from(gt[0] * gr[0] + gt[1] * gr[1]);
                        b.g_tt += f64::from(gt[0] * gt[0] + gt[1] * gt[1]);
                        b.g_rr += f64::from(gr[0] * gr[0] + gr[1] * gr[1]);
                        b.g_n += 1;
                    }
                }
            }
            acc
        })
        .reduce(ImageScore::default, |mut a, b| {
            let [b0, b1, b2] = b.buckets;
            for (ri, row) in [b0, b1, b2].into_iter().enumerate() {
                let [c0, c1, c2] = row;
                for (bi, bucket) in [c0, c1, c2].into_iter().enumerate() {
                    a.buckets[ri][bi].merge(bucket);
                }
            }
            a.ring.outside.merge(b.ring.outside);
            a.ring.inside.extend(b.ring.inside);
            a
        })
}

/// Score a reconstructed mosaic against the truth's at every scored
/// clipped photosite: stops only.
pub fn score_mosaic(
    map: &ClipMap,
    truth: &[f32],
    rec: &[f32],
    stride: &[[u64; 3]; 3],
) -> [[Bucket; 3]; 3] {
    (0..map.height)
        .into_par_iter()
        .fold(<[[Bucket; 3]; 3]>::default, |mut acc, y| {
            for x in 0..map.width {
                let i = y * map.width + x;
                let regime = map.regime[i];
                if !map.clipped[i] || !(1..=3).contains(&regime) || truth[i] <= TINY {
                    continue;
                }
                let Some(bin) = map.bin(i) else { continue };
                let (ri, bi) = (regime as usize - 1, bin.index());
                let stops = (rec[i].max(TINY) / truth[i]).log2();
                let b = &mut acc[ri][bi];
                b.n += 1;
                b.sum_stops += f64::from(stops);
                // Fewer photosites than pixels are clipped: half the stride.
                if hash(i).is_multiple_of(stride[ri][bi].div_ceil(2)) {
                    b.stops.push(stops);
                }
            }
            acc
        })
        .reduce(<[[Bucket; 3]; 3]>::default, |a, b| {
            let mut a = a;
            let [b0, b1, b2] = b;
            for (ri, row) in [b0, b1, b2].into_iter().enumerate() {
                let [c0, c1, c2] = row;
                for (bi, bucket) in [c0, c1, c2].into_iter().enumerate() {
                    a[ri][bi].merge(bucket);
                }
            }
            a
        })
}

/// A weighted quantile of `(value, weight)` pairs, `q` in 0..=1.
pub fn quantile(pairs: &mut [(f32, f64)], q: f64) -> f64 {
    if pairs.is_empty() {
        return f64::NAN;
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    let target = q * total;
    let mut acc = 0.0;
    for p in pairs.iter() {
        acc += p.1;
        if acc >= target {
            return f64::from(p.0);
        }
    }
    f64::from(pairs[pairs.len() - 1].0)
}

/// What a table row says of a pool of buckets.
#[derive(Debug, Clone, Copy, Default)]
pub struct Summary {
    pub n: usize,
    pub med_abs_stops: f64,
    pub p90_abs_stops: f64,
    pub med_stops: f64,
    pub mean_stops: f64,
    pub med_dab: f64,
    pub p90_dab: f64,
    pub mean_da: f64,
    pub mean_db: f64,
    pub med_de: f64,
    pub p90_de: f64,
    pub structure: f64,
    pub contrast: f64,
}

/// Summarize buckets pooled together, each sample weighted by the pixels
/// it stands for with every bucket capped at `cap` pixels (`f64::INFINITY`
/// for plain pooling within a frame).
pub fn summarize<'a>(buckets: impl IntoIterator<Item = &'a Bucket>, cap: f64) -> Summary {
    let mut s = Summary::default();
    let mut abs = Vec::new();
    let mut signed = Vec::new();
    let mut dab = Vec::new();
    let mut de = Vec::new();
    let (mut ws, mut sum_stops, mut sum_da, mut sum_db) = (0.0, 0.0, 0.0, 0.0);
    let (mut tr, mut tt, mut rr) = (0.0, 0.0, 0.0);
    for b in buckets {
        if b.n == 0 {
            continue;
        }
        s.n += b.n;
        let wgt = b.weight(cap);
        let scale = (b.n as f64).min(cap) / b.n as f64;
        ws += b.n as f64 * scale;
        sum_stops += b.sum_stops * scale;
        sum_da += b.sum_da * scale;
        sum_db += b.sum_db * scale;
        tr += b.g_tr * scale;
        tt += b.g_tt * scale;
        rr += b.g_rr * scale;
        abs.extend(b.stops.iter().map(|v| (v.abs(), wgt)));
        signed.extend(b.stops.iter().map(|v| (*v, wgt)));
        dab.extend(b.dab.iter().map(|v| (*v, wgt)));
        de.extend(b.de.iter().map(|v| (*v, wgt)));
    }
    if s.n == 0 {
        return Summary {
            n: 0,
            ..nan_summary()
        };
    }
    s.med_abs_stops = quantile(&mut abs, 0.5);
    s.p90_abs_stops = quantile(&mut abs, 0.9);
    s.med_stops = quantile(&mut signed, 0.5);
    s.mean_stops = sum_stops / ws;
    s.med_dab = quantile(&mut dab, 0.5);
    s.p90_dab = quantile(&mut dab, 0.9);
    s.mean_da = sum_da / ws;
    s.mean_db = sum_db / ws;
    s.med_de = quantile(&mut de, 0.5);
    s.p90_de = quantile(&mut de, 0.9);
    s.structure = if tt > 0.0 && rr > 0.0 {
        tr / (tt * rr).sqrt()
    } else if tt > 0.0 {
        0.0
    } else {
        f64::NAN
    };
    s.contrast = if tt > 0.0 { (rr / tt).sqrt() } else { f64::NAN };
    s
}

fn nan_summary() -> Summary {
    Summary {
        n: 0,
        med_abs_stops: f64::NAN,
        p90_abs_stops: f64::NAN,
        med_stops: f64::NAN,
        mean_stops: f64::NAN,
        med_dab: f64::NAN,
        p90_dab: f64::NAN,
        mean_da: f64::NAN,
        mean_db: f64::NAN,
        med_de: f64::NAN,
        p90_de: f64::NAN,
        structure: f64::NAN,
        contrast: f64::NAN,
    }
}

/// The ring's row: the error just outside (median |stops|, 90th
/// percentile, median dab) and the step across the edge.
#[derive(Debug, Clone, Copy, Default)]
pub struct RingSummary {
    pub n: usize,
    pub med_abs_stops: f64,
    pub p90_abs_stops: f64,
    pub med_dab: f64,
    pub step: f64,
}

pub fn summarize_ring<'a>(rings: impl IntoIterator<Item = &'a Ring>) -> RingSummary {
    let mut abs = Vec::new();
    let mut outside = Vec::new();
    let mut dab = Vec::new();
    let mut inside = Vec::new();
    let mut n = 0;
    for r in rings {
        n += r.outside.n;
        let wgt = r.outside.weight(POOL_CAP);
        abs.extend(r.outside.stops.iter().map(|v| (v.abs(), wgt)));
        outside.extend(r.outside.stops.iter().map(|v| (*v, wgt)));
        dab.extend(r.outside.dab.iter().map(|v| (*v, wgt)));
        inside.extend(r.inside.iter().map(|v| (*v, 1.0)));
    }
    RingSummary {
        n,
        med_abs_stops: quantile(&mut abs, 0.5),
        p90_abs_stops: quantile(&mut abs, 0.9),
        med_dab: quantile(&mut dab, 0.5),
        step: quantile(&mut inside, 0.5) - quantile(&mut outside, 0.5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use greycard_core::raw::CfaPattern;

    #[test]
    fn weighted_quantiles_pick_by_weight() {
        let mut v = vec![(1.0f32, 1.0), (2.0, 1.0), (3.0, 1.0)];
        assert_eq!(quantile(&mut v, 0.5), 2.0);
        let mut v = vec![(1.0f32, 10.0), (2.0, 1.0), (3.0, 1.0)];
        assert_eq!(quantile(&mut v, 0.5), 1.0);
        assert!(quantile(&mut [], 0.5).is_nan());
    }

    /// A flat grey frame with a clipped square: a perfect
    /// reconstruction scores zero everywhere, one a stop under scores a
    /// stop under with no color error, and a tinted one has a color
    /// error and no luminance error.
    #[test]
    fn scores_measure_what_they_say() {
        let (w, h) = (48usize, 48usize);
        let p = CfaPattern::rggb();
        let inside = |i: usize| {
            let (x, y) = (i % w, i / w);
            (16..32).contains(&x) && (16..32).contains(&y)
        };
        let map = ClipMap::build(w, h, &p, inside, |_| false);
        // A gentle ramp so the structure score has something to see.
        let truth: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let v = 1.0 + (i % w) as f32 / 16.0;
                [v, v, v]
            })
            .collect();
        let stride = strides(&map.counts());
        let tl = smooth_log_luma(&truth, w, h);
        let same = score_image(&map, &truth, &truth, &tl, &tl, &stride);
        let s = summarize(same.buckets.iter().flatten(), f64::INFINITY);
        assert_eq!(s.n, 256);
        assert!(s.med_abs_stops.abs() < 1e-6 && s.med_dab < 1e-5 && s.med_de < 1e-5);
        assert!((s.structure - 1.0).abs() < 1e-6 && (s.contrast - 1.0).abs() < 1e-6);

        let half: Vec<f32> = truth
            .iter()
            .enumerate()
            .map(|(k, v)| if inside(k / 3) { v / 2.0 } else { *v })
            .collect();
        let hl = smooth_log_luma(&half, w, h);
        let s = summarize(
            score_image(&map, &truth, &half, &tl, &hl, &stride)
                .buckets
                .iter()
                .flatten(),
            f64::INFINITY,
        );
        assert!((s.med_stops + 1.0).abs() < 1e-5, "{s:?}");
        assert!(s.med_dab < 1e-5);
        assert!(s.med_de > 0.1);

        // Same luminance, a magenta cast.
        let tinted: Vec<f32> = truth
            .chunks(3)
            .enumerate()
            .flat_map(|(k, px)| {
                if inside(k) {
                    let t = [px[0] * 1.2, 0.0, px[2] * 1.2];
                    let y = luma(t);
                    let want = luma([px[0], px[1], px[2]]);
                    let g = (want - y) / LUMA[1];
                    [t[0], g, t[2]]
                } else {
                    [px[0], px[1], px[2]]
                }
            })
            .collect();
        let rl = smooth_log_luma(&tinted, w, h);
        let scored = score_image(&map, &truth, &tinted, &tl, &rl, &stride);
        let s = summarize(scored.buckets.iter().flatten(), f64::INFINITY);
        assert!(s.med_abs_stops < 1e-4, "{s:?}");
        assert!(s.med_dab > 0.02 && s.mean_da > 0.0, "{s:?}");
        let ring = summarize_ring([&scored.ring]);
        assert!(ring.n > 0 && ring.med_abs_stops < 1e-6);
    }
}
