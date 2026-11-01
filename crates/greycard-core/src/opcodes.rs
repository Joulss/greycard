//! A DNG's opcode lists: the corrections its maker leaves to the reader,
//! in the binary layout of the DNG specification (1.7, chapter 7).
//!
//! A list is big-endian whatever the file's byte order: a count, then
//! each opcode as its id, the DNG version it was written for, its flags,
//! the byte count of its parameters, and the parameters. Of the opcodes
//! only `GainMap` is read for now; every other is named in the log and
//! left, as a reader may leave one flagged optional. One that is not
//! optional is left too, with the log saying so: a picture with a
//! correction missing is more use than no picture.

use std::fmt;

/// The opcode's flag bit saying a reader may skip it.
const OPTIONAL: u32 = 1;

/// The `GainMap` opcode's id.
const GAIN_MAP: u32 = 9;

/// The largest number of opcodes a list is read for; a count past it is
/// a damaged list, not a camera's.
const MAX_OPCODES: u32 = 1024;

/// The most points a gain map is read with, every plane counted: a
/// phone's maps are 1200, and a map past this is a damaged one.
const MAX_MAP_POINTS: u64 = 1 << 22;

/// A `GainMap` opcode: a grid of gains over an area of the image,
/// interpolated between its points and multiplied into the samples of
/// the planes it names. Phones correct their lens shading with one a
/// color of the mosaic.
///
/// The area is in the coordinates of the image the list runs on: for
/// `OpcodeList2`, the active area, its top left corner at 0, 0. The map's
/// points sit at relative positions over that whole image, 0 at its top
/// or left edge and 1 at its bottom or right.
#[derive(Debug, Clone, PartialEq)]
pub struct GainMap {
    /// The area the map applies to: rows `top..bottom`, columns
    /// `left..right`. A file's empty area is the whole image, as the
    /// DNG SDK reads one, and is held here as `0..u32::MAX` each way.
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
    /// The first plane it applies to, and how many.
    pub plane: u32,
    pub planes: u32,
    /// Every `row_pitch`th row and `col_pitch`th column of the area from
    /// its corner: a map for one color of a 2x2 mosaic has pitch 2.
    pub row_pitch: u32,
    pub col_pitch: u32,
    /// The grid's points down and across.
    pub points_v: u32,
    pub points_h: u32,
    /// The distance between points, and the first point's position, in
    /// relative image coordinates.
    pub spacing_v: f64,
    pub spacing_h: f64,
    pub origin_v: f64,
    pub origin_h: f64,
    /// The planes of the map; a plane past the last takes the last's.
    pub map_planes: u32,
    /// The gains, row by row, each point's planes together.
    pub gains: Vec<f32>,
}

impl GainMap {
    /// The gain at point (`v`, `h`) of map plane `plane`.
    #[inline]
    pub fn point(&self, v: usize, h: usize, plane: usize) -> f32 {
        self.gains[(v * self.points_h as usize + h) * self.map_planes as usize + plane]
    }
}

/// One opcode of a list, its parameters unread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opcode<'a> {
    pub id: u32,
    pub flags: u32,
    pub params: &'a [u8],
}

impl Opcode<'_> {
    /// Whether the file allows a reader to skip it.
    pub fn optional(&self) -> bool {
        self.flags & OPTIONAL != 0
    }
}

impl fmt::Display for Opcode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (opcode {})", opcode_name(self.id), self.id)
    }
}

/// The specification's name for an opcode id.
pub fn opcode_name(id: u32) -> &'static str {
    match id {
        1 => "WarpRectilinear",
        2 => "WarpFisheye",
        3 => "FixVignetteRadial",
        4 => "FixBadPixelsConstant",
        5 => "FixBadPixelsList",
        6 => "TrimBounds",
        7 => "MapTable",
        8 => "MapPolynomial",
        9 => "GainMap",
        10 => "DeltaPerRow",
        11 => "DeltaPerColumn",
        12 => "ScalePerRow",
        13 => "ScalePerColumn",
        14 => "WarpRectilinear2",
        _ => "an unknown opcode",
    }
}

/// The opcodes of a list, in order. A list cut short gives the opcodes
/// before the cut; a list that will not read at all gives none.
pub fn parse_list(bytes: &[u8]) -> Vec<Opcode<'_>> {
    let mut at = Reader { bytes, at: 0 };
    let Some(count) = at.u32() else {
        return Vec::new();
    };
    if count > MAX_OPCODES {
        log::warn!("DNG opcode list of {count} opcodes left unread");
        return Vec::new();
    }
    let mut opcodes = Vec::new();
    for _ in 0..count {
        let (Some(id), Some(_version), Some(flags), Some(len)) =
            (at.u32(), at.u32(), at.u32(), at.u32())
        else {
            log::warn!("DNG opcode list cut short");
            break;
        };
        let Some(params) = at.take(len as usize) else {
            log::warn!("DNG opcode list cut short in {}", opcode_name(id));
            break;
        };
        opcodes.push(Opcode { id, flags, params });
    }
    opcodes
}

/// The gain maps of an `OpcodeList2`, in order, and every other opcode
/// of it named in the log as left undone. A gain map that will not read
/// is left the same way.
pub fn gain_maps(list: &[u8]) -> Vec<GainMap> {
    let mut maps = Vec::new();
    for op in parse_list(list) {
        if op.id == GAIN_MAP {
            match parse_gain_map(op.params) {
                Some(map) => maps.push(map),
                None => log::warn!("DNG {op} out of shape, left undone"),
            }
        } else {
            log_left(&op, "OpcodeList2");
        }
    }
    maps
}

/// Say in the log that an opcode is not applied, and whether the file
/// allowed that.
pub fn log_left(op: &Opcode<'_>, list: &str) {
    if op.optional() {
        log::info!("DNG {list}: {op} not applied (optional)");
    } else {
        log::warn!("DNG {list}: {op} not applied, though the file does not mark it optional");
    }
}

/// A `GainMap`'s parameters, or `None` when they are out of shape: a
/// zero pitch or plane count, a grid of no points or of more than a
/// reader takes, spacings that are not positive where there is more
/// than one point, or gains that are not the count the grid says. An
/// empty area is read as the whole image.
pub fn parse_gain_map(params: &[u8]) -> Option<GainMap> {
    let mut r = Reader {
        bytes: params,
        at: 0,
    };
    let mut map = GainMap {
        top: r.u32()?,
        left: r.u32()?,
        bottom: r.u32()?,
        right: r.u32()?,
        plane: r.u32()?,
        planes: r.u32()?,
        row_pitch: r.u32()?,
        col_pitch: r.u32()?,
        points_v: r.u32()?,
        points_h: r.u32()?,
        spacing_v: r.f64()?,
        spacing_h: r.f64()?,
        origin_v: r.f64()?,
        origin_h: r.f64()?,
        map_planes: r.u32()?,
        gains: Vec::new(),
    };
    if map.bottom <= map.top || map.right <= map.left {
        (map.top, map.left, map.bottom, map.right) = (0, 0, u32::MAX, u32::MAX);
    }
    // Checked: a product that wrapped could match the byte count and
    // index past the gains in the develop.
    let count = u64::from(map.points_v)
        .checked_mul(u64::from(map.points_h))
        .and_then(|n| n.checked_mul(u64::from(map.map_planes)))?;
    let spaced = |points: u32, spacing: f64| points == 1 || (spacing.is_finite() && spacing > 0.0);
    if map.planes == 0
        || map.row_pitch == 0
        || map.col_pitch == 0
        || count == 0
        || count > MAX_MAP_POINTS
        || !spaced(map.points_v, map.spacing_v)
        || !spaced(map.points_h, map.spacing_h)
        || !map.origin_v.is_finite()
        || !map.origin_h.is_finite()
        || r.bytes.len() - r.at != count as usize * 4
    {
        return None;
    }
    map.gains = (0..count).map(|_| r.f32()).collect::<Option<Vec<_>>>()?;
    // A gain that is not a number would spread through the picture.
    if map.gains.iter().any(|g| !g.is_finite()) {
        return None;
    }
    Some(map)
}

/// Big-endian reads off a slice, `None` past its end.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let out = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(out)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    fn f64(&mut self) -> Option<f64> {
        Some(f64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }
}

/// An opcode list's bytes from its parts, for the tests here and the
/// develop's: each opcode as (id, flags, parameters).
#[cfg(test)]
pub(crate) fn list_bytes(opcodes: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = (opcodes.len() as u32).to_be_bytes().to_vec();
    for (id, flags, params) in opcodes {
        out.extend(id.to_be_bytes());
        out.extend([1, 3, 0, 0]);
        out.extend(flags.to_be_bytes());
        out.extend((params.len() as u32).to_be_bytes());
        out.extend(params);
    }
    out
}

/// A `GainMap`'s parameters from the map, as a file lays them out.
#[cfg(test)]
pub(crate) fn gain_map_bytes(map: &GainMap) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [
        map.top,
        map.left,
        map.bottom,
        map.right,
        map.plane,
        map.planes,
        map.row_pitch,
        map.col_pitch,
        map.points_v,
        map.points_h,
    ] {
        out.extend(v.to_be_bytes());
    }
    for v in [map.spacing_v, map.spacing_h, map.origin_v, map.origin_h] {
        out.extend(v.to_be_bytes());
    }
    out.extend(map.map_planes.to_be_bytes());
    for g in &map.gains {
        out.extend(g.to_be_bytes());
    }
    out
}

/// A one-plane map of `points_v` by `points_h` points spread evenly over
/// a whole `width` by `height` image, every sample, from `gain(v, h)`.
#[cfg(test)]
pub(crate) fn even_map(
    width: u32,
    height: u32,
    points_v: u32,
    points_h: u32,
    gain: impl Fn(u32, u32) -> f32,
) -> GainMap {
    let spacing = |n: u32| if n > 1 { 1.0 / f64::from(n - 1) } else { 1.0 };
    GainMap {
        top: 0,
        left: 0,
        bottom: height,
        right: width,
        plane: 0,
        planes: 1,
        row_pitch: 1,
        col_pitch: 1,
        points_v,
        points_h,
        spacing_v: spacing(points_v),
        spacing_h: spacing(points_h),
        origin_v: 0.0,
        origin_h: 0.0,
        map_planes: 1,
        gains: (0..points_v)
            .flat_map(|v| (0..points_h).map(move |h| (v, h)))
            .map(|(v, h)| gain(v, h))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list round-trips through its bytes: the gain map read back as
    /// written, a warp and an unknown opcode passed over, the optional
    /// flag read.
    #[test]
    fn a_list_gives_its_gain_maps_and_passes_over_the_rest() {
        let map = even_map(8, 6, 2, 3, |v, h| 1.0 + (v * 3 + h) as f32);
        let list = list_bytes(&[
            (1, 1, vec![0; 68]),
            (GAIN_MAP, 0, gain_map_bytes(&map)),
            (99, 0, vec![1, 2, 3]),
        ]);
        let ops = parse_list(&list);
        assert_eq!(
            ops.iter().map(|o| (o.id, o.optional())).collect::<Vec<_>>(),
            vec![(1, true), (GAIN_MAP, false), (99, false)]
        );
        assert_eq!(gain_maps(&list), vec![map]);
        assert_eq!(opcode_name(14), "WarpRectilinear2");
    }

    /// Damage is never a panic or an error: a list cut short keeps what
    /// came before the cut, a map whose gains do not match its grid is
    /// passed over, and a count past reason reads nothing.
    #[test]
    fn a_damaged_list_reads_what_it_can() {
        let map = even_map(4, 4, 2, 2, |_, _| 2.0);
        let good = gain_map_bytes(&map);
        let list = list_bytes(&[(GAIN_MAP, 0, good.clone()), (GAIN_MAP, 0, good.clone())]);
        assert_eq!(gain_maps(&list[..list.len() - 3]).len(), 1);
        assert!(parse_list(&[0, 0]).is_empty());
        assert!(parse_list(&u32::MAX.to_be_bytes()).is_empty());

        let short = list_bytes(&[(GAIN_MAP, 0, good[..good.len() - 4].to_vec())]);
        assert!(gain_maps(&short).is_empty());
        let mut no_points = map.clone();
        no_points.points_v = 0;
        no_points.gains.clear();
        assert!(parse_gain_map(&gain_map_bytes(&no_points)).is_none());
        let mut no_pitch = map.clone();
        no_pitch.col_pitch = 0;
        assert!(parse_gain_map(&gain_map_bytes(&no_pitch)).is_none());
        let mut nan = map.clone();
        nan.gains[1] = f32::NAN;
        assert!(parse_gain_map(&gain_map_bytes(&nan)).is_none());

        // A grid whose point count wraps a 64-bit product to 4, with
        // the four gains a count of 4 asks for after it: read unchecked,
        // it passes the length check and indexes past the gains.
        let mut wrapped = map.clone();
        (wrapped.points_v, wrapped.points_h, wrapped.map_planes) = (968_973_220, 49_477, 384_773);
        assert_eq!(968_973_220u64.wrapping_mul(49_477).wrapping_mul(384_773), 4);
        assert!(parse_gain_map(&gain_map_bytes(&wrapped)).is_none());
        let mut huge = map;
        (huge.points_v, huge.points_h, huge.map_planes) = (u32::MAX, u32::MAX, u32::MAX);
        assert!(parse_gain_map(&gain_map_bytes(&huge)).is_none());
    }

    /// An empty area is the whole image, as the SDK reads one.
    #[test]
    fn an_empty_area_is_the_whole_image() {
        let mut map = even_map(4, 4, 2, 2, |_, _| 2.0);
        (map.top, map.bottom) = (3, 3);
        let read = parse_gain_map(&gain_map_bytes(&map)).expect("read");
        assert_eq!(
            (read.top, read.left, read.bottom, read.right),
            (0, 0, u32::MAX, u32::MAX)
        );
        let mut s = vec![0.25f32; 16];
        let at = crate::develop::gain_map::MapPlacement {
            x: 0,
            y: 0,
            image_width: 4,
            image_height: 4,
        };
        crate::develop::gain_map::apply_gain_maps(&mut s, 4, 1, at, &[read]);
        assert!(s.iter().all(|&v| v == 0.5), "{s:?}");
    }
}
