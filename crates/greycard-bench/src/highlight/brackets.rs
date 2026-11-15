//! Real overexposure: pairs from an exposure bracket of a static scene.
//!
//! The darker frame, scaled by the exposure ratio, is the truth for the
//! brighter frame's clipped photosites. This catches what the synthetic
//! clipping cannot: a sensor's softness near saturation, blooming, and
//! the brighter frame's lower noise. A pair is taken only when the two
//! frames share the body, aperture, ISO and focal length, were taken
//! within a window of each other, and their mosaics agree, block by
//! block, outside the clipped areas once the ratio is divided out; the
//! ratio itself is measured on the mosaics rather than read from the
//! nominal shutter speeds.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use greycard_core::WhitePoint;
use greycard_core::decode::{Probe, probe_path};
use greycard_core::develop::highlights::CLIP_MAGIC;
use greycard_core::raw::{CfaPattern, Samples};

use super::clipmap::ClipMap;
use super::run::{Case, UNRELIABLE, demosaic_for};
use super::sim::{Scene, develop_truths};

/// A candidate pair: the darker and the brighter frame and the nominal
/// ratio of their exposures.
#[derive(Debug, Clone)]
pub struct Pair {
    pub dark: PathBuf,
    pub bright: PathBuf,
    pub nominal: f64,
}

/// Seconds since an epoch of EXIF's `YYYY:MM:DD HH:MM:SS`.
fn seconds(taken: &str) -> Option<i64> {
    let b = taken.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |a: usize, n: usize| taken.get(a..a + n)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (h, mi, s) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// Pairs among `files` (probed, not decoded): same body, aperture, ISO
/// and focal length, taken within `window` seconds, exposures apart by
/// at least `0.9` and at most `max_stops` stops.
pub fn find_pairs(files: &[PathBuf], window: i64, max_stops: f64) -> Vec<Pair> {
    let probes: Vec<(PathBuf, Probe, i64)> = files
        .iter()
        .filter_map(|p| {
            let probe = probe_path(p).ok()?;
            let t = seconds(probe.taken.as_deref()?)?;
            Some((p.clone(), probe, t))
        })
        .collect();
    let mut out = Vec::new();
    for (i, (pa, a, ta)) in probes.iter().enumerate() {
        for (pb, b, tb) in probes.iter().skip(i + 1) {
            let same = a.model == b.model
                && a.iso == b.iso
                && a.fnumber == b.fnumber
                && a.focal_length == b.focal_length;
            let (Some(ea), Some(eb)) = (a.exposure_time, b.exposure_time) else {
                continue;
            };
            if !same || (ta - tb).abs() > window || ea <= 0.0 || eb <= 0.0 {
                continue;
            }
            let stops = (eb / ea).log2();
            if stops.abs() < 0.9 || stops.abs() > max_stops + 0.1 {
                continue;
            }
            let (dark, bright) = if stops > 0.0 { (pa, pb) } else { (pb, pa) };
            out.push(Pair {
                dark: dark.clone(),
                bright: bright.clone(),
                nominal: 2f64.powf(stops.abs()),
            });
        }
    }
    out
}

/// What the check of a pair found.
#[derive(Debug, Clone)]
pub struct Agreement {
    /// The ratio measured on the mosaics.
    pub ratio: f64,
    /// The shift, even pixels, that puts the dark frame on the bright
    /// one: the dark frame's `(x - dx, y - dy)` is the bright one's
    /// `(x, y)`.
    pub shift: (isize, isize),
    /// Blocks compared, and the share that disagree.
    pub blocks: usize,
    pub moved: f64,
    /// Per block of [`BLOCK`] photosites, row-major: it disagreed, or a
    /// neighbor did; left out of the scores.
    pub masked: Vec<bool>,
    pub blocks_wide: usize,
}

/// Side of a block, photosites.
pub const BLOCK: usize = 64;
/// The farthest shift searched, pixels: a whole number of Bayer and of
/// X-Trans periods.
const SHIFT: isize = 12;

/// The bright frame's photosites a comparison reads: well exposed in it.
fn usable(b: f32, d: f32) -> bool {
    (0.1..0.8).contains(&b) && d > 0.002
}

/// The median of `log2(b / (d r))` over the usable photosites of a
/// sample, the dark frame read `shift` away.
fn misfit(dark: &Scene, bright: &Scene, ratio: f32, (dx, dy): (isize, isize)) -> f64 {
    let (w, h) = (bright.width as isize, bright.height as isize);
    let (d, b) = (&dark.normalized, &bright.normalized);
    let mut v: Vec<f32> = Vec::new();
    for y in (SHIFT..h - SHIFT).step_by(5) {
        for x in (SHIFT..w - SHIFT).step_by(11) {
            let i = (y * w + x) as usize;
            let j = ((y - dy) * w + x - dx) as usize;
            if usable(b[i], d[j]) {
                v.push((b[i] / (d[j] * ratio)).log2().abs());
            }
        }
    }
    if v.is_empty() {
        return f64::INFINITY;
    }
    let k = v.len() / 2;
    f64::from(*v.select_nth_unstable_by(k, f32::total_cmp).1)
}

/// Measure the pair: the ratio of the exposures, the even shift that
/// best lays the dark frame on the bright one (a tripod nudged, a
/// handheld bracket), and per block of 64x64 photosites whether the
/// two still disagree by more than a tenth of a stop, over the
/// photosites the bright frame read at `0.1..0.8` of its white.
pub fn agreement(dark: &Scene, bright: &Scene) -> Result<Agreement> {
    if (dark.width, dark.height) != (bright.width, bright.height) || dark.pattern != bright.pattern
    {
        bail!("the frames differ in size or pattern");
    }
    let (w, h) = (dark.width, dark.height);
    let (d, b) = (&dark.normalized, &bright.normalized);
    let mut ratios: Vec<f32> = (0..w * h)
        .step_by(7)
        .filter(|&i| usable(b[i], d[i]))
        .map(|i| b[i] / d[i])
        .collect();
    if ratios.len() < 1000 {
        bail!("too little of the frame is usable in both to compare");
    }
    let k = ratios.len() / 2;
    let rough = *ratios.select_nth_unstable_by(k, f32::total_cmp).1;
    let still = misfit(dark, bright, rough, (0, 0));
    let mut shift = (0, 0);
    let mut best = still;
    // Whole pattern periods only: 2 on Bayer, 6 on X-Trans.
    let (px, py) = period(&bright.pattern);
    for dy in (-SHIFT..=SHIFT).step_by(py as usize) {
        for dx in (-SHIFT..=SHIFT).step_by(px as usize) {
            let m = misfit(dark, bright, rough, (dx, dy));
            if m < best {
                (best, shift) = (m, (dx, dy));
            }
        }
    }
    // A shift is taken only when it clearly fits better: over a flat sky
    // the misfit hardly changes, and its minimum is noise.
    if best > 0.8 * still {
        shift = (0, 0);
    }
    let (dx, dy) = shift;
    let at = |x: usize, y: usize| -> Option<usize> {
        let (sx, sy) = (x as isize - dx, y as isize - dy);
        (sx >= 0 && sy >= 0 && sx < w as isize && sy < h as isize)
            .then(|| sy as usize * w + sx as usize)
    };
    let mut ratios: Vec<f32> = (0..w * h)
        .step_by(7)
        .filter_map(|i| {
            let j = at(i % w, i / w)?;
            usable(b[i], d[j]).then(|| b[i] / d[j])
        })
        .collect();
    let k = ratios.len() / 2;
    let ratio = *ratios.select_nth_unstable_by(k, f32::total_cmp).1;

    let bw = w.div_ceil(BLOCK);
    let bh = h.div_ceil(BLOCK);
    let mut moved_at = vec![false; bw * bh];
    let mut blocks = 0;
    let mut moved = 0;
    for by in 0..bh {
        for bx in 0..bw {
            let mut v: Vec<f32> = Vec::new();
            for y in by * BLOCK..((by + 1) * BLOCK).min(h) {
                for x in bx * BLOCK..((bx + 1) * BLOCK).min(w) {
                    let i = y * w + x;
                    match at(x, y) {
                        Some(j) if usable(b[i], d[j]) => v.push((b[i] / (d[j] * ratio)).log2()),
                        Some(_) => {}
                        None => moved_at[by * bw + bx] = true,
                    }
                }
            }
            if v.len() < 200 {
                continue;
            }
            blocks += 1;
            let k = v.len() / 2;
            if v.select_nth_unstable_by(k, f32::total_cmp).1.abs() > 0.1 {
                moved += 1;
                moved_at[by * bw + bx] = true;
            }
        }
    }
    // A moved block's neighbors go with it: what moved there may reach
    // into a clipped block that had nothing usable to compare.
    let masked: Vec<bool> = (0..bw * bh)
        .map(|k| {
            let (x, y) = ((k % bw) as isize, (k / bw) as isize);
            (-1..=1).any(|oy| {
                (-1..=1).any(|ox| {
                    let (nx, ny) = (x + ox, y + oy);
                    nx >= 0
                        && ny >= 0
                        && nx < bw as isize
                        && ny < bh as isize
                        && moved_at[ny as usize * bw + nx as usize]
                })
            })
        })
        .collect();
    Ok(Agreement {
        ratio: f64::from(ratio),
        shift,
        blocks,
        moved: moved as f64 / blocks.max(1) as f64,
        masked,
        blocks_wide: bw,
    })
}

/// The steps a shift may take and keep the mosaic's pattern where it
/// was: 2 on Bayer, 6 on X-Trans, across and down.
pub fn period(pattern: &CfaPattern) -> (isize, isize) {
    (pattern.width as isize, pattern.height as isize)
}

/// Refuse a shift that would move the pattern under the samples.
pub fn check_shift(pattern: &CfaPattern, (dx, dy): (isize, isize)) -> Result<()> {
    let (px, py) = period(pattern);
    if dx % px != 0 || dy % py != 0 {
        bail!(
            "a shift of ({dx}, {dy}) moves a {}x{} pattern off its phase",
            pattern.width,
            pattern.height
        );
    }
    Ok(())
}

/// A plane of `w` by `h` moved by `shift`: its `(x, y)` becomes the old
/// `(x - dx, y - dy)`, and past an edge the nearest sample of the same
/// pattern phase (steps of `period`) is repeated.
pub fn shift_plane<T: Copy>(
    v: &[T],
    (w, h): (usize, usize),
    (dx, dy): (isize, isize),
    (px, py): (isize, isize),
) -> Vec<T> {
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let mut sx = x - dx;
            let mut sy = y - dy;
            while sx < 0 {
                sx += px;
            }
            while sx >= w as isize {
                sx -= px;
            }
            while sy < 0 {
                sy += py;
            }
            while sy >= h as isize {
                sy -= py;
            }
            v[sy as usize * w + sx as usize]
        })
        .collect()
}

/// `scene` moved by `shift`, a whole number of pattern periods: the raw
/// samples and the normalized mosaic both, so the truth develops from
/// the moved frame.
pub fn shifted(mut scene: Scene, shift: (isize, isize)) -> Result<Scene> {
    check_shift(&scene.pattern, shift)?;
    if shift == (0, 0) {
        return Ok(scene);
    }
    let p = period(&scene.pattern);
    let size = (scene.frame.width, scene.frame.height);
    scene.frame.samples = match &scene.frame.samples {
        Samples::U16(v) => Samples::U16(shift_plane(v, size, shift, p)),
        Samples::F32(v) => Samples::F32(shift_plane(v, size, shift, p)),
    };
    scene.normalized = shift_plane(&scene.normalized, (scene.width, scene.height), shift, p);
    Ok(scene)
}

/// The case of a pair: the bright frame as the input, the dark one
/// (moved by the agreement's shift) times its ratio as the truth, the
/// blocks that moved left out. Both frames are balanced by the bright
/// frame's as-shot coefficients, so the truth's white and matrix are
/// the input's whatever the camera recorded for the dark frame.
pub fn bracket_case(dark: Scene, bright: &Scene, a: &Agreement) -> Result<Case> {
    let dark = shifted(dark, a.shift)?;
    let demosaic = demosaic_for(&bright.pattern);
    let white = bright
        .frame
        .as_shot_coefficients
        .map(WhitePoint::Coefficients)
        .context("the bright frame records no white balance")?;
    let (truth, capped, ceiling) = develop_truths(&dark.frame, a.ratio as f32, demosaic, white)?;
    let (b, d) = (&bright.normalized, &dark.normalized);
    let w = bright.width;
    let map = ClipMap::build(
        bright.width,
        bright.height,
        &bright.pattern,
        |i| b[i] >= CLIP_MAGIC,
        |i| {
            let (x, y) = (i % w, i / w);
            d[i] >= UNRELIABLE || a.masked[(y / BLOCK) * a.blocks_wide + x / BLOCK]
        },
    );
    Ok(Case {
        input: bright.frame.clone(),
        pattern: bright.pattern.clone(),
        truth,
        capped,
        ceiling,
        map,
        demosaic,
    })
}

/// Every raw under `inputs` (files, or folders searched one level down).
pub fn raws(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in inputs {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut v: Vec<PathBuf> = rd
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| is_raw(p))
                    .collect();
                v.sort();
                out.extend(v);
            }
        } else if is_raw(p) {
            out.push(p.clone());
        }
    }
    out
}

fn is_raw(p: &Path) -> bool {
    p.is_file() && greycard_core::decode::is_raw_path(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_times_count_seconds() {
        let a = seconds("2026:11:08 17:17:19").unwrap();
        let b = seconds("2026:11:08 17:17:36").unwrap();
        assert_eq!(b - a, 17);
        let c = seconds("2026:03:01 00:00:00").unwrap();
        let d = seconds("2026:02:28 23:59:59").unwrap();
        assert_eq!(c - d, 1);
        assert!(seconds("junk").is_none());
    }

    /// An X-Trans frame moves only by whole 6x6 periods, and every
    /// photosite keeps its color, edges included; a Bayer one by two.
    #[test]
    fn shifts_keep_the_pattern_phase() {
        let xtrans = CfaPattern::xtrans().shifted(1, 4);
        assert_eq!(period(&xtrans), (6, 6));
        assert_eq!(period(&CfaPattern::rggb()), (2, 2));
        assert!(check_shift(&xtrans, (2, 0)).is_err());
        assert!(check_shift(&xtrans, (6, -12)).is_ok());
        assert!(check_shift(&CfaPattern::rggb(), (2, -4)).is_ok());
        assert!(check_shift(&CfaPattern::rggb(), (1, 0)).is_err());
        let (w, h) = (30usize, 24usize);
        let colors: Vec<usize> = (0..w * h)
            .map(|i| xtrans.color_at(i / w, i % w).rgb_index().unwrap())
            .collect();
        for shift in [(6, -12), (-12, 6), (12, 12)] {
            assert_eq!(
                shift_plane(&colors, (w, h), shift, (6, 6)),
                colors,
                "{shift:?}"
            );
        }
        // Values move with the shift.
        let ramp: Vec<usize> = (0..w * h).collect();
        let moved = shift_plane(&ramp, (w, h), (6, 0), (6, 6));
        assert_eq!(moved[10], 4);
    }
}
