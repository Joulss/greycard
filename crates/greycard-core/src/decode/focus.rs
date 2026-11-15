//! Where the camera focused, read from the maker note.
//!
//! Canon writes the AF areas it used in the maker note's AFInfo2 tag
//! (0x0026), an array of 16-bit values: AFInfoSize (in bytes),
//! AFAreaMode, NumAFPoints, ValidAFPoints, CanonImageWidth and
//! CanonImageHeight, AFImageWidth and AFImageHeight, then NumAFPoints
//! each of AFAreaWidths, AFAreaHeights, AFAreaXPositions, and
//! AFAreaYPositions (the positions signed, of each area's center from
//! the AF image's center, Y up), then AFPointsInFocus and
//! AFPointsSelected, bit masks of (NumAFPoints + 15) / 16 words, the
//! lowest bit of the first word the first area. SensorInfo (0x00e0)
//! gives the sensor's cropped area, elements 5 to 8 its left, top,
//! right, and bottom. The layout follows ExifTool's Canon tag
//! documentation (AFInfo2, SensorInfo, and FocusMode in
//! CameraSettings), `lib/Image/ExifTool/Canon.pm` at
//! <https://exiftool.org/TagNames/Canon.html>, copyright Phil Harvey,
//! distributed under the same terms as Perl (the GPL or the Artistic
//! License). Y up and the bit order were checked against the bytes and
//! the pictures of real CR3s from the R5, R5 Mark II, R6, R6 Mark II,
//! and R7. The reading code is ours.
//!
//! rawler parses the maker note and does not read these tags, so they
//! are read here: from a CR3's CMT3 box (and its Orientation from
//! CMT1), found by a walk of the box headers that reads nothing else,
//! or from a CR2's Exif maker note with the style reader's guarded
//! TIFF reader. Either reads a few small pieces of the file's head, so
//! it serves the culling loupe's camera picture, which has no decode,
//! as well as the decode.
//!
//! The other makers' notes are read in the submodules, each to a
//! [`Spot`]: Nikon ([`nikon`]), Sony ([`sony`]), Fujifilm
//! ([`fujifilm`]), Panasonic ([`panasonic`]), and Olympus and OM System
//! ([`olympus`]). Their layouts follow ExifTool's tag documentation and
//! `lib/Image/ExifTool/Nikon.pm`, `Sony.pm`, `FujiFilm.pm`,
//! `Panasonic.pm`, and `Olympus.pm`, copyright Phil Harvey, under the
//! same terms as Perl (the GPL or the Artistic License), each named in
//! its module; the reading code is ours.

use std::io::{Cursor, Read, Seek, SeekFrom};

use ::rawler::bits::Endian;

use super::rawler_backend::RAF_MAGIC;
use super::style::{
    Field, Maker, TAG_DNG_VERSION, TAG_MAKE, Tiff, find, maker_note, read_up_to, tiff_ifd0,
};
use crate::error::{Error, Result};
use crate::raw::{FocusPoint, Orientation};

#[cfg(test)]
mod fixtures;
mod fujifilm;
mod nikon;
mod olympus;
mod panasonic;
mod sony;

/// The maker note's CameraSettings, whose element 7 is FocusMode.
const CANON_CAMERA_SETTINGS: u16 = 0x0001;
const CANON_FOCUS_MODE: usize = 7;
/// AFInfo2.
const CANON_AF_INFO2: u16 = 0x0026;
/// SensorInfo: the cropped area's left, top, right, and bottom
/// (inclusive) are elements 5 to 8.
const CANON_SENSOR_INFO: u16 = 0x00e0;
/// The fixed head of AFInfo2 before its arrays.
const AF_HEAD: usize = 8;
const TAG_ORIENTATION: u16 = 0x0112;

/// The Canon box in a CR3's moov that holds the CMT boxes.
const CANON_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];
/// The largest moov and CMT box believed, and the most boxes walked
/// at a level: past these the file is damaged, not read.
const MAX_MOOV: u64 = 64 << 20;
const MAX_CMT: u64 = 1 << 20;
const MAX_BOXES: usize = 64;

/// One of Canon's AF areas, in AF image pixels from its center, Y up.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AfArea {
    w: i64,
    h: i64,
    x: i64,
    y: i64,
}

/// What AFInfo2 says, read and not yet judged.
#[derive(Debug, Clone, PartialEq)]
struct AfInfo {
    mode: i64,
    canon: (i64, i64),
    af: (i64, i64),
    /// The valid areas, each with whether the camera found it in focus.
    areas: Vec<(AfArea, bool)>,
}

/// What a Canon maker note says that the focus point needs.
#[derive(Debug, Clone, PartialEq, Default)]
struct CanonNote {
    mode: Option<i64>,
    af: Option<AfInfo>,
    /// SensorInfo's cropped area, width and height.
    area: Option<(usize, usize)>,
    orientation: Option<Orientation>,
}

/// The camera's focus point of the raw read from `file`, as fractions
/// of an area `area` pixels (the develop's crop, before the
/// orientation): `None` for a maker or a file whose point is not read,
/// a manual focus, and a record that does not place a point on the
/// picture.
pub(crate) fn focus_stream(
    file: impl Read + Seek,
    area: (usize, usize),
) -> Result<Option<FocusPoint>> {
    Ok(match read_note(file)? {
        Some(Note::Canon(note)) => note
            .af
            .as_ref()
            .and_then(|af| canon_focus(note.mode, af, area)),
        Some(Note::Spot(spot, _)) => spot.and_then(|s| spot_focus(&s, Some(area))),
        None => None,
    })
}

/// [`focus_stream`] with no decode: the point on the cropped area the
/// note's SensorInfo gives (the decoder's crop on every Canon read),
/// or the Canon image where it gives none, with the file's orientation.
pub(crate) fn focus_head(file: impl Read + Seek) -> Result<Option<(FocusPoint, Orientation)>> {
    let note = match read_note(file)? {
        Some(Note::Canon(note)) => note,
        Some(Note::Spot(spot, orientation)) => {
            return Ok(spot
                .and_then(|s| spot_focus(&s, None))
                .map(|p| (p, orientation.unwrap_or_default())));
        }
        None => return Ok(None),
    };
    let Some(af) = note.af.as_ref() else {
        return Ok(None);
    };
    let canon = (usize::try_from(af.canon.0), usize::try_from(af.canon.1));
    let area = match (note.area, canon) {
        (Some(a), _) => a,
        (None, (Ok(w), Ok(h))) => (w, h),
        _ => return Ok(None),
    };
    Ok(canon_focus(note.mode, af, area).map(|p| (p, note.orientation.unwrap_or_default())))
}

/// [`focus_head`] on the first bytes of a file `len` bytes long: the
/// read, or `None` when the walk needed a byte past `head`, and only
/// then, so the caller reads the file itself.
pub(crate) fn focus_in_head(
    head: Vec<u8>,
    len: u64,
) -> Option<Result<Option<(FocusPoint, Orientation)>>> {
    let mut head = Head {
        len: len.max(head.len() as u64),
        buf: head,
        at: 0,
        overran: false,
    };
    let read = focus_head(&mut head);
    (!head.overran).then_some(read)
}

/// A file's first bytes read as the whole file `len` bytes long: its
/// end is the file's, and a read past the bytes in hand fails and is
/// noted, where a read past the file's end fails as a file's would.
struct Head {
    buf: Vec<u8>,
    len: u64,
    at: u64,
    overran: bool,
}

impl Read for Head {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let have = self.buf.len() as u64;
        if self.at >= have {
            if self.at < self.len && !out.is_empty() {
                self.overran = true;
            }
            return if self.at < self.len {
                Err(std::io::ErrorKind::UnexpectedEof.into())
            } else {
                Ok(0)
            };
        }
        let n = out.len().min((have - self.at) as usize);
        let from = self.at as usize;
        out[..n].copy_from_slice(&self.buf[from..from + n]);
        self.at += n as u64;
        Ok(n)
    }
}

impl Seek for Head {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let at = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.at.checked_add_signed(d),
        };
        self.at = at.ok_or(std::io::ErrorKind::InvalidInput)?;
        Ok(self.at)
    }
}

/// What a file's maker note says of its focus.
enum Note {
    Canon(CanonNote),
    /// Another maker's: the point, none where the note gives none or
    /// says the camera did not focus, and the file's orientation.
    Spot(Option<Spot>, Option<Orientation>),
}

/// The maker note's reading of a CR3, a RAF, or a TIFF raw of a maker
/// read; `None` for any other file.
fn read_note(mut file: impl Read + Seek) -> Result<Option<Note>> {
    file.seek(SeekFrom::Start(0))?;
    let mut head = [0u8; 16];
    let n = read_up_to(&mut file, &mut head)?;
    let head = &head[..n];
    file.seek(SeekFrom::Start(0))?;
    if head.len() >= 12 && &head[4..8] == b"ftyp" && &head[8..12] == b"crx " {
        Ok(canon_cr3(file)?.map(Note::Canon))
    } else if head.starts_with(RAF_MAGIC) {
        let (spot, orientation) = fujifilm::read(file)?;
        Ok(Some(Note::Spot(spot, orientation)))
    } else if head.starts_with(b"II") || head.starts_with(b"MM") {
        tiff_raw(file)
    } else {
        Ok(None)
    }
}

/// A TIFF raw's note by its maker: a CR2's, a NEF's, an ARW's, an
/// RW2's, or an ORF's. Not a DNG, whose maker note, where it came
/// along, is not where this looks, and whose converter may have cut
/// the picture otherwise.
fn tiff_raw(file: impl Read + Seek) -> Result<Option<Note>> {
    let mut tiff = Tiff::new(file)?;
    let (ifd0, endian) = tiff_ifd0(&mut tiff)?;
    if find(&ifd0, TAG_DNG_VERSION).is_some() {
        return Ok(None);
    }
    let make = find(&ifd0, TAG_MAKE)
        .and_then(|f| tiff.ascii(&f))
        .unwrap_or_default();
    let orientation = orientation(&mut tiff, &ifd0);
    let spot = match Maker::from_make(&make) {
        Maker::Canon => return Ok(canon_cr2(&mut tiff, &ifd0, endian)?.map(Note::Canon)),
        Maker::Nikon => nikon::read(&mut tiff, &ifd0, endian)?,
        Maker::Sony => sony::read(&mut tiff, &ifd0, endian)?,
        Maker::Panasonic => panasonic::read(&mut tiff, &ifd0, endian)?,
        Maker::Olympus | Maker::OmDigital => olympus::read(&mut tiff, &ifd0, endian)?,
        Maker::Fujifilm | Maker::Other => return Ok(None),
    };
    Ok(Some(Note::Spot(spot, orientation)))
}

fn box_error(what: impl std::fmt::Display) -> Error {
    Error::Decode(format!("focus point: {what}"))
}

/// A box's header at `at`, inside a parent that ends at `end`: its
/// type, the user type of a `uuid`, where its contents start, and
/// where it ends.
type BoxHead = ([u8; 4], Option<[u8; 16]>, u64, u64);

fn box_at(file: &mut (impl Read + Seek), at: u64, end: u64) -> Result<BoxHead> {
    file.seek(SeekFrom::Start(at))?;
    let mut h = [0u8; 8];
    file.read_exact(&mut h)?;
    let size = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
    let typ = [h[4], h[5], h[6], h[7]];
    let (size, mut body) = match size {
        0 => (end - at, at + 8),
        1 => {
            let mut l = [0u8; 8];
            file.read_exact(&mut l)?;
            (u64::from_be_bytes(l), at + 16)
        }
        n => (n, at + 8),
    };
    let user = if &typ == b"uuid" {
        let mut u = [0u8; 16];
        file.read_exact(&mut u)?;
        body += 16;
        Some(u)
    } else {
        None
    };
    let stop = at
        .checked_add(size)
        .filter(|&s| s <= end && s >= body)
        .ok_or_else(|| box_error(format!("a box of {size} bytes at {at}")))?;
    Ok((typ, user, body, stop))
}

/// The boxes from `from` to `end`, walked by their headers until `want`
/// says one is wanted: its contents' start and end.
fn find_box(
    file: &mut (impl Read + Seek),
    from: u64,
    end: u64,
    want: impl Fn(&[u8; 4], Option<&[u8; 16]>) -> bool,
) -> Result<Option<(u64, u64)>> {
    let mut at = from;
    for _ in 0..MAX_BOXES {
        if at.saturating_add(8) > end {
            return Ok(None);
        }
        let (typ, user, body, stop) = box_at(file, at, end)?;
        if want(&typ, user.as_ref()) {
            return Ok(Some((body, stop)));
        }
        at = stop;
    }
    Ok(None)
}

/// A box's contents; `None` past [`MAX_CMT`].
fn contents(file: &mut (impl Read + Seek), (body, stop): (u64, u64)) -> Result<Option<Vec<u8>>> {
    if stop - body > MAX_CMT {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(body))?;
    let mut buf = vec![0u8; (stop - body) as usize];
    file.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// A CMT box of the moov's Canon box, by name.
fn cmt(
    file: &mut (impl Read + Seek),
    canon: (u64, u64),
    name: &[u8; 4],
) -> Result<Option<Vec<u8>>> {
    match find_box(file, canon.0, canon.1, |t, _| t == name)? {
        Some(b) => contents(file, b),
        None => Ok(None),
    }
}

/// The maker note of a CR3, its CMT3 box, and the Orientation of its
/// CMT1 (the picture's IFD0), found in the moov's Canon box. Only the
/// box headers on the way and the two boxes are read.
fn canon_cr3(mut file: impl Read + Seek) -> Result<Option<CanonNote>> {
    let len = file.seek(SeekFrom::End(0))?;
    let Some(moov) = find_box(&mut file, 0, len, |t, _| t == b"moov")? else {
        return Ok(None);
    };
    if moov.1 - moov.0 > MAX_MOOV {
        return Err(box_error("a moov too large to be one"));
    }
    let is_canon = |t: &[u8; 4], u: Option<&[u8; 16]>| t == b"uuid" && u == Some(&CANON_UUID);
    let Some(canon) = find_box(&mut file, moov.0, moov.1, is_canon)? else {
        return Ok(None);
    };
    let Some(cmt3) = cmt(&mut file, canon, b"CMT3")? else {
        return Ok(None);
    };
    let mut note = Tiff::new(Cursor::new(cmt3))?;
    let (fields, _) = tiff_ifd0(&mut note)?;
    let mut read = canon_note(&mut note, &fields);
    if let Some(cmt1) = cmt(&mut file, canon, b"CMT1")? {
        let mut ifd0 = Tiff::new(Cursor::new(cmt1))?;
        let (fields, _) = tiff_ifd0(&mut ifd0)?;
        read.orientation = orientation(&mut ifd0, &fields);
    }
    Ok(Some(read))
}

/// The maker note of a CR2: an IFD with no header whose offsets count
/// from the TIFF's start.
fn canon_cr2<R: Read + Seek>(
    tiff: &mut Tiff<R>,
    ifd0: &[Field],
    endian: Endian,
) -> Result<Option<CanonNote>> {
    let Some(note) = maker_note(tiff, ifd0, endian)? else {
        return Ok(None);
    };
    let orientation = orientation(tiff, ifd0);
    let fields = tiff.ifd(note.at, 0, note.endian)?;
    Ok(Some(CanonNote {
        orientation,
        ..canon_note(tiff, &fields)
    }))
}

fn orientation<R: Read + Seek>(tiff: &mut Tiff<R>, ifd: &[Field]) -> Option<Orientation> {
    let o = find(ifd, TAG_ORIENTATION).and_then(|f| tiff.ints(&f))?;
    Orientation::from_exif(u16::try_from(*o.first()?).ok()?)
}

/// FocusMode, AFInfo2, and SensorInfo from a maker note's IFD.
fn canon_note<R: Read + Seek>(tiff: &mut Tiff<R>, fields: &[Field]) -> CanonNote {
    let mut ints = |tag| find(fields, tag).and_then(|f| tiff.ints(&f));
    let mode = ints(CANON_CAMERA_SETTINGS).and_then(|s| s.get(CANON_FOCUS_MODE).copied());
    let af = ints(CANON_AF_INFO2).as_deref().and_then(af_info);
    let area = ints(CANON_SENSOR_INFO).and_then(|s| {
        let (l, t, r, b) = (*s.get(5)?, *s.get(6)?, *s.get(7)?, *s.get(8)?);
        let w = usize::try_from(r - l + 1).ok().filter(|&w| w > 0)?;
        let h = usize::try_from(b - t + 1).ok().filter(|&h| h > 0)?;
        Some((w, h))
    });
    CanonNote {
        mode,
        af,
        area,
        orientation: None,
    }
}

/// AFInfo2's elements read into its fields; `None` for a record too
/// short for the counts it gives. Every element is read as the 16 bits
/// it is, the positions as signed.
fn af_info(v: &[i64]) -> Option<AfInfo> {
    let head = v.get(..AF_HEAD)?;
    let (mode, n, valid) = (head[1], usize::try_from(head[2]).ok()?, head[3]);
    let words = n.div_ceil(16);
    let body = v.get(AF_HEAD..AF_HEAD + 4 * n + words)?;
    let signed = |i: usize| body[i] as u16 as i16 as i64;
    let in_focus = |k: usize| (body[4 * n + k / 16] >> (k % 16)) & 1 == 1;
    let valid = usize::try_from(valid).ok()?.min(n);
    let areas = (0..valid)
        .map(|k| {
            let area = AfArea {
                w: body[k],
                h: body[n + k],
                x: signed(2 * n + k),
                y: signed(3 * n + k),
            };
            (area, in_focus(k))
        })
        .collect();
    Some(AfInfo {
        mode,
        canon: (head[4], head[5]),
        af: (head[6], head[7]),
        areas,
    })
}

/// Whether CameraSettings' FocusMode is a manual one: Manual Focus (3),
/// Manual Focus (6), and Pan Focus (16).
fn manual_focus(mode: i64) -> bool {
    matches!(mode, 3 | 6 | 16)
}

/// The focus point a record places on an area `area` pixels: of the
/// areas the camera found in focus, the one nearest the middle, as
/// fractions of the area. The Canon image (the camera's own picture,
/// of the size its JPEG is) is taken to sit centered on the area, and
/// the AF image to cover it, scaled. `None` on manual focus (by
/// FocusMode, or AFAreaMode Off), with no area in focus, and for a
/// record whose point would not lie on the picture.
fn canon_focus(mode: Option<i64>, af: &AfInfo, (aw, ah): (usize, usize)) -> Option<FocusPoint> {
    if mode.is_some_and(manual_focus) || af.mode == 0 {
        return None;
    }
    let (cw, ch) = af.canon;
    let (fw, fh) = af.af;
    if cw <= 0 || ch <= 0 || fw <= 0 || fh <= 0 || aw == 0 || ah == 0 {
        return None;
    }
    // The Canon image is the area, within a few pixels: an image cut
    // smaller in camera (an aspect, a crop mode) may have an AF image
    // of another picture, and is refused rather than guessed at. The
    // AF image scales to it alike on both axes, or it is of another
    // shape than the picture.
    let (aw, ah) = (aw as f64, ah as f64);
    let (cw, ch, fw, fh) = (cw as f64, ch as f64, fw as f64, fh as f64);
    let near = |a: f64, b: f64| (a - b).abs() <= 0.01 * a.max(b);
    if !near(cw, aw) || !near(ch, ah) || !near(cw / fw, ch / fh) {
        return None;
    }
    let focused: Vec<AfArea> = af
        .areas
        .iter()
        .filter(|(a, on)| *on && a.w > 0 && a.h > 0)
        .map(|(a, _)| *a)
        .collect();
    let chosen = middle_of(&focused)?;
    let (sx, sy) = (cw / fw, ch / fh);
    let (w, h) = (chosen.w as f64 * sx, chosen.h as f64 * sy);
    let cx = (aw - cw) / 2.0 + cw / 2.0 + chosen.x as f64 * sx;
    let cy = (ah - ch) / 2.0 + ch / 2.0 - chosen.y as f64 * sy;
    let rect = [(cx - w / 2.0) / aw, (cy - h / 2.0) / ah, w / aw, h / ah];
    let on = (0.0..=1.0).contains(&(cx / aw))
        && (0.0..=1.0).contains(&(cy / ah))
        && rect[2] <= 1.0
        && rect[3] <= 1.0;
    on.then(|| FocusPoint {
        area: rect.map(|v| v as f32),
        aspect: (aw / ah) as f32,
    })
}

/// Of the areas in focus, the one nearest the frame's center. When a
/// zone or the whole area finds several, they are all at the
/// subject's distance, and so is the ground the subject stands on: a
/// band of points that runs out to the frame's edge and pulls their
/// mean down off the subject (seen on a whole-area frame of two
/// people on rocks). The subject is nearer the middle than the ground
/// it stands on. Always a point the camera found sharp, never the gap
/// between two subjects that a mean can fall in. The first of two at
/// one distance.
fn middle_of(areas: &[AfArea]) -> Option<AfArea> {
    let d = |a: &AfArea| (a.x as f64).powi(2) + (a.y as f64).powi(2);
    areas
        .iter()
        .copied()
        .reduce(|best, a| if d(&a) < d(&best) { a } else { best })
}

/// A point as another maker's note gives it: the area as fractions
/// `[x, y, w, h]` of the picture the maker measures in, before the
/// orientation, and that picture's size in the maker's units, which
/// says its shape.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spot {
    rect: [f64; 4],
    of: (f64, f64),
}

/// A point the maker gives no size for is drawn as a square this many
/// times smaller than the picture's shorter side: about one AF area of
/// the bodies that do give one.
const POINT_SPAN: f64 = 16.0;

impl Spot {
    /// The area centered at (`cx`, `cy`), `w` by `h`, in units of a
    /// picture `of`; a size of zero is a point, drawn [`POINT_SPAN`]
    /// across. `None` for a picture of no size or a number not finite.
    fn at(cx: f64, cy: f64, w: f64, h: f64, of: (f64, f64)) -> Option<Spot> {
        if !(of.0 > 0.0 && of.1 > 0.0 && of.0.is_finite() && of.1.is_finite()) {
            return None;
        }
        let (w, h) = if w > 0.0 && h > 0.0 {
            (w, h)
        } else {
            let s = of.0.min(of.1) / POINT_SPAN;
            (s, s)
        };
        let rect = [
            (cx - w / 2.0) / of.0,
            (cy - h / 2.0) / of.1,
            w / of.0,
            h / of.1,
        ];
        rect.iter()
            .all(|v| v.is_finite())
            .then_some(Spot { rect, of })
    }

    /// The area `[left, top, right, bottom]` in units of `of`.
    fn spanning([l, t, r, b]: [f64; 4], of: (f64, f64)) -> Option<Spot> {
        Spot::at((l + r) / 2.0, (t + b) / 2.0, r - l, b - t, of)
    }
}

/// Of the boxes `[left, top, right, bottom]` that hold the point
/// (`x`, `y`), the smallest: a face's eye over the face, the face over
/// the body.
fn smallest_holding(
    boxes: impl IntoIterator<Item = [f64; 4]>,
    (x, y): (f64, f64),
) -> Option<[f64; 4]> {
    let size = |b: &[f64; 4]| (b[2] - b[0]) * (b[3] - b[1]);
    boxes
        .into_iter()
        .filter(|b| {
            b[0] < b[2] && b[1] < b[3] && (b[0]..=b[2]).contains(&x) && (b[1]..=b[3]).contains(&y)
        })
        .reduce(|best, b| if size(&b) < size(&best) { b } else { best })
}

/// A rational value's elements, unsigned (RATIONAL) or signed
/// (SRATIONAL); an element over zero is not a number.
fn rationals<R: Read + Seek>(tiff: &mut Tiff<R>, f: &Field) -> Option<Vec<f64>> {
    let b = tiff.bytes(f)?;
    let word = |c: &[u8]| {
        let c = [c[0], c[1], c[2], c[3]];
        match f.endian {
            Endian::Little => u32::from_le_bytes(c),
            Endian::Big => u32::from_be_bytes(c),
        }
    };
    let pairs = b.as_chunks::<8>().0.iter();
    Some(match f.typ {
        5 => pairs
            .map(|p| f64::from(word(&p[..4])) / f64::from(word(&p[4..])))
            .collect(),
        10 => pairs
            .map(|p| f64::from(word(&p[..4]) as i32) / f64::from(word(&p[4..]) as i32))
            .collect(),
        _ => return None,
    })
}

/// A 16-bit word of a record at `at`, in the note's byte order.
fn u16_in(endian: Endian, b: &[u8], at: usize) -> Option<u16> {
    let c = b.get(at..at.checked_add(2)?)?;
    Some(match endian {
        Endian::Little => u16::from_le_bytes([c[0], c[1]]),
        Endian::Big => u16::from_be_bytes([c[0], c[1]]),
    })
}

/// Whether two sizes are of one shape, their aspects within `tol` of
/// each other.
fn same_shape((aw, ah): (f64, f64), (bw, bh): (f64, f64), tol: f64) -> bool {
    let (a, b) = (aw / ah, bw / bh);
    a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0 && (a - b).abs() <= tol * a.max(b)
}

/// The focus point a maker's spot places on a decode's crop `area`
/// pixels, or with no decode (`None`) on the maker's own picture, the
/// whole of what the camera kept. The maker's picture is taken to be
/// the crop, scaled: one of another shape (an aspect chosen in camera
/// that the raw does not keep) is refused rather than guessed at. As
/// with Canon's, a point whose center is off the picture or whose area
/// is larger than it is none.
fn spot_focus(spot: &Spot, area: Option<(usize, usize)>) -> Option<FocusPoint> {
    if let Some((aw, ah)) = area
        && !same_shape((aw as f64, ah as f64), spot.of, 0.01)
    {
        return None;
    }
    let [x, y, w, h] = spot.rect;
    let area = spot.rect.map(|v| v as f32);
    // A size so small it is no size once single precision.
    let on = spot.rect.iter().all(|v| v.is_finite())
        && area[2] > 0.0
        && area[3] > 0.0
        && w <= 1.0
        && h <= 1.0
        && (0.0..=1.0).contains(&(x + w / 2.0))
        && (0.0..=1.0).contains(&(y + h / 2.0));
    let aspect = (spot.of.0 / spot.of.1) as f32;
    (on && aspect.is_finite() && aspect > 0.0).then_some(FocusPoint { area, aspect })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// An AFInfo2 record's elements, every area valid.
    fn record(
        mode: u16,
        canon: (u16, u16),
        af: (u16, u16),
        areas: &[(u16, u16, i16, i16, bool)],
    ) -> Vec<u16> {
        let n = areas.len();
        let words = n.div_ceil(16);
        let mut v = vec![0, mode, n as u16, n as u16, canon.0, canon.1, af.0, af.1];
        v.extend(areas.iter().map(|a| a.0));
        v.extend(areas.iter().map(|a| a.1));
        v.extend(areas.iter().map(|a| a.2 as u16));
        v.extend(areas.iter().map(|a| a.3 as u16));
        let mut focus = vec![0u16; words];
        for (k, a) in areas.iter().enumerate() {
            if a.4 {
                focus[k / 16] |= 1 << (k % 16);
            }
        }
        v.extend(&focus);
        // AFPointsSelected: all of them.
        v.extend(std::iter::repeat_n(0xffff, words));
        v[0] = 2 * v.len() as u16;
        v
    }

    fn ints(v: &[u16]) -> Vec<i64> {
        v.iter().map(|&n| i64::from(n)).collect()
    }

    fn focus(mode: Option<i64>, v: &[u16], area: (usize, usize)) -> Option<FocusPoint> {
        canon_focus(mode, &af_info(&ints(v))?, area)
    }

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
    }

    /// One area right of and above the middle lands right of and above
    /// the middle of the picture: Y counts up.
    #[test]
    fn one_area_lands_where_it_says_with_y_up() {
        let v = record(
            13,
            (6000, 4000),
            (6000, 4000),
            &[(400, 200, 1500, 1000, true)],
        );
        let p = focus(Some(1), &v, (6000, 4000)).unwrap();
        // Center (4500, 1000), 400 by 200.
        assert!(close(
            p.area,
            [4300.0 / 6000.0, 0.225, 400.0 / 6000.0, 0.05]
        ));
    }

    /// An AF image smaller than the Canon image is scaled up to it, and
    /// a Canon image smaller than the area sits in its middle.
    #[test]
    fn the_af_image_is_scaled_and_the_canon_image_centered() {
        let v = record(
            13,
            (6000, 4000),
            (3000, 2000),
            &[(100, 100, -500, -500, true)],
        );
        let p = focus(Some(1), &v, (6020, 4010)).unwrap();
        let (cx, cy) = (10.0 + 3000.0 - 1000.0, 5.0 + 2000.0 + 1000.0);
        assert!(close(
            p.area,
            [
                (cx - 100.0) / 6020.0,
                (cy - 100.0) / 4010.0,
                200.0 / 6020.0,
                200.0 / 4010.0
            ]
        ));
    }

    /// Of a subject in focus and the ground band under it, the subject's
    /// point nearest the middle; an area not in focus, nearer still, is
    /// not counted.
    #[test]
    fn several_in_focus_take_the_one_nearest_the_middle() {
        let mut areas = vec![(50, 50, 0, 0, false)];
        // The subject, a column left of the middle.
        areas.extend((0..4).map(|k| (50, 50, -400, 200 - 100 * k, true)));
        // The ground, a band along the bottom.
        areas.extend((0..12).map(|k| (50, 50, -1200 + 200 * k, -1800, true)));
        let v = record(13, (6000, 4000), (6000, 4000), &areas);
        let p = focus(Some(1), &v, (6000, 4000)).unwrap();
        let (x, y) = p.center(crate::raw::Orientation::Normal);
        // The subject's point at (-400, 0): picture (2600, 2000).
        assert!((x - 2600.0 / 6000.0).abs() < 1e-4, "{x}");
        assert!((y - 0.5).abs() < 1e-4, "{y}");
    }

    #[test]
    fn manual_focus_and_nothing_in_focus_have_no_point() {
        let one = [(400, 400, 0, 0, true)];
        let v = record(13, (6000, 4000), (6000, 4000), &one);
        assert!(focus(Some(1), &v, (6000, 4000)).is_some());
        assert!(focus(None, &v, (6000, 4000)).is_some());
        for manual in [3, 6, 16] {
            assert_eq!(focus(Some(manual), &v, (6000, 4000)), None);
        }
        let off = record(0, (6000, 4000), (6000, 4000), &one);
        assert_eq!(focus(Some(1), &off, (6000, 4000)), None);
        let none = record(13, (6000, 4000), (6000, 4000), &[(400, 400, 0, 0, false)]);
        assert_eq!(focus(Some(1), &none, (6000, 4000)), None);
    }

    /// A point off the picture, a Canon image larger than the area, a
    /// record of no size, and a record cut short are no point.
    #[test]
    fn implausible_records_have_no_point() {
        let off = record(13, (6000, 4000), (6000, 4000), &[(100, 100, 3200, 0, true)]);
        assert_eq!(focus(Some(1), &off, (6000, 4000)), None);
        let big = record(13, (6000, 4000), (6000, 4000), &[(100, 100, 0, 0, true)]);
        assert_eq!(focus(Some(1), &big, (5000, 3000)), None);
        let empty = record(13, (0, 0), (6000, 4000), &[(100, 100, 0, 0, true)]);
        assert_eq!(focus(Some(1), &empty, (6000, 4000)), None);
        let zero = record(13, (6000, 4000), (6000, 4000), &[(0, 0, 0, 0, true)]);
        assert_eq!(focus(Some(1), &zero, (6000, 4000)), None);
        let short = record(13, (6000, 4000), (6000, 4000), &[(100, 100, 0, 0, true); 3]);
        assert_eq!(af_info(&ints(&short[..12])), None);
    }

    /// An AF image scaled to the picture unlike on its two axes, and a
    /// Canon image cut smaller than the crop (an aspect or a crop mode
    /// chosen in camera), are refused rather than guessed at.
    #[test]
    fn an_af_image_of_another_shape_or_picture_is_refused() {
        let one = [(100, 100, 0, 0, true)];
        let squeezed = record(13, (6000, 4000), (3000, 1000), &one);
        assert_eq!(focus(Some(1), &squeezed, (6000, 4000)), None);
        let slightly = record(13, (6000, 4000), (6000, 3990), &one);
        assert!(focus(Some(1), &slightly, (6000, 4000)).is_some());
        let cut = record(13, (6000, 4000), (6000, 4000), &one);
        assert_eq!(focus(Some(1), &cut, (8000, 5300)), None);
        assert_eq!(focus(Some(1), &cut, (6000, 4500)), None);
        assert!(focus(Some(1), &cut, (6010, 4010)).is_some());
    }

    /// A CR3's boxes: ftyp, a moov holding a track box and the Canon
    /// box with CMT1 and CMT3 (each a little-endian TIFF), then mdat.
    fn cr3(cmt1: &[u8], cmt3: &[u8], large_mdat: bool) -> Vec<u8> {
        let boxed = |typ: &[u8; 4], body: &[u8]| {
            let mut b = (8 + body.len() as u32).to_be_bytes().to_vec();
            b.extend(typ);
            b.extend(body);
            b
        };
        let mut canon = CANON_UUID.to_vec();
        canon.extend(boxed(b"CNCV", b"CanonCR3_001/00.09.00/00.00.00"));
        canon.extend(boxed(b"CMT1", cmt1));
        canon.extend(boxed(b"CMT2", &[0; 20]));
        canon.extend(boxed(b"CMT3", cmt3));
        let mut moov = boxed(b"mvhd", &[0; 100]);
        moov.extend(boxed(b"uuid", &canon));
        moov.extend(boxed(b"trak", &[0; 40]));
        let mut out = boxed(b"ftyp", b"crx \0\0\0\x01crx isom");
        out.extend(boxed(b"moov", &moov));
        if large_mdat {
            // A 64-bit size, as a big mdat has.
            out.extend(1u32.to_be_bytes());
            out.extend(b"mdat");
            out.extend((16u64 + 64).to_be_bytes());
            out.extend([7u8; 64]);
        } else {
            out.extend(boxed(b"mdat", &[7; 64]));
        }
        out
    }

    /// A little-endian TIFF of one IFD at 8 with these SHORT entries.
    fn tiff_of(entries: &[(u16, Vec<u16>)]) -> Vec<u8> {
        let n = entries.len();
        let mut data_at = 8 + 2 + 12 * n as u32 + 4;
        let mut out = b"II*\0".to_vec();
        out.extend(8u32.to_le_bytes());
        out.extend((n as u16).to_le_bytes());
        let mut data = Vec::new();
        for (tag, v) in entries {
            out.extend(tag.to_le_bytes());
            out.extend(3u16.to_le_bytes());
            out.extend((v.len() as u32).to_le_bytes());
            let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
            if bytes.len() <= 4 {
                let mut inline = bytes.clone();
                inline.resize(4, 0);
                out.extend(inline);
            } else {
                out.extend(data_at.to_le_bytes());
                data_at += bytes.len() as u32;
                data.extend(bytes);
            }
        }
        out.extend(0u32.to_le_bytes());
        out.extend(data);
        out
    }

    fn sensor_info(area: (u16, u16)) -> Vec<u16> {
        let mut s = vec![0u16; 17];
        s[0] = 34;
        (s[1], s[2]) = (area.0 + 200, area.1 + 120);
        (s[5], s[6]) = (144, 112);
        (s[7], s[8]) = (144 + area.0 - 1, 112 + area.1 - 1);
        s
    }

    /// The head read of a CR3 finds the note and the orientation in the
    /// moov's Canon box, with the area from SensorInfo, through a large
    /// mdat's 64-bit size as well; the decode's read agrees.
    #[test]
    fn a_cr3s_head_is_read_through_its_boxes() {
        let af = record(
            13,
            (6000, 4000),
            (6000, 4000),
            &[(400, 200, 1500, 1000, true)],
        );
        let mut settings = vec![0u16; 40];
        settings[CANON_FOCUS_MODE] = 1;
        let note = tiff_of(&[
            (CANON_CAMERA_SETTINGS, settings),
            (CANON_AF_INFO2, af),
            (CANON_SENSOR_INFO, sensor_info((6000, 4000))),
        ]);
        let ifd0 = tiff_of(&[(TAG_ORIENTATION, vec![8])]);
        for large in [false, true] {
            let file = cr3(&ifd0, &note, large);
            let (p, o) = focus_head(Cursor::new(&file)).unwrap().unwrap();
            assert_eq!(o, Orientation::Rotate270);
            assert!(close(
                p.area,
                [4300.0 / 6000.0, 0.225, 400.0 / 6000.0, 0.05]
            ));
            let decoded = focus_stream(Cursor::new(&file), (6000, 4000)).unwrap();
            assert_eq!(decoded, Some(p));
        }
        // A crop of another size than the note's area: no point.
        let file = cr3(&ifd0, &note, false);
        assert_eq!(
            focus_stream(Cursor::new(&file), (8000, 5300)).unwrap(),
            None
        );
    }

    /// A head that holds the whole moov is the answer, a point or none
    /// (a manual focus here); only a walk that needs a byte past the
    /// head says to read the file, wherever in the walk that is.
    #[test]
    fn a_head_is_final_unless_the_walk_ran_past_it() {
        let af = record(13, (6000, 4000), (6000, 4000), &[(400, 200, 0, 0, true)]);
        let note = |mode: u16| {
            let mut settings = vec![0u16; 40];
            settings[CANON_FOCUS_MODE] = mode;
            tiff_of(&[
                (CANON_CAMERA_SETTINGS, settings),
                (CANON_AF_INFO2, af.clone()),
            ])
        };
        let ifd0 = tiff_of(&[(TAG_ORIENTATION, vec![1])]);
        for (mode, found) in [(1, true), (3, false)] {
            let mut file = cr3(&ifd0, &note(mode), false);
            let len = file.len() as u64;
            // The mdat's data, past the head, is never wanted.
            let head = file[..file.len() - 40].to_vec();
            let read = focus_in_head(head, len + 1_000_000).expect("the head was enough");
            assert_eq!(read.unwrap().is_some(), found, "focus mode {mode}");
            // Cut inside the moov: read the file.
            let cmt3 = file.windows(4).position(|w| w == b"CMT3").unwrap();
            file.truncate(cmt3 + 20);
            assert!(focus_in_head(file, len).is_none(), "focus mode {mode}");
        }
        // A file that is all head and no CR3 is an answer at once.
        let other = focus_in_head(vec![1, 2, 3], 3).unwrap();
        assert_eq!(other.unwrap(), None);
    }

    /// Box sizes that run past their parent, or a size of a header's
    /// alone, are errors, not reads past the end or a loop.
    #[test]
    fn damaged_cr3_boxes_are_errors() {
        let note = tiff_of(&[(
            CANON_AF_INFO2,
            record(13, (6000, 4000), (6000, 4000), &[(1, 1, 0, 0, true)]),
        )]);
        let good = cr3(&tiff_of(&[]), &note, false);
        let moov = good.windows(4).position(|w| w == b"moov").unwrap() - 4;
        let mut past = good.clone();
        past[moov..moov + 4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(focus_head(Cursor::new(past)).is_err());
        let mut tiny = good.clone();
        let cmt3 = good.windows(4).position(|w| w == b"CMT3").unwrap() - 4;
        tiny[cmt3..cmt3 + 4].copy_from_slice(&4u32.to_be_bytes());
        assert!(focus_head(Cursor::new(tiny)).is_err());
        // Cut short inside the moov.
        assert!(focus_head(Cursor::new(&good[..moov + 40])).is_err());
    }

    /// Only the valid areas count, though the masks cover them all.
    #[test]
    fn areas_past_the_valid_count_are_not_read() {
        let mut v = record(
            13,
            (6000, 4000),
            (6000, 4000),
            &[(100, 100, 0, 0, false), (100, 100, 900, 0, true)],
        );
        v[3] = 1;
        assert_eq!(focus(Some(1), &v, (6000, 4000)), None);
    }

    /// A little-endian TIFF whose IFD0 says Canon and points at an Exif
    /// IFD whose maker note is a Canon IFD, offsets from the file's
    /// start, as a CR2 lays them out.
    fn cr2(settings: &[u16], af: &[u16]) -> Vec<u8> {
        let le16 = |v: &[u16]| v.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<u8>>();
        // IFD0 at 8: Make, ExifIFD. Exif IFD, then the note's IFD,
        // then the values.
        let make = b"Canon\0".to_vec();
        let ifd0_at = 8u32;
        let ifd0_len = 2 + 2 * 12 + 4;
        let exif_at = ifd0_at + ifd0_len;
        let exif_len = 2 + 12 + 4;
        let note_at = exif_at + exif_len;
        let note_len = 2 + 2 * 12 + 4;
        let make_at = note_at + note_len;
        let settings_at = make_at + make.len() as u32;
        let af_at = settings_at + 2 * settings.len() as u32;
        let end = af_at + 2 * af.len() as u32;
        let entry = |tag: u16, typ: u16, count: u32, value: u32| {
            let mut e = tag.to_le_bytes().to_vec();
            e.extend(typ.to_le_bytes());
            e.extend(count.to_le_bytes());
            e.extend(value.to_le_bytes());
            e
        };
        let mut out = b"II*\0".to_vec();
        out.extend(ifd0_at.to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(entry(0x010f, 2, make.len() as u32, make_at));
        out.extend(entry(0x8769, 4, 1, exif_at));
        out.extend(0u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(entry(0x927c, 7, end - note_at, note_at));
        out.extend(0u32.to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(entry(0x0001, 3, settings.len() as u32, settings_at));
        out.extend(entry(0x0026, 3, af.len() as u32, af_at));
        out.extend(0u32.to_le_bytes());
        out.extend(make);
        out.extend(le16(settings));
        out.extend(le16(af));
        assert_eq!(out.len() as u32, end);
        out
    }

    #[test]
    fn a_cr2s_note_is_read() {
        let af = record(
            2,
            (6000, 4000),
            (6000, 4000),
            &[(300, 300, -1500, -1000, true)],
        );
        let mut settings = vec![0u16; 40];
        settings[0] = 80;
        settings[CANON_FOCUS_MODE] = 0;
        let p = focus_stream(Cursor::new(cr2(&settings, &af)), (6000, 4000))
            .unwrap()
            .unwrap();
        assert!(close(p.area, [0.225, 0.7125, 0.05, 0.075]));
        settings[CANON_FOCUS_MODE] = 3;
        let manual = focus_stream(Cursor::new(cr2(&settings, &af)), (6000, 4000)).unwrap();
        assert_eq!(manual, None);
    }

    #[test]
    fn another_file_has_no_point() {
        assert_eq!(
            focus_stream(Cursor::new(vec![1u8, 2, 3]), (10, 10)).unwrap(),
            None
        );
        let mut nikon = cr2(&[0; 10], &[0; 10]);
        let at = nikon.windows(6).position(|w| w == b"Canon\0").unwrap();
        nikon[at..at + 6].copy_from_slice(b"NIKON\0");
        assert_eq!(focus_stream(Cursor::new(nikon), (10, 10)).unwrap(), None);
    }

    /// Turned with the picture: a point at the top right of the area is
    /// at the bottom right after a quarter turn clockwise.
    #[test]
    fn the_point_turns_with_the_picture() {
        let p = FocusPoint {
            area: [0.8, 0.1, 0.1, 0.1],
            aspect: 1.5,
        };
        assert_eq!(
            p.oriented_aspect(crate::raw::Orientation::Rotate90),
            1.0 / 1.5
        );
        assert_eq!(p.oriented_aspect(crate::raw::Orientation::Rotate180), 1.5);
        let (x, y) = p.center(crate::raw::Orientation::Rotate90);
        assert!(
            (x - 0.85).abs() < 1e-5 && (y - 0.85).abs() < 1e-5,
            "{x} {y}"
        );
    }
}
