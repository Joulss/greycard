//! Sony's focus point: the maker note's FocusLocation (0x2027), four
//! 16-bit words, the width and height of the picture it is measured in
//! and the point's X and Y from its top left corner, and FocusFrameSize
//! (0x2037), three words, the focus frame's width and height centered
//! on the point and a word that is nonzero when the frame is valid. The
//! camera writes the picture's middle when it has no point. FocusMode
//! (0x201b) is 0 for manual focus. The layouts are ExifTool's
//! (`lib/Image/ExifTool/Sony.pm`, copyright Phil Harvey, under the same
//! terms as Perl); the reading code is ours. Checked on the bytes and
//! the picture of an ILCE-7M4 file.
//!
//! No point: manual focus, no FocusLocation, the middle with no valid
//! frame (the camera's stand-in for none), the top left corner (not
//! filled) and a point at or past the far edges, and a FocusLocation
//! of another size than the raw (an APS-C crop on a full frame body,
//! not seen on a file). Most DSC compacts
//! write 0 in FocusMode whatever the focus (ExifTool reads the tag only
//! for the interchangeable lens bodies and a few DSC models), so their
//! raws read as manual focus and have no point: a none that may be
//! wrong, never a wrong point.

use std::io::{Read, Seek};

use ::rawler::bits::Endian;

use super::{Spot, u16_in};
use crate::decode::style::{
    Field, TAG_IMAGE_LENGTH, TAG_IMAGE_WIDTH, Tiff, find, int_tag, maker_note,
};
use crate::error::Result;

const SONY_FOCUS_MODE: u16 = 0x201b;
const SONY_FOCUS_LOCATION: u16 = 0x2027;
const SONY_FOCUS_FRAME_SIZE: u16 = 0x2037;
const TAG_SUB_IFDS: u16 = 0x014a;

/// The point of an ARW's note; `None` where it gives none.
pub(super) fn read<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<Spot>> {
    let Some(note) = maker_note(tiff, ifd0, endian)? else {
        return Ok(None);
    };
    // A bare IFD, offsets from the file's start; a compact's begins
    // "SONY DSC " and three NULs.
    let head = tiff.read_at(note.at, 12)?;
    let at = match head.starts_with(b"SONY ") {
        true => note.at + 12,
        false => note.at,
    };
    let ifd = tiff.ifd(at, 0, note.endian)?;
    if int_tag(tiff, &ifd, SONY_FOCUS_MODE) == Some(0) {
        return Ok(None);
    }
    let location = find(&ifd, SONY_FOCUS_LOCATION).and_then(|f| tiff.ints(&f));
    // UNDEFINED on the bodies seen: three words in the note's order.
    let frame = find(&ifd, SONY_FOCUS_FRAME_SIZE).and_then(|f| {
        let b = tiff.bytes(&f)?;
        (0..3)
            .map(|k| u16_in(f.endian, &b, 2 * k).map(i64::from))
            .collect::<Option<Vec<_>>>()
    });
    let Some(location) = location else {
        return Ok(None);
    };
    // The raw's IFD lies past the head on the A7 IV: read only for a
    // point.
    if spot(&location, frame.as_deref(), None).is_none() {
        return Ok(None);
    }
    let raw = raw_size(tiff, ifd0, endian)?;
    Ok(spot(&location, frame.as_deref(), raw))
}

/// The raw's size, from IFD0's first sub-IFD, where the ARW keeps it.
fn raw_size<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<(i64, i64)>> {
    let Some(at) = find(ifd0, TAG_SUB_IFDS).and_then(|f| tiff.ints(&f)?.first().copied()) else {
        return Ok(None);
    };
    let Ok(at) = u64::try_from(at) else {
        return Ok(None);
    };
    let ifd = tiff.ifd(at, 0, endian)?;
    Ok(int_tag(tiff, &ifd, TAG_IMAGE_WIDTH).zip(int_tag(tiff, &ifd, TAG_IMAGE_LENGTH)))
}

/// FocusLocation `[w, h, x, y]` and FocusFrameSize `[w, h, valid]` to a
/// spot, held to the raw's size where it is known: within 2% on each
/// axis (the raw keeps a few columns and rows of margin past the
/// picture).
fn spot(location: &[i64], frame: Option<&[i64]>, raw: Option<(i64, i64)>) -> Option<Spot> {
    let &[w, h, x, y] = location.get(..4)? else {
        return None;
    };
    if w <= 0 || h <= 0 {
        return None;
    }
    if let Some((rw, rh)) = raw {
        let near = |a: i64, b: i64| (a - b).abs() * 50 <= a.max(b);
        if !near(w, rw) || !near(h, rh) {
            return None;
        }
    }
    let frame = match frame {
        Some(&[fw, fh, valid, ..]) if valid != 0 && fw > 0 && fh > 0 => Some((fw, fh)),
        _ => None,
    };
    if frame.is_none() && (2 * x - w).abs() <= 2 && (2 * y - h).abs() <= 2 {
        return None;
    }
    if (x == 0 && y == 0) || x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    let (fw, fh) = frame.unwrap_or((0, 0));
    Spot::at(
        x as f64,
        y as f64,
        fw as f64,
        fh as f64,
        (w as f64, h as f64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_location_and_its_frame() {
        let s = spot(
            &[7008, 4672, 4982, 2735],
            Some(&[153, 154, 1]),
            Some((7040, 4688)),
        )
        .unwrap();
        assert_eq!(s.of, (7008.0, 4672.0));
        let [x, y, w, h] = s.rect;
        assert!((x + w / 2.0 - 4982.0 / 7008.0).abs() < 1e-9);
        assert!((y + h / 2.0 - 2735.0 / 4672.0).abs() < 1e-9);
        assert!((w - 153.0 / 7008.0).abs() < 1e-9);
        // No valid frame: a point, drawn its default size.
        let p = spot(&[7008, 4672, 4982, 2735], Some(&[153, 154, 0]), None).unwrap();
        assert!((p.rect[3] - 4672.0 / 16.0 / 4672.0).abs() < 1e-9);
    }

    #[test]
    fn the_middle_without_a_frame_a_crop_and_nonsense_are_none() {
        assert_eq!(spot(&[7008, 4672, 3504, 2336], None, None), None);
        assert!(spot(&[7008, 4672, 3504, 2336], Some(&[200, 200, 1]), None).is_some());
        // APS-C crop on a full frame raw.
        assert_eq!(
            spot(&[4608, 3072, 1000, 1000], None, Some((7040, 4688))),
            None
        );
        assert_eq!(spot(&[0, 0, 0, 0], None, None), None);
        assert_eq!(spot(&[7008, 4672, 0, 0], None, None), None);
        assert_eq!(spot(&[7008, 4672, 0, 0], Some(&[153, 154, 1]), None), None);
        assert!(spot(&[7008, 4672, 0, 100], None, None).is_some());
        assert_eq!(spot(&[65535, 65535, 65535, 65535], None, None), None);
        assert_eq!(spot(&[7008, 4672, 7008, 100], None, None), None);
        assert_eq!(
            spot(&[7008, 4672, 100, 4672], Some(&[153, 154, 1]), None),
            None
        );
        assert!(spot(&[7008, 4672, 7007, 4671], None, None).is_some());
        assert_eq!(spot(&[7008, 4672], None, None), None);
        assert_eq!(spot(&[-5, 4672, 1, 1], None, None), None);
    }
}
