//! Olympus and OM System's focus point, in the maker note's Camera
//! Settings (0x2020): AFPointSelected (0x0305), five signed rationals,
//! the first unused, then the AF area's left, top, right, and bottom as
//! fractions of the picture (a point when they meet); and AFAreas
//! (0x0304), 64 words, each an area's left, top, right, and bottom
//! bytes from the high byte down, in 255ths, zero when unused. Where an
//! area holds the selected point, the smallest such is the area.
//! FocusMode (0x0301) is 10 for manual focus, FocusProcess (0x0302) 0
//! when AF was not used, and AFSearch (0x0303) 0 when it found no
//! focus. The picture is the sensor's valid area, Image Processing's
//! (0x2040) CropWidth and CropHeight. The layouts are ExifTool's
//! (`lib/Image/ExifTool/Olympus.pm`, copyright Phil Harvey, under the
//! same terms as Perl); the reading code is ours. Checked on the bytes
//! and the picture of an OM-5 file.
//!
//! No point: manual focus, AF not used or not found, a selection that
//! is not a fraction of the picture, and an aspect chosen in camera
//! (the AspectFrame less than the whole area), whose fractions we have
//! not seen measured against which picture.

use std::io::{Read, Seek};

use ::rawler::bits::Endian;

use super::{Spot, rationals, smallest_holding};
use crate::decode::style::{
    Field, OLYMPUS_ASPECT_FRAME, OLYMPUS_CAMERA_SETTINGS, OLYMPUS_CROP_HEIGHT, OLYMPUS_CROP_WIDTH,
    OLYMPUS_IMAGE_PROCESSING, Tiff, find, maker_note, olympus_note, olympus_sub,
};
use crate::error::Result;

const OLYMPUS_FOCUS_MODE: u16 = 0x0301;
const OLYMPUS_FOCUS_PROCESS: u16 = 0x0302;
const OLYMPUS_AF_SEARCH: u16 = 0x0303;
const OLYMPUS_AF_AREAS: u16 = 0x0304;
const OLYMPUS_AF_POINT_SELECTED: u16 = 0x0305;

/// The point of an ORF's note; `None` where it gives none.
pub(super) fn read<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<Spot>> {
    let Some(note) = maker_note(tiff, ifd0, endian)? else {
        return Ok(None);
    };
    let Some(main) = olympus_note(tiff, &note)? else {
        return Ok(None);
    };
    let Some(cs) = olympus_sub(tiff, &main, OLYMPUS_CAMERA_SETTINGS)? else {
        return Ok(None);
    };
    let mut first = |tag| find(&cs, tag).and_then(|f| tiff.ints(&f)?.first().copied());
    if first(OLYMPUS_FOCUS_MODE) == Some(10)
        || first(OLYMPUS_FOCUS_PROCESS) == Some(0)
        || first(OLYMPUS_AF_SEARCH) == Some(0)
    {
        return Ok(None);
    }
    let Some(selected) = find(&cs, OLYMPUS_AF_POINT_SELECTED).and_then(|f| rationals(tiff, &f))
    else {
        return Ok(None);
    };
    let areas = find(&cs, OLYMPUS_AF_AREAS)
        .and_then(|f| tiff.ints(&f))
        .unwrap_or_default();
    let Some(ip) = olympus_sub(tiff, &main, OLYMPUS_IMAGE_PROCESSING)? else {
        return Ok(None);
    };
    let mut ints = |tag| find(&ip, tag).and_then(|f| tiff.ints(&f));
    let size = ints(OLYMPUS_CROP_WIDTH)
        .and_then(|w| w.first().copied())
        .zip(ints(OLYMPUS_CROP_HEIGHT).and_then(|h| h.first().copied()));
    let frame = ints(OLYMPUS_ASPECT_FRAME);
    Ok(size.and_then(|(w, h)| spot(&selected, &areas, (w, h), frame.as_deref())))
}

/// AFPointSelected and AFAreas on a valid area `w` by `h`, with its
/// AspectFrame (inclusive corners) where the note gives one.
fn spot(
    selected: &[f64],
    areas: &[i64],
    (w, h): (i64, i64),
    frame: Option<&[i64]>,
) -> Option<Spot> {
    if w <= 0 || h <= 0 {
        return None;
    }
    if let Some(f) = frame
        && f.get(..4) != Some(&[0, 0, w - 1, h - 1])
    {
        return None;
    }
    let &[l, t, r, b] = selected.get(1..5)? else {
        return None;
    };
    let unit = |v: f64| (0.0..=1.0).contains(&v);
    if ![l, t, r, b].into_iter().all(unit) || r < l || b < t || [l, t, r, b] == [0.0; 4] {
        return None;
    }
    let of = (w as f64, h as f64);
    let center = ((l + r) / 2.0, (t + b) / 2.0);
    // A point at the far edges is off the picture.
    if center.0 >= 1.0 || center.1 >= 1.0 {
        return None;
    }
    let boxes = areas.iter().filter(|&&v| v != 0).map(|&v| {
        let byte = |s: u32| ((v >> s) & 0xff) as f64 / 255.0;
        [byte(24), byte(16), byte(8), byte(0)]
    });
    let scale = |b: [f64; 4]| [b[0] * of.0, b[1] * of.1, b[2] * of.0, b[3] * of.1];
    match smallest_holding(boxes, center) {
        Some(b) => Spot::spanning(scale(b), of),
        None => Spot::spanning(scale([l, t, r, b]), of),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OM5_AREA: i64 = 1668773512; // (99, 119)-(118, 136)

    #[test]
    fn the_area_holding_the_selected_point() {
        let sel = [f64::NAN, 273.0 / 640.0, 0.5, 273.0 / 640.0, 0.5];
        let s = spot(
            &sel,
            &[OM5_AREA, 0, 0],
            (5184, 3888),
            Some(&[0, 0, 5183, 3887]),
        )
        .unwrap();
        let [x, y, w, h] = s.rect;
        assert!((x - 99.0 / 255.0).abs() < 1e-9 && (y - 119.0 / 255.0).abs() < 1e-9);
        assert!((w - 19.0 / 255.0).abs() < 1e-9 && (h - 17.0 / 255.0).abs() < 1e-9);
        // No area holds it: a point at the selection.
        let p = spot(&sel, &[], (5184, 3888), None).unwrap();
        assert!((p.rect[1] + p.rect[3] / 2.0 - 0.5).abs() < 1e-9);
        // A selection with a size of its own is that area.
        let wide = [0.0, 0.25, 0.25, 0.75, 0.75];
        let a = spot(&wide, &[], (5184, 3888), None).unwrap();
        assert!((a.rect[2] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn an_aspect_frame_and_nonsense_are_none() {
        let sel = [0.0, 0.4, 0.5, 0.4, 0.5];
        assert_eq!(
            spot(&sel, &[], (5184, 3888), Some(&[0, 486, 5183, 3401])),
            None
        );
        assert_eq!(spot(&[0.0; 5], &[], (5184, 3888), None), None);
        assert_eq!(
            spot(&[0.0, 1.5, 0.5, 1.5, 0.5], &[], (5184, 3888), None),
            None
        );
        assert_eq!(
            spot(&[0.0, f64::NAN, 0.5, 0.4, 0.5], &[], (5184, 3888), None),
            None
        );
        assert_eq!(spot(&sel[..3], &[], (5184, 3888), None), None);
        assert_eq!(
            spot(&[0.0, 1.0, 1.0, 1.0, 1.0], &[], (5184, 3888), None),
            None
        );
        assert_eq!(spot(&sel, &[], (0, 3888), None), None);
    }
}
