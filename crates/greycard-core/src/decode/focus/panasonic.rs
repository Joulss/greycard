//! Panasonic's focus point: the maker note's AFPointPosition (0x004d),
//! two rationals, the primary AF area's center as fractions of the
//! picture from its top left, past 1 when there is none. The picture's
//! size is the note's ImageWidth and ImageHeight (0x004b, 0x004c); no
//! size is given for the area. FocusMode (0x0007) is 2 for manual
//! focus. An RW2's note is in the EXIF of the JPEG it embeds (IFD0's
//! 0x002e), "Panasonic", three NULs, and an IFD with offsets from that
//! EXIF's TIFF header; IFD0's 0x0002 and 0x0003 are the sensor's width
//! and height. The layouts are ExifTool's
//! (`lib/Image/ExifTool/Panasonic.pm`, copyright Phil Harvey, under the
//! same terms as Perl); the reading code is ours. Checked on the bytes
//! and the picture of a DC-S5M2 file.
//!
//! No point: manual focus, a position off the picture or on its far
//! edges (the "none" and "n/a" values), the top left corner (not
//! filled), and a picture of another shape than the sensor (an aspect
//! chosen in camera).

use std::io::{Cursor, Read, Seek};

use ::rawler::bits::Endian;

use super::{Spot, rationals, same_shape};
use crate::decode::style::{
    Field, JPEG_HEAD, Tiff, find, int_tag, jpeg_exif, jpeg_head, maker_note, tiff_ifd0,
};
use crate::error::{Error, Result};

const PANASONIC_SENSOR_WIDTH: u16 = 0x0002;
const PANASONIC_SENSOR_HEIGHT: u16 = 0x0003;
const PANASONIC_JPG_FROM_RAW: u16 = 0x002e;
const PANASONIC_FOCUS_MODE: u16 = 0x0007;
const PANASONIC_IMAGE_WIDTH: u16 = 0x004b;
const PANASONIC_IMAGE_HEIGHT: u16 = 0x004c;
const PANASONIC_AF_POINT_POSITION: u16 = 0x004d;

/// The point of an RW2's note; `None` where it gives none.
pub(super) fn read<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<Spot>> {
    let sensor = int_tag(tiff, ifd0, PANASONIC_SENSOR_WIDTH)
        .zip(int_tag(tiff, ifd0, PANASONIC_SENSOR_HEIGHT))
        .map(|(w, h)| (w as f64, h as f64));
    // The RW2's own Exif IFD has carried no note on any body seen; the
    // embedded JPEG's has.
    if let Some(note) = maker_note(tiff, ifd0, endian)? {
        return from_note(tiff, &note, sensor);
    }
    let Some(jpeg) = find(ifd0, PANASONIC_JPG_FROM_RAW) else {
        return Ok(None);
    };
    let n = u64::from(jpeg.count).min(JPEG_HEAD);
    if jpeg.at.checked_add(n).is_none_or(|end| end > tiff.len) {
        return Err(Error::Decode(format!(
            "focus point: {n} bytes at {} run past the end of the file",
            jpeg.at
        )));
    }
    let head = jpeg_head(&mut tiff.r, jpeg.at, n)?;
    let Some(exif) = jpeg_exif(&head) else {
        return Ok(None);
    };
    let mut inner = Tiff::new(Cursor::new(exif))?;
    let (ifd0, endian) = tiff_ifd0(&mut inner)?;
    match maker_note(&mut inner, &ifd0, endian)? {
        Some(note) => from_note(&mut inner, &note, sensor),
        None => Ok(None),
    }
}

fn from_note<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    note: &Field,
    sensor: Option<(f64, f64)>,
) -> Result<Option<Spot>> {
    let head = tiff.read_at(note.at, 12)?;
    if !head.starts_with(b"Panasonic\0") {
        return Ok(None);
    }
    let ifd = tiff.ifd(note.at + 12, 0, note.endian)?;
    if int_tag(tiff, &ifd, PANASONIC_FOCUS_MODE) == Some(2) {
        return Ok(None);
    }
    let of = int_tag(tiff, &ifd, PANASONIC_IMAGE_WIDTH)
        .zip(int_tag(tiff, &ifd, PANASONIC_IMAGE_HEIGHT))
        .map(|(w, h)| (w as f64, h as f64));
    let position = find(&ifd, PANASONIC_AF_POINT_POSITION).and_then(|f| rationals(tiff, &f));
    Ok(position.and_then(|p| spot(&p, of, sensor)))
}

/// AFPointPosition `[x, y]` on a picture `of`, held to the sensor's
/// shape where it is known.
fn spot(position: &[f64], of: Option<(f64, f64)>, sensor: Option<(f64, f64)>) -> Option<Spot> {
    let &[x, y] = position.get(..2)? else {
        return None;
    };
    let of = of?;
    // The top left corner is a position not filled; the far edges are
    // off the picture.
    let inside = |v: f64| (0.0..1.0).contains(&v);
    if !inside(x) || !inside(y) || (x == 0.0 && y == 0.0) {
        return None;
    }
    if sensor.is_some_and(|s| !same_shape(s, of, 0.01)) {
        return None;
    }
    Spot::at(x * of.0, y * of.1, 0.0, 0.0, of)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_a_point_on_the_picture() {
        let of = Some((6000.0, 4000.0));
        let s = spot(
            &[774.0 / 1024.0, 384.0 / 1024.0],
            of,
            Some((6008.0, 4008.0)),
        )
        .unwrap();
        let [x, y, w, h] = s.rect;
        assert!((x + w / 2.0 - 774.0 / 1024.0).abs() < 1e-9);
        assert!((y + h / 2.0 - 0.375).abs() < 1e-9);
        assert!((h - 1.0 / 16.0).abs() < 1e-9 && (w - 250.0 / 6000.0).abs() < 1e-9);
    }

    #[test]
    fn none_and_another_shape_are_no_point() {
        let of = Some((6000.0, 4000.0));
        // ExifTool's "none" (16777216) and "n/a" (4194303.9).
        assert_eq!(spot(&[16777216.0, 16777216.0], of, None), None);
        assert_eq!(spot(&[4194303.9, 4194303.9], of, None), None);
        assert_eq!(spot(&[f64::NAN, 0.5], of, None), None);
        assert_eq!(spot(&[0.5, 0.5], None, None), None);
        assert_eq!(spot(&[0.0, 0.0], of, None), None);
        assert!(spot(&[0.0, 0.5], of, None).is_some());
        assert_eq!(spot(&[1.0, 1.0], of, None), None);
        assert_eq!(spot(&[0.5, 1.0], of, None), None);
        // 16:9 chosen in camera on a 3:2 sensor.
        assert_eq!(
            spot(&[0.5, 0.5], Some((6000.0, 3376.0)), Some((6008.0, 4008.0))),
            None
        );
    }
}
