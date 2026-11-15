//! Nikon's focus point: the maker note's AFInfo2 record (0x00b7), a
//! block of bytes that begins with its version as four ASCII digits.
//! Where the record carries coordinates, they are the AF area's center
//! and size in pixels of an AF image (the camera's full-size picture),
//! from its top left corner, 16-bit words in the note's byte order:
//!
//! - 0100, the live view DSLRs of 2007 to 2016: from 0x10, on contrast
//!   detect (byte 4 = 1), in focus at 0x1c.
//! - 0101, the D5, D500, D850, and D7500: from 0x46, on contrast
//!   detect, in focus at 0x52.
//! - 0300 and 0301, the D6, D780, Z5, Z6, Z7, Z50, Zfc, and their II:
//!   from 0x2a, when byte 7 (AFCoordinatesAvailable) is 1.
//! - 0400 to 0402, the Z8, Z9, Zf, Z6III, Z5II, and Z50II: from 0x3e,
//!   when byte 7 is 1, FocusResult at 0x4a.
//!
//! Each at its offset: AFImageWidth, AFImageHeight, AFAreaXPosition,
//! AFAreaYPosition, AFAreaWidth, AFAreaHeight. The layouts are
//! ExifTool's (AFInfo2V0100, V0101, V0300, and V0400 in
//! `lib/Image/ExifTool/Nikon.pm`, copyright Phil Harvey, under the same
//! terms as Perl); the reading code is ours. Version 0401 was checked on
//! the bytes and pictures of real Z6III files; the others are read as
//! documented and tested on built bytes only.
//!
//! No point: a phase detect record of the DSLRs' viewfinder AF (its
//! point is one of a grid whose layout per body we do not hold), an
//! auto area or 3D tracking record before the camera found a subject
//! (coordinates not filled), a focus the camera says it did not reach,
//! a manual focus (FocusMode 0x0007, "MF" or "Manual"), and a crop
//! taken in camera (CropHiSpeed 0x001b, its cropped size smaller than
//! the full), whose AF image is not checked against the raw.

use std::io::{Read, Seek};

use ::rawler::bits::Endian;

use super::{Spot, u16_in};
use crate::decode::style::{Field, Tiff, find, maker_note};
use crate::error::Result;

const NIKON_FOCUS_MODE: u16 = 0x0007;
const NIKON_CROP_HI_SPEED: u16 = 0x001b;
const NIKON_AF_INFO2: u16 = 0x00b7;

/// The point of a NEF's note; `None` where it gives none.
pub(super) fn read<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<Spot>> {
    let Some(note) = maker_note(tiff, ifd0, endian)? else {
        return Ok(None);
    };
    if note.count < 18 {
        return Ok(None);
    }
    // "Nikon\0", a version, two spare bytes, and a TIFF header whose
    // offsets count from itself; the oldest bodies' note has no header
    // and no AFInfo2.
    let head = tiff.read_at(note.at, 18)?;
    let endian = match (&head[..6], &head[10..12]) {
        (b"Nikon\0", b"II") => Endian::Little,
        (b"Nikon\0", b"MM") => Endian::Big,
        _ => return Ok(None),
    };
    let base = note.at + 10;
    let first = u64::from(u32_of(endian, &head[14..18]));
    let ifd = tiff.ifd(base + first, base, endian)?;
    if find(&ifd, NIKON_FOCUS_MODE)
        .and_then(|f| tiff.ascii(&f))
        .is_some_and(|m| manual(&m))
    {
        return Ok(None);
    }
    if let Some(crop) = find(&ifd, NIKON_CROP_HI_SPEED).and_then(|f| tiff.ints(&f))
        && !uncropped(&crop)
    {
        return Ok(None);
    }
    Ok(find(&ifd, NIKON_AF_INFO2)
        .and_then(|f| tiff.bytes(&f))
        .and_then(|b| af_info2(&b, endian)))
}

fn u32_of(endian: Endian, b: &[u8]) -> u32 {
    let b = [b[0], b[1], b[2], b[3]];
    match endian {
        Endian::Little => u32::from_le_bytes(b),
        Endian::Big => u32::from_be_bytes(b),
    }
}

/// FocusMode is text: "AF-S", "AF-C", "AF-A", "MF", or "Manual".
fn manual(mode: &str) -> bool {
    let mode = mode.trim().to_ascii_uppercase();
    mode.starts_with("MF") || mode.starts_with("MANUAL")
}

/// CropHiSpeed: the mode, the full width and height, the cropped width
/// and height, and the crop's origin. A crop smaller than the full
/// (DX on an FX body, 1.2x, 5:4, 1:1, 16:9) is not one we have seen
/// the AF image of; a record too short to say is no crop.
fn uncropped(c: &[i64]) -> bool {
    match c {
        [_, fw, fh, cw, ch, ..] if *fw > 0 && *fh > 0 => cw * 100 >= fw * 99 && ch * 100 >= fh * 99,
        _ => true,
    }
}

/// AFInfo2's point, by its version; `None` for a record of no
/// coordinates, one not in focus, and a version we do not read.
fn af_info2(b: &[u8], endian: Endian) -> Option<Spot> {
    let byte = |at: usize| b.get(at).copied();
    let (at, in_focus) = match b.get(..4)? {
        b"0100" if byte(4)? == 1 => (0x10, Some(0x1c)),
        b"0101" if byte(4)? == 1 => (0x46, Some(0x52)),
        b"0300" | b"0301" if byte(7)? == 1 => (0x2a, None),
        b"0400" | b"0401" | b"0402" if byte(7)? == 1 => (0x3e, Some(0x4a)),
        _ => return None,
    };
    if let Some(f) = in_focus
        && byte(f)? != 1
    {
        return None;
    }
    let word = |k: usize| u16_in(endian, b, at + 2 * k).map(f64::from);
    let (w, h) = (word(0)?, word(1)?);
    let (x, y) = (word(2)?, word(3)?);
    let (aw, ah) = (word(4)?, word(5)?);
    // Coordinates not filled are zeros, or all ones: no point, not a
    // corner.
    if (x == 0.0 && y == 0.0) || x >= w || y >= h {
        return None;
    }
    Spot::at(x, y, aw, ah, (w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 0401 record as a Z6III writes it: 80 bytes,
    /// little-endian words from 0x3e.
    pub(crate) fn v0401(x: u16, y: u16, focused: u8) -> Vec<u8> {
        let mut b = vec![0u8; 80];
        b[..4].copy_from_slice(b"0401");
        b[4] = 2; // hybrid
        b[5] = 207; // 3D tracking
        b[7] = 1;
        for (k, v) in [6048u16, 4032, x, y, 270, 268].into_iter().enumerate() {
            b[0x3e + 2 * k..0x40 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        b[0x4a] = focused;
        b
    }

    #[test]
    fn a_z_record_is_its_centered_area() {
        let s = af_info2(&v0401(4158, 1103, 1), Endian::Little).unwrap();
        assert_eq!(s.of, (6048.0, 4032.0));
        let [x, y, w, h] = s.rect;
        assert!((x + w / 2.0 - 4158.0 / 6048.0).abs() < 1e-9);
        assert!((y + h / 2.0 - 1103.0 / 4032.0).abs() < 1e-9);
        assert!((w - 270.0 / 6048.0).abs() < 1e-9 && (h - 268.0 / 4032.0).abs() < 1e-9);
    }

    #[test]
    fn out_of_focus_unfilled_and_unknown_records_have_no_point() {
        assert_eq!(af_info2(&v0401(4158, 1103, 0), Endian::Little), None);
        assert_eq!(af_info2(&v0401(0, 0, 1), Endian::Little), None);
        // At or past the AF image's far edges, as a record of all ones.
        assert_eq!(af_info2(&v0401(6048, 100, 1), Endian::Little), None);
        assert_eq!(af_info2(&v0401(100, 4032, 1), Endian::Little), None);
        let mut ones = v0401(4158, 1103, 1);
        ones[0x3e..0x4a].fill(0xff);
        assert_eq!(af_info2(&ones, Endian::Little), None);
        assert!(af_info2(&v0401(6047, 4031, 1), Endian::Little).is_some());
        let mut grid = v0401(4158, 1103, 1);
        grid[7] = 0;
        assert_eq!(af_info2(&grid, Endian::Little), None);
        let mut future = v0401(4158, 1103, 1);
        future[..4].copy_from_slice(b"0500");
        assert_eq!(af_info2(&future, Endian::Little), None);
        // Cut short before FocusResult: none, no panic; the bytes past it
        // are not read.
        let full = v0401(4158, 1103, 1);
        for n in 0..full.len() {
            let read = af_info2(&full[..n], Endian::Little);
            assert_eq!(read.is_some(), n > 0x4a, "{n}");
        }
    }

    /// The older versions at their own offsets, big-endian too.
    #[test]
    fn older_versions_read_at_their_offsets() {
        let record = |ver: &[u8; 4], method: u8, at: usize, focus: Option<usize>| {
            let mut b = vec![0u8; 0x60];
            b[..4].copy_from_slice(ver);
            b[4] = method;
            b[7] = 1;
            for (k, v) in [6000u16, 4000, 1500, 1000, 200, 100]
                .into_iter()
                .enumerate()
            {
                b[at + 2 * k..at + 2 + 2 * k].copy_from_slice(&v.to_be_bytes());
            }
            if let Some(f) = focus {
                b[f] = 1;
            }
            b
        };
        for (ver, at, focus) in [
            (b"0100", 0x10, Some(0x1c)),
            (b"0101", 0x46, Some(0x52)),
            (b"0300", 0x2a, None),
        ] {
            let s = af_info2(&record(ver, 1, at, focus), Endian::Big).unwrap();
            assert_eq!(s.of, (6000.0, 4000.0), "{ver:?}");
            assert!((s.rect[0] - 1400.0 / 6000.0).abs() < 1e-9, "{ver:?}");
        }
        // Phase detect through the viewfinder: a grid point, not read.
        assert_eq!(
            af_info2(&record(b"0100", 0, 0x10, Some(0x1c)), Endian::Big),
            None
        );
    }

    #[test]
    fn manual_focus_and_crops_are_told() {
        assert!(manual("MF") && manual("Manual ") && !manual("AF-C  "));
        assert!(uncropped(&[11, 6064, 4040, 6064, 4040, 0, 0]));
        assert!(!uncropped(&[1, 6064, 4040, 3984, 2656, 1040, 692]));
        assert!(uncropped(&[0, 0, 0]));
    }
}
