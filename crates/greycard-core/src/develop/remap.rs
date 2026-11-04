//! A DNG's sample maps on the mosaic: `OpcodeList2`'s `MapTable` and
//! `MapPolynomial` opcodes, which run on the linear samples after the
//! black level and before the demosaic. Lightroom's merges (a panorama,
//! an HDR) are linear DNGs whose tone and color sit in three of them,
//! one a plane; without them the picture is magenta and clipped.
//!
//! The curve goes on every sample of the area, the planes, and the
//! pitches the opcode names, as the DNG SDK's `dng_opcode_MapTable` and
//! `dng_opcode_MapPolynomial` run it on their 16-bit stage, the result
//! held to the range 0 to 1.

use rayon::prelude::*;

use super::gain_map::MapPlacement;
use crate::opcodes::Remap;

/// Run `map`'s curve over `samples`, `cpp` interleaved planes a pixel,
/// `width` pixels a row, placed in the list's image by `at`. A sample
/// outside the map's area, planes, or pitches is left as it was.
/// Answers how many samples the curve moved.
pub fn apply_remap(
    samples: &mut [f32],
    width: usize,
    cpp: usize,
    at: MapPlacement,
    map: &Remap,
) -> usize {
    let (top, left) = (map.top as usize, map.left as usize);
    let (bottom, right) = (map.bottom as usize, map.right as usize);
    let (row_pitch, col_pitch) = (map.row_pitch as usize, map.col_pitch as usize);
    let planes = (map.plane as usize)..(map.plane as usize + map.planes as usize).min(cpp);
    if width == 0 || planes.is_empty() || row_pitch == 0 || col_pitch == 0 {
        return 0;
    }
    let columns: Vec<usize> = (0..width)
        .filter(|&x| {
            let c = x + at.x;
            c >= left && c < right && (c - left).is_multiple_of(col_pitch)
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
            let mut moved = 0;
            for plane in planes.clone() {
                for &x in &columns {
                    let s = &mut row[x * cpp + plane];
                    let v = map.curve.at(*s);
                    moved += usize::from(v != *s);
                    *s = v;
                }
            }
            moved
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcodes::{Curve, whole_remap};

    fn placed(width: usize, height: usize) -> MapPlacement {
        MapPlacement {
            x: 0,
            y: 0,
            image_width: width,
            image_height: height,
        }
    }

    /// A polynomial on one plane of three leaves the others, and its
    /// result is held at the white.
    #[test]
    fn a_curve_goes_on_the_planes_it_names() {
        let mut samples = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let map = whole_remap(1, 1, Curve::Polynomial(vec![0.0, 3.0]));
        let moved = apply_remap(&mut samples, 2, 3, placed(2, 1), &map);
        assert_eq!(moved, 2);
        let want = [0.1, 0.6, 0.3, 0.4, 1.0, 0.6];
        for (s, w) in samples.iter().zip(want) {
            assert!((s - w).abs() < 1e-6, "{samples:?} != {want:?}");
        }
    }

    /// The area and the pitches are read in the list's image, the
    /// buffer placed by `at`: a map of every second column from column
    /// 1 touches the buffer's column 0 when the buffer starts at 1.
    #[test]
    fn the_area_and_pitches_are_in_the_images_coordinates() {
        let mut samples = vec![0.5; 8];
        let mut map = whole_remap(0, 1, Curve::Table(vec![0, 65535]));
        (map.top, map.left, map.bottom, map.right) = (0, 1, 1, 5);
        map.col_pitch = 2;
        let at = MapPlacement {
            x: 1,
            y: 0,
            image_width: 8,
            image_height: 2,
        };
        let moved = apply_remap(&mut samples, 4, 1, at, &map);
        assert_eq!(moved, 2);
        assert_eq!(samples, [1.0, 0.5, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5]);
    }

    /// A map of planes the buffer has not is left, and a count past
    /// the buffer's planes is cut to them.
    #[test]
    fn the_planes_are_cut_to_the_buffers() {
        let mut samples = vec![0.25; 6];
        let past = whole_remap(3, 1, Curve::Polynomial(vec![1.0]));
        assert_eq!(apply_remap(&mut samples, 2, 3, placed(2, 1), &past), 0);
        assert_eq!(samples, [0.25; 6]);
        let huge = whole_remap(1, u32::MAX, Curve::Polynomial(vec![1.0]));
        assert_eq!(apply_remap(&mut samples, 2, 3, placed(2, 1), &huge), 4);
        assert_eq!(samples, [0.25, 1.0, 1.0, 0.25, 1.0, 1.0]);
    }

    /// An identity table moves nothing.
    #[test]
    fn an_identity_table_moves_nothing() {
        let table: Vec<u16> = (0..=65535).collect();
        let mut samples: Vec<f32> = (0..16).map(|i| i as f32 / 15.0).collect();
        let before = samples.clone();
        let map = whole_remap(0, 1, Curve::Table(table));
        let moved = apply_remap(&mut samples, 4, 1, placed(4, 4), &map);
        assert_eq!(moved, 0);
        assert_eq!(samples, before);
    }
}
