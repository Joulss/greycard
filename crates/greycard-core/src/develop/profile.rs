//! The camera profile's hue/saturation/value map, after the matrix.
//!
//! DNG defines the map in HSV of linear ProPhoto: camera to XYZ under
//! the scene illuminant, XYZ to ProPhoto, RGB to HSV, the map's hue
//! shift, saturation scale and value scale, back. The engine's matrix
//! lands in linear Rec.2020 instead (`color::WORKING_SPACE`), and
//! Rec.2020 to ProPhoto through Bradford undoes exactly the adaptation
//! the camera matrix did on the way in, so the stage is the same three
//! steps with one matrix pair in front of them.
//!
//! `ProfileHueSatMapEncoding` spaces out the table's value axis and
//! nothing else: the hue and the saturation are read in linear space
//! whatever it says, and a map with no value axis is not encoded at
//! all. Encoding the RGB before the conversion instead would move
//! every lookup's hue and saturation as well.
//!
//! The look table is the same lattice and goes through the same stage,
//! after the map, with two differences ([`MapStage::look`]). DNG puts it
//! after the exposure, so it is read where the profile's
//! `BaselineExposureOffset` puts the picture: the pixel is scaled by it
//! for the lookup and scaled back after, which reads the table where
//! Adobe does without moving the picture's brightness, which is the
//! Look's. And a saturation the table raises stops at ProPhoto's edge,
//! where DNG clips it: a look table is display referred, and its
//! largest scales (above 4 in some of Adobe's) would otherwise throw
//! colors far outside any gamut. A color already outside ProPhoto is
//! never pushed further out, nor pulled in.
//!
//! Written from the DNG 1.7.1.0 specification; see [`crate::dcp`] for
//! what was read alongside it.

use rayon::prelude::*;

use crate::color::{CAT, WORKING_SPACE, apply3, srgb_decode, srgb_encode};
use crate::dcp::{Encoding, HueSatMap};
use crate::error::{Error, Result};
use crate::image::WorkingImage;
use rawcolor::RgbSpace;

/// The most stops a look table is read above or below the develop:
/// a body's baseline and a profile's offset are each a stop or two, and
/// 20 is past any sum of real ones while 2^20 still scales an f32 pixel
/// by a finite, non-zero amount either way.
pub const MAX_LOOK_STOPS: f32 = 20.0;

/// A map ready to run on a picture: the interpolated table and the
/// matrices that take the working space to ProPhoto and back.
#[derive(Debug, Clone, PartialEq)]
pub struct MapStage {
    pub map: HueSatMap,
    to_prophoto: [[f32; 3]; 3],
    from_prophoto: [[f32; 3]; 3],
    /// Whether the value axis is sRGB encoded: the encoding is the
    /// axis's own, so a map without one is never encoded.
    encoded: bool,
    /// What the ProPhoto pixel is multiplied by for the lookup and
    /// divided by after: 1 for a hue/sat map, the profile's exposure
    /// for a look table.
    exposure: f32,
    /// Whether a raised saturation stops at ProPhoto's edge: a look
    /// table's, not a map's.
    hold_gamut: bool,
}

impl MapStage {
    pub fn new(map: HueSatMap) -> Result<Self> {
        let to = WORKING_SPACE
            .to_space_matrix(&RgbSpace::PROPHOTO, CAT)
            .ok_or_else(|| Error::Profile("the working space does not reach ProPhoto".into()))?;
        let from = RgbSpace::PROPHOTO
            .to_space_matrix(&WORKING_SPACE, CAT)
            .ok_or_else(|| Error::Profile("ProPhoto does not reach the working space".into()))?;
        Ok(MapStage {
            encoded: map.encoding == Encoding::Srgb && map.val_divisions > 1,
            map,
            to_prophoto: to.rows.map(|row| row.map(|v| v as f32)),
            from_prophoto: from.rows.map(|row| row.map(|v| v as f32)),
            exposure: 1.0,
            hold_gamut: false,
        })
    }

    /// A look table, read with the picture `stops` brighter than it is
    /// (the profile's `BaselineExposureOffset`), and its saturation held
    /// inside ProPhoto (the module's doc says why).
    ///
    /// `stops` that are not a number read the table at the develop, and
    /// any past [`MAX_LOOK_STOPS`] are held there: the offset is checked
    /// where the file is read, but the consumer's baseline comes from a
    /// sidecar, and a scale of infinity or zero would make every pixel
    /// NaN.
    pub fn look(table: HueSatMap, stops: f32) -> Result<Self> {
        let stops = if stops.is_finite() {
            stops.clamp(-MAX_LOOK_STOPS, MAX_LOOK_STOPS)
        } else {
            0.0
        };
        let mut stage = Self::new(table)?;
        stage.exposure = stops.exp2();
        stage.hold_gamut = true;
        Ok(stage)
    }

    /// One working-space pixel through the map.
    ///
    /// The saturation and the value come out unclipped, where the
    /// specification clips both to 1. That is deliberate and is the
    /// one place this departs from it: the engine is scene referred,
    /// its working space is wider than ProPhoto in places and its
    /// highlights go above 1, so clipping the saturation would gamut
    /// map every color outside ProPhoto into it and clipping the value
    /// would throw away reconstructed highlights, both in the middle of
    /// a pipeline whose whole point is to keep them until the tone
    /// mapping. RawTherapee leaves them unclipped for the same reason.
    /// A look table holds a raised saturation at ProPhoto's edge
    /// ([`MapStage::look`]); its value is unclipped as a map's is.
    #[inline]
    pub fn pixel(&self, rgb: [f32; 3]) -> [f32; 3] {
        let p = apply3(&self.to_prophoto, rgb).map(|c| c * self.exposure);
        let (h, s, v) = to_hsv(p);
        // Black, or a color so far outside ProPhoto that its largest
        // channel is negative: there is no hue to shift and no
        // saturation to scale, so it is left as it is.
        if v <= 0.0 {
            return rgb;
        }
        // The encoding is the value axis's alone: the hue and the
        // saturation are always read in linear space, and a map with no
        // value axis (`val_divisions` of 1) is not encoded at all,
        // since there is no axis for the encoding to space out. The
        // scale multiplies the encoded value, which is then decoded.
        let value = if self.encoded { encode(v) } else { v };
        let [hue_shift, sat_scale, val_scale] = self.map.lookup(h, s, value);
        let value = value * val_scale;
        let value = if self.encoded { decode(value) } else { value };
        let mut sat = s * sat_scale;
        if self.hold_gamut {
            // Raised no further than ProPhoto's edge, or than where the
            // color already was if it was past it.
            sat = sat.min(s.max(1.0));
        }
        let p = from_hsv(h + hue_shift, sat, value).map(|c| c / self.exposure);
        apply3(&self.from_prophoto, p)
    }

    /// The map over a whole picture, in place.
    pub fn apply(&self, image: &mut WorkingImage) {
        let width = image.width;
        image
            .data
            .par_chunks_mut(width * WorkingImage::CHANNELS)
            .for_each(|row| {
                for px in row.as_chunks_mut::<{ WorkingImage::CHANNELS }>().0 {
                    *px = self.pixel(*px);
                }
            });
    }
}

/// sRGB's curve on the value, which is positive by the time it is
/// asked for. Above 1 the formula carries on and inverts, so a
/// highlight is spaced out rather than clipped.
#[inline]
fn encode(v: f32) -> f32 {
    srgb_encode(v)
}

#[inline]
fn decode(v: f32) -> f32 {
    srgb_decode(v)
}

/// RGB to hue in degrees, saturation and value.
///
/// Nothing is clamped: a color outside the space has a channel below
/// zero, which comes out as a saturation above one, and [`from_hsv`]
/// puts it back where it was. Clamping here would quietly gamut-map
/// every such pixel, which is not the map's job.
#[inline]
fn to_hsv(rgb: [f32; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let span = max - min;
    if span == 0.0 || max == 0.0 {
        return (0.0, 0.0, max);
    }
    let hue = if max == r {
        (g - b) / span
    } else if max == g {
        2.0 + (b - r) / span
    } else {
        4.0 + (r - g) / span
    };
    (hue.rem_euclid(6.0) * 60.0, span / max, max)
}

/// The way back.
///
/// The sector is taken modulo six rather than trusted: `rem_euclid`
/// answers exactly 360 for a hue a hair below zero, which would
/// otherwise land in the last arm with no fraction and turn a red a
/// shade under 0 degrees magenta.
#[inline]
fn from_hsv(hue: f32, sat: f32, val: f32) -> [f32; 3] {
    let h = hue.rem_euclid(360.0) / 60.0;
    let i = h.floor();
    let f = h - i;
    // 0 to 6 by construction, and 6 only where `rem_euclid` rounded a
    // hair below zero up to 360; that is sector 0 with no fraction.
    let i = i as i32;
    let i = if i >= 6 { 0 } else { i };
    let p = val * (1.0 - sat);
    let q = val * (1.0 - sat * f);
    let t = val * (1.0 - sat * (1.0 - f));
    match i {
        0 => [val, t, p],
        1 => [q, val, p],
        2 => [p, val, t],
        3 => [p, q, val],
        4 => [t, p, val],
        _ => [val, p, q],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dcp::HueSatMap;

    fn identity_stage() -> MapStage {
        MapStage::new(HueSatMap::identity(6, 3, 1)).unwrap()
    }

    #[test]
    fn hsv_round_trips_including_colors_outside_the_space() {
        for c in [
            [0.18f32, 0.05, 0.4],
            [1.2, 0.9, 0.1],
            [0.5, 0.5, 0.5],
            [-0.02, 0.3, 0.6],
            [0.0, 0.0, 0.0],
        ] {
            let (h, s, v) = to_hsv(c);
            let back = from_hsv(h, s, v);
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 1e-6, "{c:?} came back {back:?}");
            }
        }
    }

    #[test]
    fn an_identity_map_leaves_a_pixel_alone() {
        let stage = identity_stage();
        for c in [
            [0.18f32, 0.05, 0.4],
            [1.2, 0.9, 0.1],
            [0.5, 0.5, 0.5],
            [0.0, 0.0, 0.0],
        ] {
            let out = stage.pixel(c);
            for k in 0..3 {
                assert!((out[k] - c[k]).abs() < 1e-5, "{c:?} became {out:?}");
            }
        }
    }

    #[test]
    fn an_srgb_encoded_identity_map_leaves_a_pixel_alone() {
        let mut map = HueSatMap::identity(6, 3, 3);
        map.encoding = Encoding::Srgb;
        let stage = MapStage::new(map).unwrap();
        for c in [[0.18f32, 0.05, 0.4], [1.2, 0.9, 0.1], [-0.02, 0.3, 0.6]] {
            let out = stage.pixel(c);
            for k in 0..3 {
                assert!((out[k] - c[k]).abs() < 1e-5, "{c:?} became {out:?}");
            }
        }
    }

    /// The value axis of an sRGB-encoded map is indexed by the encoded
    /// value, and the scale multiplies it there.
    ///
    /// An identity map cancels the encoding whatever is done with it,
    /// so this one is not one: its three value slices scale by 1, 2 and
    /// 3. The pixel is neutral, which the working space and ProPhoto
    /// agree on (both adapt to their own white), so its ProPhoto value
    /// is the number chosen here and its hue and saturation say
    /// nothing.
    #[test]
    fn an_srgb_encoded_map_is_indexed_by_the_encoded_value() {
        let mut map = HueSatMap::identity(2, 2, 3);
        map.encoding = Encoding::Srgb;
        for (i, e) in map.data.iter_mut().enumerate() {
            // Value slowest: four entries a slice.
            e[2] = 1.0 + (i / 4) as f32;
        }
        let stage = MapStage::new(map.clone()).unwrap();

        // The linear value whose sRGB encoding is exactly 0.5, which is
        // the middle slice's node: scale 2, so the encoded value
        // becomes 1.0 and comes back as linear 1.0.
        let v = srgb_decode(0.5);
        let out = stage.pixel([v, v, v]);
        for c in out {
            assert!((c - 1.0).abs() < 1e-5, "{v} became {out:?}, wanted 1.0");
        }

        // A quarter of the way up the axis: encoded 0.25, between the
        // first two slices, so the scale is 1.5 and the answer is
        // linear again after the decode.
        let v = srgb_decode(0.25);
        let out = stage.pixel([v, v, v]);
        let want = srgb_decode(0.25 * 1.5);
        for c in out {
            assert!((c - want).abs() < 1e-5, "{v} became {out:?}, wanted {want}");
        }

        // The same map read as linear indexes by the linear value
        // instead, which is a different slice and a different answer.
        map.encoding = Encoding::Linear;
        let linear = MapStage::new(map).unwrap().pixel([v, v, v]);
        assert!(
            (linear[0] - out[0]).abs() > 0.05,
            "the encoding made no difference: {linear:?} against {out:?}"
        );
    }

    /// The encoding does not move where the table is read in hue and
    /// saturation.
    ///
    /// A neutral pixel cannot catch this, since encoding three equal
    /// channels is the same as encoding the value; this one is a
    /// saturated color read against a map that varies with saturation
    /// alone, so reading it at the encoded saturation lands on a
    /// different entry and the answer differs.
    #[test]
    fn an_srgb_encoding_does_not_move_the_hue_and_saturation_lookup() {
        // One hue, two saturations, two values, the value slices alike:
        // the value scale is 1 at saturation 0 and 2 at saturation 1.
        let mut map = HueSatMap::identity(1, 2, 2);
        map.encoding = Encoding::Srgb;
        for (i, e) in map.data.iter_mut().enumerate() {
            e[2] = 1.0 + (i % 2) as f32;
        }
        let stage = MapStage::new(map).unwrap();

        let prophoto = [0.4f32, 0.1, 0.05];
        let out = apply3(
            &stage.to_prophoto,
            stage.pixel(apply3(&stage.from_prophoto, prophoto)),
        );

        // By hand: value 0.4, saturation (0.4 - 0.05) / 0.4 = 0.875 in
        // linear space, so the scale is 1 + 0.875. It multiplies the
        // encoded value, which is then decoded; the hue and the
        // saturation are untouched, so every channel takes the same
        // factor.
        let sat = 0.875f32;
        let scale = 1.0 + sat;
        let want = srgb_decode(srgb_encode(0.4) * scale) / 0.4;
        for (got, p) in out.iter().zip(prophoto) {
            assert!((got - p * want).abs() < 1e-4, "{out:?} is not {want}x");
        }

        // Reading the table at the encoded saturation instead would
        // have landed at 0.62 and scaled by 1.62, which is a different
        // picture; this keeps that from creeping back.
        let wrong = srgb_decode(srgb_encode(0.4) * (1.0 + srgb_encode(sat))) / 0.4;
        assert!((want - wrong).abs() > 0.05, "{want} against {wrong}");
    }

    /// A map with no value axis is not encoded at all: the scale
    /// multiplies the linear value, as it does for a linear map.
    #[test]
    fn an_srgb_encoded_map_with_no_value_axis_is_not_encoded() {
        let mut map = HueSatMap::identity(4, 2, 1);
        map.encoding = Encoding::Srgb;
        for e in &mut map.data {
            e[2] = 0.5;
        }
        let stage = MapStage::new(map).unwrap();
        let out = stage.pixel([0.4, 0.2, 0.1]);
        for (got, want) in out.iter().zip([0.2, 0.1, 0.05]) {
            assert!((got - want).abs() < 1e-5, "{out:?} is not half");
        }
    }

    /// The hue shift is in degrees, and the table's hue axis is 360
    /// degrees wide. Taking either for the 0-to-6 of the HSV sector is
    /// the classic mistake, so this pins both with a hand-computed
    /// answer.
    #[test]
    fn a_hue_shift_turns_the_color_by_that_many_degrees() {
        // Pure red in ProPhoto: hue 0, saturation 1. Every node shifts
        // by 120 degrees, so it comes out pure green.
        let mut map = HueSatMap::identity(4, 2, 1);
        for e in &mut map.data {
            e[0] = 120.0;
        }
        let stage = MapStage::new(map).unwrap();
        let red = apply3(&stage.from_prophoto, [1.0, 0.0, 0.0]);
        let out = apply3(&stage.to_prophoto, stage.pixel(red));
        for (got, want) in out.iter().zip([0.0, 1.0, 0.0]) {
            assert!((got - want).abs() < 1e-5, "{out:?} is not green");
        }

        // And a shift that lands between two hue nodes of the table is
        // still read at the color's own hue: a 45 degree shift on a
        // hue-60 color (yellow) gives hue 105.
        let mut map = HueSatMap::identity(4, 2, 1);
        for e in &mut map.data {
            e[0] = 45.0;
        }
        let stage = MapStage::new(map).unwrap();
        let yellow = apply3(&stage.from_prophoto, [1.0, 1.0, 0.0]);
        let out = apply3(&stage.to_prophoto, stage.pixel(yellow));
        let (h, s, v) = to_hsv(out);
        assert!((h - 105.0).abs() < 1e-2, "hue {h}");
        assert!((s - 1.0).abs() < 1e-5 && (v - 1.0).abs() < 1e-5, "{s} {v}");
    }

    /// A hue a hair below zero is red, not magenta: the sector index
    /// must wrap where `rem_euclid` rounds up to 360.
    #[test]
    fn a_hue_just_below_zero_comes_back_red() {
        let out = from_hsv(-1e-7, 1.0, 1.0);
        assert_eq!(out[0], 1.0, "{out:?}");
        assert!(out[1] < 1e-5 && out[2] < 1e-5, "{out:?}");
    }

    /// A look table's exposure moves where the table is read and
    /// nothing else: an identity table read a stop and a half down
    /// leaves the pixel as it was.
    #[test]
    fn a_look_tables_exposure_is_undone() {
        let stage = MapStage::look(HueSatMap::identity(6, 3, 4), -1.5).unwrap();
        for c in [[0.18f32, 0.05, 0.4], [1.2, 0.9, 0.1], [0.5, 0.5, 0.5]] {
            let out = stage.pixel(c);
            for k in 0..3 {
                assert!((out[k] - c[k]).abs() < 1e-5, "{c:?} became {out:?}");
            }
        }
    }

    /// The table is read at the value the exposure puts the pixel at:
    /// a linear table whose value scale is 1 at the bottom and 2 at the
    /// top, read a stop up, scales a neutral 0.25 by the 1.5 of its
    /// middle, where read as it is it would take 1.25.
    #[test]
    fn a_look_table_is_read_at_its_exposure() {
        let mut table = HueSatMap::identity(1, 2, 2);
        for (i, e) in table.data.iter_mut().enumerate() {
            // Value slowest: two entries a slice.
            e[2] = 1.0 + (i / 2) as f32;
        }
        let up = MapStage::look(table.clone(), 1.0).unwrap();
        let out = up.pixel([0.25; 3]);
        for c in out {
            assert!((c - 0.375).abs() < 1e-5, "{out:?}, wanted 0.375");
        }
        let level = MapStage::look(table, 0.0).unwrap();
        let out = level.pixel([0.25; 3]);
        for c in out {
            assert!((c - 0.3125).abs() < 1e-5, "{out:?}, wanted 0.3125");
        }
    }

    /// A look table's raised saturation stops at ProPhoto's edge, and a
    /// color already past it is not pushed further; a lowered one is
    /// lowered either way. A hue/sat map is left unclipped (§122).
    #[test]
    fn a_look_table_holds_saturation_inside_prophoto() {
        let mut up = HueSatMap::identity(4, 2, 1);
        for e in &mut up.data {
            e[1] = 4.0;
        }
        let look = MapStage::look(up.clone(), 0.0).unwrap();
        let sat_of = |stage: &MapStage, p: [f32; 3]| {
            let out = stage.pixel(apply3(&stage.from_prophoto, p));
            to_hsv(apply3(&stage.to_prophoto, out)).1
        };
        // Saturation 0.5 would go to 2; it stops at 1.
        let s = sat_of(&look, [0.4, 0.3, 0.2]);
        assert!((s - 1.0).abs() < 1e-4, "saturation {s}");
        // Saturation 1.25, past the edge, stays where it was.
        let s = sat_of(&look, [0.4, 0.2, -0.1]);
        assert!((s - 1.25).abs() < 1e-4, "saturation {s}");
        // The same table as a map is not held.
        let map = MapStage::new(up).unwrap();
        let s = sat_of(&map, [0.4, 0.3, 0.2]);
        assert!((s - 2.0).abs() < 1e-4, "saturation {s}");
        // And a lowering lowers, past the edge too.
        let mut down = HueSatMap::identity(4, 2, 1);
        for e in &mut down.data {
            e[1] = 0.5;
        }
        let look = MapStage::look(down, 0.0).unwrap();
        let s = sat_of(&look, [0.4, 0.2, -0.1]);
        assert!((s - 0.625).abs() < 1e-4, "saturation {s}");
    }

    #[test]
    fn a_value_scale_scales_the_pixel() {
        // Every node halves the value and leaves hue and saturation:
        // the whole picture comes out half as bright, whatever the
        // color.
        let mut map = HueSatMap::identity(6, 3, 1);
        for e in &mut map.data {
            e[2] = 0.5;
        }
        let stage = MapStage::new(map).unwrap();
        for c in [[0.18f32, 0.05, 0.4], [0.5, 0.5, 0.5], [0.9, 0.2, 0.1]] {
            let out = stage.pixel(c);
            for k in 0..3 {
                assert!(
                    (out[k] - c[k] * 0.5).abs() < 1e-5,
                    "{c:?} became {out:?}, wanted half"
                );
            }
        }
    }

    #[test]
    fn a_saturation_scale_of_zero_makes_grey() {
        let mut map = HueSatMap::identity(6, 3, 1);
        for e in &mut map.data {
            e[1] = 0.0;
        }
        let stage = MapStage::new(map).unwrap();
        // Grey in ProPhoto, which is not grey in the working space.
        let out = stage.pixel([0.4, 0.2, 0.1]);
        let prophoto = apply3(&stage.to_prophoto, out);
        assert!(
            (prophoto[0] - prophoto[1]).abs() < 1e-5 && (prophoto[1] - prophoto[2]).abs() < 1e-5,
            "{prophoto:?} is not neutral"
        );
    }

    #[test]
    #[ignore = "a measurement, not a test"]
    fn time_at_24_megapixels() {
        let (w, h) = (6000, 4000);
        let mut data = Vec::with_capacity(w * h * 3);
        for i in 0..w * h {
            let t = (i % 997) as f32 / 997.0;
            data.extend([0.2 + t * 0.6, 0.3 + t * 0.2, 0.1 + t * 0.7]);
        }
        let mut image = WorkingImage::from_data(w, h, data).unwrap();
        let path = std::env::var("GREYCARD_DCP").ok();
        let map = match &path {
            Some(p) => crate::dcp::Dcp::load(p)
                .unwrap()
                .primary
                .hue_sat_map
                .unwrap(),
            None => crate::dcp::HueSatMap::identity(90, 30, 1),
        };
        println!(
            "map {}x{}x{}",
            map.hue_divisions, map.sat_divisions, map.val_divisions
        );
        let stage = MapStage::new(map).unwrap();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            stage.apply(&mut image);
            println!("{:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
        }
        // A look table the size of Adobe's Camera profiles', sRGB
        // encoded, as the profile stage runs it.
        let mut look = HueSatMap::identity(90, 16, 16);
        look.encoding = Encoding::Srgb;
        look.data[0][1] = 1.1;
        let stage = MapStage::look(look, -0.64).unwrap();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            stage.apply(&mut image);
            println!(
                "look 90x16x16: {:.1} ms",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    #[test]
    fn the_map_runs_over_a_picture() {
        let mut image = WorkingImage::from_data(2, 1, vec![0.4, 0.2, 0.1, 0.1, 0.2, 0.4]).unwrap();
        let mut map = HueSatMap::identity(6, 3, 1);
        for e in &mut map.data {
            e[2] = 0.25;
        }
        let stage = MapStage::new(map).unwrap();
        stage.apply(&mut image);
        for (got, want) in image.data.iter().zip([0.1, 0.05, 0.025, 0.025, 0.05, 0.1]) {
            assert!((got - want).abs() < 1e-5, "{got} is not {want}");
        }
    }
}
