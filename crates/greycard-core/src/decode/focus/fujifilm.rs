//! Fujifilm's focus point: the maker note's FocusPixel (0x1023), two
//! 16-bit words, the point's X and Y in the pixels of the RAF's
//! embedded JPEG (its EXIF's PixelXDimension and PixelYDimension), from
//! the top left. The note is the one in that JPEG's EXIF: "FUJIFILM",
//! the IFD's offset, and a little-endian IFD with offsets from the
//! note's start. No size is given; where a detected face or eye box
//! (FaceElementPositions 0x4203 with FaceElementTypes 0x4201,
//! FacePositions 0x4103, each left, top, right, and bottom in the same
//! pixels) holds the point, the smallest such box gives the area its
//! size, centered on the point; an eye's box is the area itself.
//! FocusMode (0x1021) is 1 for manual focus,
//! FocusWarning (0x1301) 1 when the camera found the picture out of
//! focus, and CropMode (0x104d) nonzero for a crop taken in camera. The
//! RAF's own metadata (at the header's 0x5C) gives the raw's cropped
//! size (0x111, height then width), the picture the JPEG shows. The
//! layouts are ExifTool's (`lib/Image/ExifTool/FujiFilm.pm`, copyright
//! Phil Harvey, under the same terms as Perl); the reading code is
//! ours. Checked on the bytes and pictures of GFX100S II, X100VI,
//! X-E4, and X-T5 files.
//!
//! No point: manual focus, the focus warning, the top left corner
//! (unfilled) and a pixel at or past the far edges, the middle pixel
//! unless a face or eye box holds it (see [`spot`]), a crop mode (the
//! sports finder, the 1.25x crop, the digital teleconverter, a full
//! frame lens on a GFX), and a JPEG of another shape than the raw's
//! cropped size (an aspect chosen in camera).

use std::io::{Cursor, Read, Seek, SeekFrom};

use ::rawler::bits::Endian;

use super::{Spot, orientation, same_shape, smallest_holding};
use crate::decode::rawler_backend::{RAF_MAGIC, be_u32_at};
use crate::decode::style::{
    TAG_EXIF_IFD, TAG_MAKER_NOTE, Tiff, find, int_tag, jpeg_exif, jpeg_head, read_up_to, tiff_ifd0,
};
use crate::error::Result;
use crate::raw::Orientation;

const FUJI_FOCUS_MODE: u16 = 0x1021;
const FUJI_FOCUS_PIXEL: u16 = 0x1023;
const FUJI_CROP_MODE: u16 = 0x104d;
const FUJI_FOCUS_WARNING: u16 = 0x1301;
const FUJI_FACE_POSITIONS: u16 = 0x4103;
const FUJI_FACE_ELEMENT_TYPES: u16 = 0x4201;
const FUJI_FACE_ELEMENT_POSITIONS: u16 = 0x4203;
/// FaceElementTypes that are eyes: left, right, both, and an animal's
/// and a bird's left and right.
const FUJI_EYES: [i64; 7] = [2, 3, 9, 17, 18, 21, 22];
const TAG_PIXEL_X: u16 = 0xa002;
const TAG_PIXEL_Y: u16 = 0xa003;
/// The RAF metadata's cropped size, height then width.
const RAF_CROPPED_SIZE: u16 = 0x111;
/// The most entries a RAF's metadata is walked for.
const RAF_MAX_ENTRIES: u32 = 256;
/// The RAF header: the magic, then at 0x54 the JPEG's offset and
/// length, at 0x5C the metadata's.
const RAF_HEAD: usize = 0x64;

/// The point of a RAF and the orientation of its JPEG; no point where
/// the note gives none, and nothing for a file too short to say.
pub(super) fn read(mut file: impl Read + Seek) -> Result<(Option<Spot>, Option<Orientation>)> {
    let mut header = [0u8; RAF_HEAD];
    if read_up_to(&mut file, &mut header)? < RAF_HEAD || !header.starts_with(RAF_MAGIC) {
        return Ok((None, None));
    }
    let head = jpeg_head(
        &mut file,
        be_u32_at(&header, 0x54),
        be_u32_at(&header, 0x58),
    )?;
    let Some(exif) = jpeg_exif(&head) else {
        return Ok((None, None));
    };
    let mut tiff = Tiff::new(Cursor::new(exif))?;
    let (ifd0, endian) = tiff_ifd0(&mut tiff)?;
    let orientation = orientation(&mut tiff, &ifd0);
    let Some(at) = int_tag(&mut tiff, &ifd0, TAG_EXIF_IFD).and_then(|a| u64::try_from(a).ok())
    else {
        return Ok((None, orientation));
    };
    let exif_ifd = tiff.ifd(at, 0, endian)?;
    let of = int_tag(&mut tiff, &exif_ifd, TAG_PIXEL_X)
        .zip(int_tag(&mut tiff, &exif_ifd, TAG_PIXEL_Y))
        .map(|(w, h)| (w as f64, h as f64));
    let Some(note) = find(&exif_ifd, TAG_MAKER_NOTE) else {
        return Ok((None, orientation));
    };
    let nh = tiff.read_at(note.at, 12)?;
    if !nh.starts_with(b"FUJIFILM") {
        return Ok((None, orientation));
    }
    let first = u64::from(u32::from_le_bytes([nh[8], nh[9], nh[10], nh[11]]));
    let ifd = tiff.ifd(note.at + first, note.at, Endian::Little)?;
    let int = |tiff: &mut Tiff<_>, tag| int_tag(tiff, &ifd, tag);
    if int(&mut tiff, FUJI_FOCUS_MODE) == Some(1)
        || int(&mut tiff, FUJI_FOCUS_WARNING) == Some(1)
        || int(&mut tiff, FUJI_CROP_MODE).is_some_and(|c| c != 0)
    {
        return Ok((None, orientation));
    }
    let Some(of) = of else {
        return Ok((None, orientation));
    };
    let Some(pixel) = find(&ifd, FUJI_FOCUS_PIXEL).and_then(|f| tiff.ints(&f)) else {
        return Ok((None, orientation));
    };
    // Each box with whether it is an eye's: an element by its type,
    // a face never.
    let types = find(&ifd, FUJI_FACE_ELEMENT_TYPES)
        .and_then(|f| tiff.ints(&f))
        .unwrap_or_default();
    let mut boxes = Vec::new();
    for tag in [FUJI_FACE_ELEMENT_POSITIONS, FUJI_FACE_POSITIONS] {
        if let Some(v) = find(&ifd, tag).and_then(|f| tiff.ints(&f)) {
            boxes.extend(v.as_chunks::<4>().0.iter().enumerate().map(|(k, b)| {
                let eye = tag == FUJI_FACE_ELEMENT_POSITIONS
                    && types.get(k).is_some_and(|t| FUJI_EYES.contains(t));
                (b.map(|n| n as f64), eye)
            }));
        }
    }
    let Some(spot) = spot(&pixel, &boxes, of) else {
        return Ok((None, orientation));
    };
    // Past the JPEG, so read only for a point.
    let cropped = raf_cropped_size(&mut file, &header)?;
    if cropped.is_some_and(|c| !same_shape(c, of, 0.01)) {
        return Ok((None, orientation));
    }
    Ok((Some(spot), orientation))
}

/// FocusPixel `[x, y]` in a picture `of`, with the smallest box
/// holding it giving its area's size (each box with whether it is an
/// eye's), or a point. The area is centered on the pixel, where the
/// camera focused, not on a face box, whose middle may be a hundred
/// pixels off it; an eye's box, small and on the eye, is the area. The
/// picture's middle pixel is none unless a face or eye box holds it: in
/// single point AF, 10 of 17 GFX100S II frames write the exact middle,
/// and on some (a couple their face detection found elsewhere, a figure
/// from behind, a lens held to water) the middle is soft, a focus and
/// recompose. A wrong point is worse than none, so a centered point
/// really focused on is lost too. The top left corner is a pixel not
/// filled, and one at or past the far edges is none.
fn spot(pixel: &[i64], boxes: &[([f64; 4], bool)], of: (f64, f64)) -> Option<Spot> {
    let &[x, y] = pixel.get(..2)? else {
        return None;
    };
    let (x, y) = (x as f64, y as f64);
    if (x == 0.0 && y == 0.0) || x >= of.0 || y >= of.1 {
        return None;
    }
    let held = smallest_holding(boxes.iter().map(|(b, _)| *b), (x, y));
    let eye = held.is_some_and(|h| boxes.iter().any(|(b, e)| *e && *b == h));
    match held {
        Some(b) if eye => Spot::spanning(b, of),
        Some(b) => Spot::at(x, y, b[2] - b[0], b[3] - b[1], of),
        None if middle((x, y), of) => None,
        None => Spot::at(x, y, 0.0, 0.0, of),
    }
}

/// Whether a pixel is the picture's middle, half its width and height
/// rounded either way, or a pixel short of it as a count from zero
/// writes it.
fn middle((x, y): (f64, f64), (w, h): (f64, f64)) -> bool {
    (2.0 * x - w).abs() <= 2.0 && (2.0 * y - h).abs() <= 2.0
}

/// The raw's cropped size, width and height, from the RAF's metadata:
/// a big-endian count, then entries of a tag, a size, and the data.
/// `None` where the metadata does not say.
fn raf_cropped_size(file: &mut (impl Read + Seek), header: &[u8]) -> Result<Option<(f64, f64)>> {
    let (at, len) = (be_u32_at(header, 0x5C), be_u32_at(header, 0x60));
    let end = at + len;
    let mut word = [0u8; 4];
    file.seek(SeekFrom::Start(at))?;
    if read_up_to(file, &mut word)? < 4 {
        return Ok(None);
    }
    let mut pos = at + 4;
    for _ in 0..u32::from_be_bytes(word).min(RAF_MAX_ENTRIES) {
        if pos + 4 > end {
            break;
        }
        file.seek(SeekFrom::Start(pos))?;
        if read_up_to(file, &mut word)? < 4 {
            break;
        }
        let tag = u16::from_be_bytes([word[0], word[1]]);
        let size = u64::from(u16::from_be_bytes([word[2], word[3]]));
        if tag == RAF_CROPPED_SIZE && size == 4 && pos + 8 <= end {
            let mut v = [0u8; 4];
            if read_up_to(file, &mut v)? < 4 {
                break;
            }
            let h = u16::from_be_bytes([v[0], v[1]]);
            let w = u16::from_be_bytes([v[2], v[3]]);
            return Ok((w > 0 && h > 0).then(|| (f64::from(w), f64::from(h))));
        }
        pos += 4 + size;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smallest_box_holding_the_pixel_is_the_area() {
        let of = (4000.0, 3000.0);
        let face = ([2287.0, 1324.0, 3104.0, 2003.0], false);
        let eye = ([2474.0, 1299.0, 2554.0, 1379.0], true);
        // The eye's box is the area.
        let s = spot(&[2491, 1337], &[face, eye], of).unwrap();
        assert!((s.rect[0] - 2474.0 / 4000.0).abs() < 1e-9);
        assert!((s.rect[2] - 80.0 / 4000.0).abs() < 1e-9);
        // A face's box gives the size, centered on the pixel.
        let s = spot(&[2491, 1337], &[face], of).unwrap();
        let [x, y, w, h] = s.rect;
        assert!((x + w / 2.0 - 2491.0 / 4000.0).abs() < 1e-9);
        assert!((y + h / 2.0 - 1337.0 / 3000.0).abs() < 1e-9);
        assert!((w - 817.0 / 4000.0).abs() < 1e-9 && (h - 679.0 / 3000.0).abs() < 1e-9);
        // The same box not marked an eye: centered on the pixel.
        let unmarked = (eye.0, false);
        let s = spot(&[2491, 1337], &[unmarked], of).unwrap();
        assert!((s.rect[0] + s.rect[2] / 2.0 - 2491.0 / 4000.0).abs() < 1e-9);
        // Outside every box: a point.
        let p = spot(&[100, 100], &[face, eye], of).unwrap();
        assert!((p.rect[3] - 1.0 / 16.0).abs() < 1e-9);
        assert_eq!(spot(&[100], &[], of), None);
        // The middle: none, faces or not, unless a box holds it.
        assert_eq!(spot(&[1999, 1499], &[face, eye], of), None);
        assert_eq!(spot(&[2000, 1500], &[face], of), None);
        assert_eq!(spot(&[1999, 1499], &[], of), None);
        let middle_eye = ([1980.0, 1480.0, 2020.0, 1520.0], true);
        assert!(spot(&[1999, 1499], &[face, middle_eye], of).is_some());
        // Off the middle by more than a pixel: a point.
        assert!(spot(&[2003, 1499], &[], of).is_some());
        // An odd size's middle, rounded either way.
        let odd = (4415.0, 2943.0);
        for (x, y) in [(2207, 1471), (2208, 1472), (2207, 1472)] {
            assert_eq!(spot(&[x, y], &[], odd), None, "{x} {y}");
        }
        assert!(spot(&[2210, 1471], &[], odd).is_some());
        // Not filled, and at or past the far edges.
        assert_eq!(spot(&[0, 0], &[], of), None);
        assert!(spot(&[0, 1499], &[], of).is_some());
        assert_eq!(spot(&[4000, 100], &[], of), None);
        assert_eq!(spot(&[100, 3000], &[], of), None);
        assert_eq!(spot(&[65535, 65535], &[], of), None);
        assert!(spot(&[3999, 2999], &[], of).is_some());
    }

    /// The RAF metadata walk finds the cropped size among other
    /// entries, stops at its section's end, and survives a count that
    /// lies.
    #[test]
    fn the_raf_metadata_gives_the_cropped_size() {
        let mut header = vec![0u8; RAF_HEAD];
        header[0x5C..0x60].copy_from_slice(&(RAF_HEAD as u32).to_be_bytes());
        let mut meta = 1000u32.to_be_bytes().to_vec();
        meta.extend([0x01, 0x00, 0x00, 0x04, 0x22, 0x32, 0x2e, 0x20]);
        meta.extend([0x01, 0x11, 0x00, 0x04, 0x14, 0x20, 0x1e, 0x30]);
        let mut file = header.clone();
        file.extend(&meta);
        file[0x60..0x64].copy_from_slice(&(meta.len() as u32).to_be_bytes());
        let got = raf_cropped_size(&mut Cursor::new(&file), &file).unwrap();
        assert_eq!(got, Some((7728.0, 5152.0)));
        // The section cut before the entry: none.
        file[0x60..0x64].copy_from_slice(&12u32.to_be_bytes());
        assert_eq!(
            raf_cropped_size(&mut Cursor::new(&file), &file).unwrap(),
            None
        );
    }
}
