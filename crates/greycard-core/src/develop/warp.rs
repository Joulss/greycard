//! A DNG's rectilinear warps on the demosaiced picture: `OpcodeList3`'s
//! `WarpRectilinear` and `WarpRectilinear2`, a lens's distortion and,
//! with a set of coefficients a plane, its lateral chromatic aberration.
//!
//! Each output pixel is mapped to where the picture as taken has it and
//! read there, bilinear between the four samples around it. The mapping
//! is the specification's formula as Adobe's DNG SDK reads it
//! (`dng_filter_warp`), with no SDK code: pixels at their whole
//! coordinates in the image the list runs on (the active area), the
//! center at its relative position over that image's bounds, the radius
//! relative to the farthest corner and held at 1, the read held to the
//! picture's edge. Consecutive warps are one map, each pixel traced
//! back through every warp and read once, where the SDK resamples after
//! each; it resamples bicubic, and bilinear is our choice, as the lens
//! correction's is.

use rayon::prelude::*;

use crate::develop::gain_map::MapPlacement;
use crate::opcodes::{Warp, WarpPlane};
use crate::raw::{RawFrame, Rect};

/// Where a buffer sits in the image the warps are defined over, in the
/// buffer's own pixels: a binned buffer's pixel stands for a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarpPlacement {
    /// The image's coordinates of the buffer's first pixel.
    pub x: f64,
    pub y: f64,
    /// The image's size.
    pub image_width: f64,
    pub image_height: f64,
}

impl WarpPlacement {
    /// The placement of a buffer binned by `bin` from the crop `area`
    /// places: a binned pixel sits at the middle of its block.
    pub fn binned(area: MapPlacement, bin: usize) -> Self {
        let bin = bin.max(1) as f64;
        let middle = (bin - 1.0) / 2.0;
        Self {
            x: (area.x as f64 + middle) / bin,
            y: (area.y as f64 + middle) / bin,
            image_width: area.image_width as f64 / bin,
            image_height: area.image_height as f64 / bin,
        }
    }
}

/// Where the frame's crop sits in the area its warps are measured in,
/// or `None` when it has no warps, or (which no camera writes) a crop
/// reaching outside that area, said in the log.
pub fn area_of(frame: &RawFrame) -> Option<MapPlacement> {
    if frame.dng.warps.is_empty() {
        return None;
    }
    let crop = frame.effective_crop();
    let area = frame.dng.opcode_area.unwrap_or(Rect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    });
    if crop.x < area.x || crop.y < area.y {
        log::warn!("DNG warps left undone: the crop {crop:?} is outside {area:?}");
        return None;
    }
    Some(MapPlacement {
        x: crop.x - area.x,
        y: crop.y - area.y,
        image_width: area.width,
        image_height: area.height,
    })
}

/// The warps, in order, on interleaved RGB `rgb` of `width` by
/// `height`, placed in their image by `at`: one resample, each pixel's
/// source traced back through the warps from the last to the first.
pub fn apply_warps(
    rgb: &mut Vec<f32>,
    width: usize,
    height: usize,
    at: WarpPlacement,
    warps: &[Warp],
) {
    if !warps.is_empty() {
        *rgb = warp_rgb(rgb, width, height, at, warps);
    }
}

/// The warps, in order, of interleaved RGB: a new picture of the same
/// size, read once.
pub fn warp_rgb(
    rgb: &[f32],
    width: usize,
    height: usize,
    at: WarpPlacement,
    warps: &[Warp],
) -> Vec<f32> {
    let warps: Vec<&Warp> = warps.iter().filter(|w| !w.planes.is_empty()).collect();
    if width == 0
        || height == 0
        || warps.is_empty()
        || at.image_width <= 0.0
        || at.image_height <= 0.0
    {
        return rgb.to_vec();
    }
    let geometries: Vec<Geometry> = warps.iter().map(|w| Geometry::new(at, w)).collect();
    let shared = warps.iter().all(|w| w.planes.len() == 1);
    // A plane no warp moves is copied, not resampled: the round trip
    // through the center is not exact in floats.
    let identity = WarpPlane::identity();
    let moves = |p: &WarpPlane| p.radial != identity.radial || p.tangential != identity.tangential;
    let moved: [bool; 3] = std::array::from_fn(|c| warps.iter().any(|w| moves(w.plane(c))));
    let (w_max, h_max) = ((width - 1) as f64, (height - 1) as f64);
    // The source of (x, y) for plane `c`: through the last warp, then
    // each before it, held to the picture as each warp's read is.
    let trace = |x: usize, y: usize, c: usize| -> (f64, f64) {
        let mut p = (x as f64, y as f64);
        for (warp, g) in warps.iter().zip(&geometries).rev() {
            let (sx, sy) = g.source(p.0, p.1, warp.plane(c), warp.reciprocal);
            p = (sx.clamp(0.0, w_max), sy.clamp(0.0, h_max));
        }
        p
    };
    let mut out = vec![0.0f32; rgb.len()];
    out.par_chunks_mut(width * 3)
        .enumerate()
        .for_each(|(y, row)| {
            let source = &rgb[y * width * 3..(y + 1) * width * 3];
            for (x, px) in row.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                let one = shared.then(|| trace(x, y, 0));
                for (c, v) in px.iter_mut().enumerate() {
                    *v = if !moved[c] {
                        source[x * 3 + c]
                    } else {
                        let (sx, sy) = one.unwrap_or_else(|| trace(x, y, c));
                        bilinear(rgb, width, height, c, sx, sy)
                    };
                }
            }
        });
    out
}

/// The warp's center and scale in the buffer's placement.
struct Geometry {
    at: WarpPlacement,
    cx: f64,
    cy: f64,
    radius: f64,
}

impl Geometry {
    fn new(at: WarpPlacement, warp: &Warp) -> Self {
        let cx = warp.center_x * at.image_width;
        let cy = warp.center_y * at.image_height;
        let far_x = cx.max(at.image_width - cx);
        let far_y = cy.max(at.image_height - cy);
        Self {
            at,
            cx,
            cy,
            radius: far_x.hypot(far_y).max(f64::MIN_POSITIVE),
        }
    }

    /// Where the buffer's point (`x`, `y`) is read from, in the
    /// buffer's coordinates.
    fn source(&self, x: f64, y: f64, plane: &WarpPlane, reciprocal: bool) -> (f64, f64) {
        let dx = (x + self.at.x - self.cx) / self.radius;
        let dy = (y + self.at.y - self.cy) / self.radius;
        let rr = (dx * dx + dy * dy).min(1.0);
        let ratio = ratio(plane, rr, reciprocal);
        let [kt0, kt1] = plane.tangential;
        let tan_x = kt1 * (rr + 2.0 * dx * dx) + 2.0 * kt0 * dx * dy;
        let tan_y = kt0 * (rr + 2.0 * dy * dy) + 2.0 * kt1 * dx * dy;
        let sx = self.cx + self.radius * (dx * ratio + tan_x) - self.at.x;
        let sy = self.cy + self.radius * (dy * ratio + tan_y) - self.at.y;
        if sx.is_finite() && sy.is_finite() {
            (sx, sy)
        } else {
            (x, y)
        }
    }
}

/// The radial polynomial's ratio of source to corrected radius at the
/// squared radius `rr`, the radius held to the plane's valid range.
fn ratio(plane: &WarpPlane, rr: f64, reciprocal: bool) -> f64 {
    let (lo, hi) = plane.valid;
    let r = rr.clamp(lo * lo, hi * hi).sqrt();
    let poly = plane.radial.iter().rev().fold(0.0, |acc, k| acc * r + k);
    if reciprocal { 1.0 / poly } else { poly }
}

/// Plane `c` of interleaved RGB read at (`x`, `y`), bilinear, held to
/// the picture's edge.
fn bilinear(rgb: &[f32], width: usize, height: usize, c: usize, x: f64, y: f64) -> f32 {
    let x = x.clamp(0.0, (width - 1) as f64);
    let y = y.clamp(0.0, (height - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let at = |x: usize, y: usize| rgb[(y * width + x) * 3 + c];
    let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
    let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
    top + (bottom - top) * fy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcodes::RADIAL_TERMS;

    fn whole(width: usize, height: usize) -> WarpPlacement {
        WarpPlacement {
            x: 0.0,
            y: 0.0,
            image_width: width as f64,
            image_height: height as f64,
        }
    }

    fn radial(terms: &[(usize, f64)]) -> WarpPlane {
        let mut plane = WarpPlane::identity();
        plane.radial = [0.0; RADIAL_TERMS];
        for &(power, k) in terms {
            plane.radial[power] = k;
        }
        plane
    }

    fn warp(planes: Vec<WarpPlane>) -> Warp {
        Warp {
            planes,
            center_x: 0.5,
            center_y: 0.5,
            reciprocal: false,
        }
    }

    /// A ramp in x on red, in y on green, and their sum on blue.
    fn ramps(width: usize, height: usize) -> Vec<f32> {
        (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| [x as f32, y as f32, (x + y) as f32]))
            .collect()
    }

    /// The identity moves nothing, to the bit.
    #[test]
    fn the_identity_moves_nothing() {
        let (w, h) = (9, 7);
        let rgb = ramps(w, h);
        let out = warp_rgb(
            &rgb,
            w,
            h,
            whole(w, h),
            &[warp(vec![WarpPlane::identity()])],
        );
        assert_eq!(out, rgb);
    }

    /// A constant ratio of one half reads every pixel from halfway to the
    /// center: on a ramp the value is the center's plus half the offset,
    /// exactly, since bilinear is exact on a ramp.
    #[test]
    fn a_ratio_of_a_half_reads_halfway_to_the_center() {
        let (w, h) = (10, 8);
        let rgb = ramps(w, h);
        let out = warp_rgb(&rgb, w, h, whole(w, h), &[warp(vec![radial(&[(0, 0.5)])])]);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                let sx = 5.0 + (x as f32 - 5.0) * 0.5;
                let sy = 4.0 + (y as f32 - 4.0) * 0.5;
                assert!((out[i] - sx).abs() < 1e-5, "{x},{y}: {} {sx}", out[i]);
                assert!((out[i + 1] - sy).abs() < 1e-5);
                assert!((out[i + 2] - sx - sy).abs() < 1e-4);
            }
        }
    }

    /// The polynomial is in the radius relative to the farthest corner:
    /// with f(r) = 1 + r^2, a corner pixel reads from twice as far out,
    /// held at the edge, and the center does not move.
    #[test]
    fn the_radius_is_relative_to_the_farthest_corner() {
        let (w, h) = (8, 6);
        let at = whole(w, h);
        let g = Geometry::new(at, &warp(vec![radial(&[(0, 1.0), (2, 1.0)])]));
        let plane = radial(&[(0, 1.0), (2, 1.0)]);
        assert_eq!(g.radius, 5.0);
        assert_eq!(g.source(4.0, 3.0, &plane, false), (4.0, 3.0));
        // The pixel at (0, 0) is 4 and 3 off the center, r = 1.
        assert_eq!(g.source(0.0, 0.0, &plane, false), (-4.0, -3.0));
        // A reciprocal of the same polynomial reads from halfway.
        assert_eq!(g.source(0.0, 0.0, &plane, true), (2.0, 1.5));
    }

    /// The tangential terms as the SDK writes them: kt1 moves along x by
    /// r^2 + 2 dx^2, kt0 along y by r^2 + 2 dy^2.
    #[test]
    fn the_tangential_terms_follow_the_sdk() {
        let (w, h) = (8, 6);
        let mut plane = WarpPlane::identity();
        plane.tangential = [0.0, 0.1];
        let g = Geometry::new(whole(w, h), &warp(vec![plane.clone()]));
        // dx = 0.8, dy = 0 at (8, 3): r^2 = 0.64, tan_x = 0.1 * 1.92.
        let (sx, sy) = g.source(8.0, 3.0, &plane, false);
        assert!((sx - (8.0 + 5.0 * 0.192)).abs() < 1e-12, "{sx}");
        assert!((sy - 3.0).abs() < 1e-12, "{sy}");
    }

    /// A set a plane moves each plane by its own: red scaled, green and
    /// blue left.
    #[test]
    fn three_sets_move_each_plane_by_its_own() {
        let (w, h) = (10, 8);
        let rgb = ramps(w, h);
        let sets = vec![
            radial(&[(0, 0.5)]),
            WarpPlane::identity(),
            WarpPlane::identity(),
        ];
        let out = warp_rgb(&rgb, w, h, whole(w, h), &[warp(sets)]);
        let i = 3; // (1, 0)
        assert!((out[i] - 3.0).abs() < 1e-5, "{}", out[i]);
        assert_eq!(out[i + 1], rgb[i + 1]);
        assert_eq!(out[i + 2], rgb[i + 2]);
    }

    /// A crop warps as the same part of the whole: placed by its corner,
    /// its pixels are the whole's, away from the edge the crop clamps.
    #[test]
    fn a_crop_warps_as_its_part_of_the_whole() {
        let (w, h) = (24, 18);
        let rgb = ramps(w, h);
        let lens = warp(vec![radial(&[(0, 0.97), (2, 0.05), (4, -0.02)])]);
        let full = warp_rgb(&rgb, w, h, whole(w, h), std::slice::from_ref(&lens));
        let (cx, cy, cw, ch) = (3, 2, 18, 14);
        let crop: Vec<f32> = (cy..cy + ch)
            .flat_map(|y| rgb[(y * w + cx) * 3..(y * w + cx + cw) * 3].to_vec())
            .collect();
        let at = WarpPlacement {
            x: cx as f64,
            y: cy as f64,
            ..whole(w, h)
        };
        let out = warp_rgb(&crop, cw, ch, at, std::slice::from_ref(&lens));
        for y in 3..ch - 3 {
            for x in 3..cw - 3 {
                for c in 0..3 {
                    let a = out[(y * cw + x) * 3 + c];
                    let b = full[((y + cy) * w + x + cx) * 3 + c];
                    assert!((a - b).abs() < 1e-4, "{x},{y},{c}: {a} {b}");
                }
            }
        }
    }

    /// A bin of two places its pixels at the middle of their blocks, so
    /// a binned ramp warps as the full one does, scaled.
    #[test]
    fn a_binned_buffer_sits_at_its_blocks_middles() {
        let area = MapPlacement {
            x: 2,
            y: 4,
            image_width: 100,
            image_height: 80,
        };
        let at = WarpPlacement::binned(area, 2);
        assert_eq!((at.x, at.y), (1.25, 2.25));
        assert_eq!((at.image_width, at.image_height), (50.0, 40.0));
        assert_eq!(WarpPlacement::binned(area, 1).x, 2.0);
    }

    /// Two warps are one resample that traces each pixel through the
    /// second and then the first: on a ramp of pixel coordinates the
    /// same as running them one after the other to a hundredth of a
    /// pixel (the second's read of the first's picture interpolates a
    /// map that is not linear, which the trace does not), and a
    /// per-plane set is traced for its own plane.
    #[test]
    fn two_warps_read_once_as_one_after_the_other() {
        let (w, h) = (20, 16);
        let rgb = ramps(w, h);
        let mut first = warp(vec![radial(&[(0, 0.9), (2, 0.08)])]);
        first.center_x = 0.45;
        let second = warp(vec![
            radial(&[(0, 0.95)]),
            WarpPlane::identity(),
            radial(&[(0, 1.02), (4, -0.03)]),
        ]);
        let both = [first.clone(), second.clone()];
        let once = warp_rgb(&rgb, w, h, whole(w, h), &both);
        let step = warp_rgb(&rgb, w, h, whole(w, h), std::slice::from_ref(&first));
        let twice = warp_rgb(&step, w, h, whole(w, h), std::slice::from_ref(&second));
        for (i, (a, b)) in once.iter().zip(&twice).enumerate() {
            assert!((a - b).abs() < 1e-2, "{i}: {a} {b}");
        }
        let mut applied = rgb.clone();
        apply_warps(&mut applied, w, h, whole(w, h), &both);
        assert_eq!(applied, once);
    }

    /// A polynomial that reaches zero is a source at infinity, which
    /// reads the pixel where it is rather than a NaN.
    #[test]
    fn a_degenerate_polynomial_leaves_the_pixel() {
        let (w, h) = (6, 6);
        let rgb = ramps(w, h);
        let mut lens = warp(vec![radial(&[(0, 0.0)])]);
        lens.reciprocal = true;
        let out = warp_rgb(&rgb, w, h, whole(w, h), &[lens]);
        assert!(out.iter().all(|v| v.is_finite()));
    }
}
