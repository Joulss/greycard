//! A DNG's gain maps on the mosaic: `OpcodeList2`'s `GainMap` opcodes,
//! which run on linear samples after the black level and before the
//! demosaic. A phone corrects its lens shading with them, one map a
//! color of the mosaic.
//!
//! The interpolation follows the DNG specification and Adobe's DNG SDK
//! (`dng_gain_map`): each pixel's center placed in relative coordinates
//! over the whole image the list runs on, bilinear between the map's
//! points, and held at the edge points past them. The product is held
//! at 1, the white, as the SDK holds it, so a sample the sensor clipped
//! is still clipped after its gain and the highlight pass still sees it.

use rayon::prelude::*;

use crate::opcodes::GainMap;

/// Where a buffer sits in the image the maps are defined over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPlacement {
    /// The buffer's top left sample, in the image's coordinates.
    pub x: usize,
    pub y: usize,
    /// The image's size: what relative coordinate 1 is.
    pub image_width: usize,
    pub image_height: usize,
}

/// Multiply the gain maps into `samples`, `cpp` interleaved planes a
/// pixel, `width` pixels a row, placed in the maps' image by `at`. The
/// maps run in order, each on the planes and the rows and columns it
/// names; a sample outside every map's area is left as it was. Answers
/// how many samples a gain took past the white and were held there.
pub fn apply_gain_maps(
    samples: &mut [f32],
    width: usize,
    cpp: usize,
    at: MapPlacement,
    maps: &[GainMap],
) -> usize {
    if width == 0 || cpp == 0 || at.image_width == 0 || at.image_height == 0 {
        return 0;
    }
    maps.iter()
        .map(|map| apply_one(samples, width, cpp, at, map))
        .sum()
}

fn apply_one(
    samples: &mut [f32],
    width: usize,
    cpp: usize,
    at: MapPlacement,
    map: &GainMap,
) -> usize {
    let (top, left) = (map.top as usize, map.left as usize);
    let (bottom, right) = (map.bottom as usize, map.right as usize);
    let (row_pitch, col_pitch) = (map.row_pitch as usize, map.col_pitch as usize);
    let planes = (map.plane as usize)..(map.plane as usize + map.planes as usize).min(cpp);
    if planes.is_empty() || row_pitch == 0 || col_pitch == 0 {
        return 0;
    }
    // Each column's place in the map, worked out once for every row.
    let columns: Vec<(usize, Axis)> = (0..width)
        .filter_map(|x| {
            let c = x + at.x;
            let on = c >= left && c < right && (c - left).is_multiple_of(col_pitch);
            on.then(|| {
                let rel = (c as f64 + 0.5) / at.image_width as f64;
                (x, Axis::at(rel, map.origin_h, map.spacing_h, map.points_h))
            })
        })
        .collect();
    if columns.is_empty() {
        return 0;
    }
    samples
        .par_chunks_mut(width * cpp)
        .enumerate()
        .map(|(y, row)| {
            let r = y + at.y;
            if r < top || r >= bottom || !(r - top).is_multiple_of(row_pitch) {
                return 0;
            }
            let mut held = 0;
            let rel = (r as f64 + 0.5) / at.image_height as f64;
            let v = Axis::at(rel, map.origin_v, map.spacing_v, map.points_v);
            for plane in planes.clone() {
                // The SDK's choice: a plane past the map's takes its last.
                let mp = plane.min(map.map_planes as usize - 1);
                for &(x, h) in &columns {
                    let gain = lerp(
                        lerp(map.point(v.i0, h.i0, mp), map.point(v.i0, h.i1, mp), h.t),
                        lerp(map.point(v.i1, h.i0, mp), map.point(v.i1, h.i1, mp), h.t),
                        v.t,
                    );
                    let s = &mut row[x * cpp + plane];
                    let v = *s * gain;
                    held += usize::from(v > 1.0);
                    // Held at 0 too: a file's negative gain is damage,
                    // and the samples stay where the develop expects.
                    *s = v.clamp(0.0, 1.0);
                }
            }
            held
        })
        .sum()
}

/// A position between two of a map's points along one axis.
#[derive(Debug, Clone, Copy)]
struct Axis {
    i0: usize,
    i1: usize,
    t: f32,
}

impl Axis {
    /// The points either side of relative coordinate `rel`, and how far
    /// along it is; the first or last point alone outside the grid.
    fn at(rel: f64, origin: f64, spacing: f64, points: u32) -> Self {
        let last = points.saturating_sub(1) as usize;
        let f = if points > 1 {
            (rel - origin) / spacing
        } else {
            0.0
        };
        if f.is_nan() || f <= 0.0 {
            return Self {
                i0: 0,
                i1: 0,
                t: 0.0,
            };
        }
        if f >= last as f64 {
            return Self {
                i0: last,
                i1: last,
                t: 0.0,
            };
        }
        let i0 = f.floor() as usize;
        Self {
            i0,
            i1: i0 + 1,
            t: (f - i0 as f64) as f32,
        }
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcodes::even_map;

    fn whole(width: usize, height: usize) -> MapPlacement {
        MapPlacement {
            x: 0,
            y: 0,
            image_width: width,
            image_height: height,
        }
    }

    /// A map of 2 everywhere doubles the plane it names, and only the
    /// rows and columns its pitch picks: a map for the red of an RGGB
    /// mosaic leaves the greens and the blue alone.
    #[test]
    fn a_map_of_two_doubles_its_plane() {
        let (w, h) = (8, 6);
        let mut flat = vec![0.25f32; w * h];
        apply_gain_maps(
            &mut flat,
            w,
            1,
            whole(w, h),
            &[even_map(8, 6, 3, 3, |_, _| 2.0)],
        );
        assert!(flat.iter().all(|&s| s == 0.5), "{flat:?}");

        let mut mosaic = vec![0.25f32; w * h];
        let mut red = even_map(8, 6, 3, 3, |_, _| 2.0);
        (red.row_pitch, red.col_pitch) = (2, 2);
        apply_gain_maps(&mut mosaic, w, 1, whole(w, h), &[red]);
        for y in 0..h {
            for x in 0..w {
                let want = if y % 2 == 0 && x % 2 == 0 { 0.5 } else { 0.25 };
                assert_eq!(mosaic[y * w + x], want, "({x}, {y})");
            }
        }

        // Interleaved RGB: plane 1 alone.
        let mut rgb = vec![0.25f32; w * h * 3];
        let mut green = even_map(8, 6, 2, 2, |_, _| 2.0);
        green.plane = 1;
        apply_gain_maps(&mut rgb, w, 3, whole(w, h), &[green]);
        assert!(rgb.chunks(3).all(|p| p == [0.25, 0.5, 0.25]));
    }

    /// A map rising toward one corner brightens that corner and leaves
    /// the opposite one, bilinear between: the center takes the mean of
    /// the four points, and the product is held at the white.
    #[test]
    fn a_gradient_brightens_the_corner() {
        let (w, h) = (101, 101);
        let mut s = vec![0.25f32; w * h];
        let map = even_map(
            101,
            101,
            2,
            2,
            |v, hh| if v == 1 && hh == 1 { 3.0 } else { 1.0 },
        );
        apply_gain_maps(&mut s, w, 1, whole(w, h), &[map]);
        let at = |x: usize, y: usize| s[y * w + x];
        assert!((at(0, 0) - 0.25).abs() < 0.01, "{}", at(0, 0));
        assert!((at(100, 100) - 0.74).abs() < 0.01, "{}", at(100, 100));
        assert!((at(50, 50) - 0.25 * 1.5).abs() < 1e-4, "{}", at(50, 50));
        assert!(at(100, 50) > at(50, 50) && at(50, 50) > at(0, 50));

        let mut bright = vec![0.5f32, 0.5, 0.5, 0.2];
        let held = apply_gain_maps(
            &mut bright,
            2,
            1,
            whole(2, 2),
            &[even_map(2, 2, 1, 1, |_, _| 3.0)],
        );
        assert_eq!(bright[..3], [1.0; 3]);
        assert!((bright[3] - 0.6).abs() < 1e-6, "{}", bright[3]);
        assert_eq!(held, 3);
    }

    /// A buffer cut out of the image takes the gains of where it sits
    /// in the image, not of where it starts.
    #[test]
    fn a_crop_takes_the_gains_of_its_place() {
        let (w, h) = (10, 10);
        let map = even_map(10, 10, 2, 2, |_, hh| if hh == 1 { 2.0 } else { 1.0 });
        let mut full = vec![0.25f32; w * h];
        apply_gain_maps(&mut full, w, 1, whole(w, h), std::slice::from_ref(&map));
        let mut crop = vec![0.25f32; 4 * 3];
        let at = MapPlacement {
            x: 5,
            y: 2,
            image_width: w,
            image_height: h,
        };
        apply_gain_maps(&mut crop, 4, 1, at, &[map]);
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(crop[y * 4 + x], full[(y + 2) * w + x + 5]);
            }
        }
    }
}
