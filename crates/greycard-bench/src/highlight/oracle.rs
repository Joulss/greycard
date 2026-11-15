//! Oracles: reconstructions that cheat with the truth, to say where the
//! remaining error lives and how much a better method could win.
//!
//! - [`luma`]: the truth's luminance with a method's chroma. What a
//!   method would score if it got the brightness right; what is left is
//!   its color error.
//! - [`chroma`]: a method's luminance with the truth's chroma. The
//!   reverse.
//! - [`offset`]: inpaint opposed with each clipped region's ideal
//!   chrominance offset, per color, taken from the truth: the best that
//!   opposed's model (an opposed average plus one offset) can do with a
//!   perfect per-region estimate, which is what the segment pass
//!   estimates. Under the develop's ceiling, as the method is.

use std::collections::HashMap;

use rayon::prelude::*;

use greycard_core::develop::highlights::CLIP_MAGIC;
use greycard_core::raw::CfaPattern;

use super::clipmap::ClipMap;
use super::score::luma as lum;

/// `base` with the truth's luminance wherever the input was clipped.
pub fn luma(map: &ClipMap, truth: &[f32], base: &[f32]) -> Vec<f32> {
    swap(map, truth, base, |t, b| {
        let (yt, yb) = (lum(t), lum(b));
        if yb > 1e-6 { b.map(|v| v * yt / yb) } else { t }
    })
}

/// The truth's chroma at `base`'s luminance wherever the input was
/// clipped.
pub fn chroma(map: &ClipMap, truth: &[f32], base: &[f32]) -> Vec<f32> {
    swap(map, truth, base, |t, b| {
        let (yt, yb) = (lum(t), lum(b));
        if yt > 1e-6 { t.map(|v| v * yb / yt) } else { b }
    })
}

fn swap(
    map: &ClipMap,
    truth: &[f32],
    base: &[f32],
    f: impl Fn([f32; 3], [f32; 3]) -> [f32; 3] + Sync,
) -> Vec<f32> {
    let mut out = base.to_vec();
    out.par_chunks_mut(3).enumerate().for_each(|(i, px)| {
        if map.cell_colors[map.cell_of(i)] == 0 {
            return;
        }
        let t = [truth[3 * i], truth[3 * i + 1], truth[3 * i + 2]];
        let b = [px[0], px[1], px[2]];
        px.copy_from_slice(&f(t, b));
    });
    out
}

/// Inpaint opposed on `input` (white balanced, each channel at its own
/// clip, as the develop hands it the pass) with each region's offset
/// per color taken from the truth's mosaic: the median of what the
/// truth reads over the opposed average at the region's clipped
/// photosites of that color. A region with none of a color takes the
/// frame's median for it.
pub fn offset(
    map: &ClipMap,
    input: &[f32],
    truth: &[f32],
    pattern: &CfaPattern,
    gains: [f32; 3],
) -> Vec<f32> {
    let (w, h) = (map.width, map.height);
    let clips = gains.map(|g| g * CLIP_MAGIC);
    let white = clips.iter().copied().fold(0.0f32, f32::max) / CLIP_MAGIC;
    let color = |x: usize, y: usize| pattern.color_at(y, x).rgb_index().expect("RGB");

    // The opposed average of every clipped photosite, or none where its
    // whole neighborhood clipped.
    let refs: Vec<Option<f32>> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let c = color(x, y);
            if input[i] < clips[c] {
                return None;
            }
            refavg(input, w, h, pattern, clips, x, y, c)
        })
        .collect();

    let mut votes: HashMap<(u32, usize), Vec<f32>> = HashMap::new();
    let mut frame: [Vec<f32>; 3] = Default::default();
    for (i, r) in refs.iter().enumerate() {
        let Some(r) = r else { continue };
        let (x, y) = (i % w, i / w);
        let c = color(x, y);
        let d = truth[i] - r;
        let region = map.cell_region[map.cell_of(i)];
        votes.entry((region, c)).or_default().push(d);
        frame[c].push(d);
    }
    let median = |v: &mut Vec<f32>| {
        if v.is_empty() {
            return 0.0;
        }
        let k = v.len() / 2;
        *v.select_nth_unstable_by(k, f32::total_cmp).1
    };
    let fallback: [f32; 3] = std::array::from_fn(|c| median(&mut frame[c]));
    let offsets: HashMap<(u32, usize), f32> = votes
        .into_iter()
        .map(|(k, mut v)| (k, median(&mut v)))
        .collect();

    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let c = color(x, y);
            let v = input[i];
            if v < clips[c] {
                return v;
            }
            match refs[i] {
                None => white,
                Some(r) => {
                    let region = map.cell_region[map.cell_of(i)];
                    let o = offsets.get(&(region, c)).copied().unwrap_or(fallback[c]);
                    v.max(r + o)
                }
            }
        })
        .collect()
}

/// `base` (a reconstructed mosaic) with the truth's photosites wherever
/// the input clipped one in a cell where every color clipped: what is
/// left to invent, given. The rest stays the base's.
pub fn fully_clipped(map: &ClipMap, base: &[f32], truth: &[f32]) -> Vec<f32> {
    (0..base.len())
        .into_par_iter()
        .map(|i| {
            if map.clipped[i] && map.cell_colors[map.cell_of(i)].count_ones() == 3 {
                truth[i]
            } else {
                base[i]
            }
        })
        .collect()
}

/// darktable's opposed average: the means of the 3x3 neighborhood per
/// color in cube-root space, the two other colors' averaged and cubed
/// back; `None` when every photosite of the neighborhood is clipped.
#[allow(clippy::too_many_arguments)]
fn refavg(
    samples: &[f32],
    w: usize,
    h: usize,
    pattern: &CfaPattern,
    clips: [f32; 3],
    x: usize,
    y: usize,
    c: usize,
) -> Option<f32> {
    let mut sum = [0.0f32; 3];
    let mut cnt = [0u32; 3];
    let mut all = true;
    for ny in y.saturating_sub(1)..(y + 2).min(h) {
        for nx in x.saturating_sub(1)..(x + 2).min(w) {
            let k = pattern.color_at(ny, nx).rgb_index().expect("RGB");
            let v = samples[ny * w + nx];
            all &= v >= clips[k];
            sum[k] += v.max(0.0);
            cnt[k] += 1;
        }
    }
    if all {
        return None;
    }
    let mean = |k: usize| {
        if cnt[k] > 0 {
            (sum[k] / cnt[k] as f32).cbrt()
        } else {
            0.0
        }
    };
    let root = 0.5 * (mean((c + 1) % 3) + mean((c + 2) % 3));
    Some(root * root * root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_luma_and_chroma_oracles_swap_halves() {
        let (w, h) = (20usize, 20usize);
        let p = CfaPattern::rggb();
        let map = ClipMap::build(w, h, &p, |i| i % w > 10, |_| false);
        let truth: Vec<f32> = (0..w * h).flat_map(|_| [2.0, 1.0, 0.5]).collect();
        let base: Vec<f32> = (0..w * h).flat_map(|_| [1.0, 1.0, 1.0]).collect();
        let l = luma(&map, &truth, &base);
        let i = 5 * w + 15;
        let yt = lum([2.0, 1.0, 0.5]);
        assert!((lum([l[3 * i], l[3 * i + 1], l[3 * i + 2]]) - yt).abs() < 1e-5);
        assert!((l[3 * i] - l[3 * i + 1]).abs() < 1e-6);
        let c = chroma(&map, &truth, &base);
        assert!((lum([c[3 * i], c[3 * i + 1], c[3 * i + 2]]) - 1.0).abs() < 1e-5);
        assert!((c[3 * i] / c[3 * i + 1] - 2.0).abs() < 1e-5);
        // Unclipped pixels keep the base.
        assert_eq!(&l[..3], &[1.0, 1.0, 1.0]);
    }

    /// Only clipped photosites of cells with every color clipped take the
    /// truth.
    #[test]
    fn the_raised_oracle_gives_only_the_fully_clipped_cells() {
        let (w, h) = (8usize, 8usize);
        let p = CfaPattern::rggb();
        // Columns 0-3 clip everything; columns 4-5 clip green alone.
        let clipped = |i: usize| {
            let (x, y) = (i % w, i / w);
            x < 4 || ((4..6).contains(&x) && p.color_at(y, x).rgb_index() == Some(1))
        };
        let map = ClipMap::build(w, h, &p, clipped, |_| false);
        let base = vec![1.0f32; w * h];
        let truth = vec![5.0f32; w * h];
        let out = fully_clipped(&map, &base, &truth);
        assert_eq!(out[0], 5.0);
        assert_eq!(out[5], 1.0);
        assert_eq!(out[7], 1.0);
    }

    /// Green clipped over a ramp it keeps a constant offset from: the
    /// oracle's offset puts green back on the truth.
    #[test]
    fn the_offset_oracle_recovers_a_constant_offset() {
        let (w, h) = (60usize, 30usize);
        let p = CfaPattern::rggb();
        let gains = [2.0f32, 1.0, 2.0];
        let ramp = |x: usize| 0.6 + 1.0 * x as f32 / w as f32;
        let truth: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                match p.color_at(y, x).rgb_index().unwrap() {
                    1 => ramp(x) - 0.2,
                    _ => ramp(x),
                }
            })
            .collect();
        let input: Vec<f32> = truth
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let (x, y) = (i % w, i / w);
                v.min(gains[p.color_at(y, x).rgb_index().unwrap()])
            })
            .collect();
        let map = ClipMap::build(w, h, &p, |i| input[i] < truth[i], |_| false);
        let out = offset(&map, &input, &truth, &p, gains);
        for y in 10..20 {
            for x in 50..56 {
                let i = y * w + x;
                assert!(
                    (out[i] - truth[i]).abs() < 0.03,
                    "({x},{y}) {} {}",
                    out[i],
                    truth[i]
                );
            }
        }
    }
}
