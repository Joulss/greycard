//! A DNG's opcode lists: the corrections its maker leaves to the reader,
//! in the binary layout of the DNG specification (1.7, chapter 7).
//!
//! A list is big-endian whatever the file's byte order: a count, then
//! each opcode as its id, the DNG version it was written for, its flags,
//! the byte count of its parameters, and the parameters. Of the opcodes
//! the gain maps, the two sample maps, the rectilinear warps, and the
//! two bad-pixel lists are read; every other is named in the log and
//! left, as a reader may leave one flagged optional. One that is not
//! optional is left too, with the log saying so: a picture with a
//! correction missing is more use than no picture.
//!
//! Where the specification leaves a reader a choice, the reading
//! follows Adobe's DNG SDK (`dng_lens_correction.cpp` for the warps'
//! parameters, `dng_opcode_list.cpp` for the rule between the two
//! warps); no SDK code is taken. The bad photosites are repaired
//! otherwise than the SDK's `dng_bad_pixels.cpp` does
//! (`develop::bad_pixels` says how).

use std::fmt;

/// The opcode's flag bit saying a reader may skip it.
const OPTIONAL: u32 = 1;

/// The opcodes' ids.
const WARP_RECTILINEAR: u32 = 1;
const WARP_FISHEYE: u32 = 2;
const FIX_BAD_PIXELS_CONSTANT: u32 = 4;
const FIX_BAD_PIXELS_LIST: u32 = 5;
const MAP_TABLE: u32 = 7;
const MAP_POLYNOMIAL: u32 = 8;
const GAIN_MAP: u32 = 9;
const WARP_RECTILINEAR_2: u32 = 14;

/// The radial terms of a warp's polynomial, as `WarpRectilinear2` lays
/// them out: the powers of the radius from 0 to 14.
pub const RADIAL_TERMS: usize = 15;

/// The largest number of opcodes a list is read for; a count past it is
/// a damaged list, not a camera's.
const MAX_OPCODES: u32 = 1024;

/// The most points a gain map is read with, every plane counted: a
/// phone's maps are 1200, and a map past this is a damaged one.
const MAX_MAP_POINTS: u64 = 1 << 22;

/// The highest degree of a `MapPolynomial`: the SDK's limit, and a
/// file past it is damaged.
const MAX_DEGREE: usize = 8;

/// The most entries of a `MapTable`, one a 16-bit value; the SDK's
/// limit too.
const MAX_TABLE: usize = 65536;

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

/// A `WarpRectilinear` or `WarpRectilinear2` opcode: a lens's distortion,
/// and with a set of coefficients a plane its lateral chromatic
/// aberration, as a map from each corrected pixel to where it is found
/// in the picture as taken. It runs on the demosaiced picture of the
/// active area (`OpcodeList3`).
///
/// For a pixel at (x, y), with the center (cx, cy) in pixels and m the
/// distance from the center to the farthest corner: dx = (x - cx) / m,
/// dy = (y - cy) / m, r^2 = dx^2 + dy^2, and the source is
///
/// ```text
/// xs = cx + m (dx f(r) + kt1 (r^2 + 2 dx^2) + 2 kt0 dx dy)
/// ys = cy + m (dy f(r) + kt0 (r^2 + 2 dy^2) + 2 kt1 dx dy)
/// ```
///
/// where f(r) is the radial polynomial (its reciprocal when the file
/// says so), r held to the plane's valid range. `WarpRectilinear`'s four
/// terms are the even powers 0, 2, 4, and 6 of the same polynomial.
#[derive(Debug, Clone, PartialEq)]
pub struct Warp {
    /// One set for every plane, or one each for red, green, and blue.
    pub planes: Vec<WarpPlane>,
    /// The optical center, relative to the image: 0 at its left or top
    /// edge, 1 at its right or bottom.
    pub center_x: f64,
    pub center_y: f64,
    /// Whether the radius is divided by the polynomial rather than
    /// multiplied.
    pub reciprocal: bool,
}

/// One plane's coefficients of a [`Warp`].
#[derive(Debug, Clone, PartialEq)]
pub struct WarpPlane {
    /// The radial polynomial's terms, kr0 to kr14 by power of r.
    pub radial: [f64; RADIAL_TERMS],
    /// The tangential terms kt0 and kt1.
    pub tangential: [f64; 2],
    /// The radii, relative to the farthest corner, the polynomial is
    /// read between; a radius outside is held to the nearer.
    pub valid: (f64, f64),
}

impl WarpPlane {
    /// A plane that leaves every pixel where it is.
    pub fn identity() -> Self {
        let mut radial = [0.0; RADIAL_TERMS];
        radial[0] = 1.0;
        Self {
            radial,
            tangential: [0.0; 2],
            valid: (0.0, 1.0),
        }
    }
}

impl Warp {
    /// Whether the warp leaves every pixel where it is.
    pub fn is_identity(&self) -> bool {
        let identity = WarpPlane::identity();
        self.planes
            .iter()
            .all(|p| p.radial == identity.radial && p.tangential == identity.tangential)
    }

    /// The coefficients of `plane`: its own, or the one set's.
    pub fn plane(&self, plane: usize) -> &WarpPlane {
        &self.planes[plane.min(self.planes.len() - 1)]
    }
}

/// A `FixBadPixelsConstant` or `FixBadPixelsList` opcode: photosites the
/// maker knows to be bad, each to be made again from its same-color
/// neighbors. Mosaic only; `OpcodeList1` or `OpcodeList2`.
#[derive(Debug, Clone, PartialEq)]
pub enum BadPixels {
    /// Every photosite whose value, as the list sees it, is `value`:
    /// the stored value for `OpcodeList1`, the value scaled to 0..65535
    /// over the levels for `OpcodeList2`.
    Constant { value: u32, phase: u32 },
    /// The photosites at `points`, (row, column), and every one inside
    /// `rects`, (top, left, bottom, right) with the bottom and right
    /// outside, in the coordinates of the image the list runs on.
    List {
        phase: u32,
        points: Vec<[i32; 2]>,
        rects: Vec<[i32; 4]>,
    },
}

impl BadPixels {
    /// The mosaic's phase the maker wrote, 0 to 3: which of red, the two
    /// greens, and blue is at the image's top left. Carried for the log;
    /// the develop reads the frame's own pattern, which also has an
    /// answer for an X-Trans mosaic.
    pub fn phase(&self) -> u32 {
        match self {
            BadPixels::Constant { phase, .. } | BadPixels::List { phase, .. } => *phase,
        }
    }
}

/// A `MapTable` or `MapPolynomial` opcode: a curve on every sample of
/// the planes it names, over an area of the image. A maker undoes a
/// nonlinearity with one; Lightroom's merges (a panorama, an HDR) are
/// linear DNGs that carry their tone and color in three, one a plane,
/// and are magenta and clipped without them. Read for `OpcodeList2`,
/// where the curve runs on the linear values, 0 at the black and 1 at
/// the white; the first list's runs on the stored values and is left.
#[derive(Debug, Clone, PartialEq)]
pub struct Remap {
    /// The area, the planes, and the pitches, as a [`GainMap`]'s.
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
    pub plane: u32,
    pub planes: u32,
    pub row_pitch: u32,
    pub col_pitch: u32,
    pub curve: Curve,
}

/// The curve of a [`Remap`].
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    /// The coefficients by power of the sample, the constant term first.
    Polynomial(Vec<f64>),
    /// A table over the 16-bit sample; a sample past its end takes its
    /// last entry.
    Table(Vec<u16>),
}

impl Curve {
    /// The curve at `x`, a sample 0 at the black and 1 at the white,
    /// held to that range as the SDK holds it.
    #[inline]
    pub fn at(&self, x: f32) -> f32 {
        match self {
            Curve::Polynomial(k) => {
                let x = f64::from(x);
                let y = k.iter().rev().fold(0.0, |acc, k| acc * x + k);
                // A NaN pins to 0, as the table's index does.
                if y.is_nan() {
                    0.0
                } else {
                    y.clamp(0.0, 1.0) as f32
                }
            }
            Curve::Table(t) => {
                let i = ((x * 65535.0).round() as usize).min(t.len() - 1);
                f32::from(t[i]) / 65535.0
            }
        }
    }
}

/// Which of a DNG's three opcode lists an opcode came from: the first
/// runs on the stored values, the second on the linear values of the
/// active area, the third after the demosaic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcodeList {
    One,
    Two,
    Three,
}

impl OpcodeList {
    pub fn name(self) -> &'static str {
        match self {
            OpcodeList::One => "OpcodeList1",
            OpcodeList::Two => "OpcodeList2",
            OpcodeList::Three => "OpcodeList3",
        }
    }
}

/// One opcode of the first two lists, run on the mosaic.
#[derive(Debug, Clone, PartialEq)]
pub enum MosaicOp {
    /// `OpcodeList2`'s.
    GainMap(GainMap),
    /// `OpcodeList2`'s.
    Remap(Remap),
    /// The first list's or the second's.
    BadPixels(OpcodeList, BadPixels),
}

/// The most warps a file's correction is read with: a camera writes one
/// or two, and each is a resample of every pixel.
pub const MAX_WARPS: usize = 4;

/// What a DNG's three opcode lists ask of the develop, the rest logged
/// as left.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Corrections {
    /// The first two lists' opcodes, the first's then the second's, each
    /// in its file order: a constant bad-pixel test sees the values the
    /// opcodes before it made.
    pub mosaic: Vec<MosaicOp>,
    /// `OpcodeList3`'s rectilinear warps, a superseded one left out, at
    /// most [`MAX_WARPS`].
    pub warps: Vec<Warp>,
}

impl Corrections {
    /// The gain maps, in order.
    pub fn gain_maps(&self) -> impl Iterator<Item = &GainMap> {
        gain_maps_of(&self.mosaic)
    }
}

/// The gain maps of a mosaic's opcodes, in order.
pub fn gain_maps_of(ops: &[MosaicOp]) -> impl Iterator<Item = &GainMap> {
    ops.iter().filter_map(|op| match op {
        MosaicOp::GainMap(map) => Some(map),
        MosaicOp::Remap(_) | MosaicOp::BadPixels(..) => None,
    })
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
    corrections(None, Some(list), None)
        .gain_maps()
        .cloned()
        .collect()
}

/// What the three lists, those a file has, ask of the develop: each
/// opcode read is kept in the list's order, and every other named in
/// the log as left undone, as is one that will not read.
///
/// A rectilinear warp that follows an applied, optional
/// `WarpRectilinear2` is the older readers' copy of it and is left out,
/// as the SDK leaves it: a phone writes its distortion both ways.
pub fn corrections(
    list1: Option<&[u8]>,
    list2: Option<&[u8]>,
    list3: Option<&[u8]>,
) -> Corrections {
    let mut out = Corrections::default();
    for (which, bytes) in [
        (OpcodeList::One, list1),
        (OpcodeList::Two, list2),
        (OpcodeList::Three, list3),
    ] {
        let Some(bytes) = bytes else { continue };
        let mut after_warp_2 = false;
        for op in parse_list(bytes) {
            let name = which.name();
            let follows_warp_2 = std::mem::take(&mut after_warp_2);
            let read = match (which, op.id) {
                (OpcodeList::Two, GAIN_MAP) => parse_gain_map(op.params).map(|map| {
                    out.mosaic.push(MosaicOp::GainMap(map));
                }),
                (OpcodeList::Two, MAP_TABLE | MAP_POLYNOMIAL) => {
                    parse_remap(op.id, op.params).map(|map| {
                        out.mosaic.push(MosaicOp::Remap(map));
                    })
                }
                (
                    OpcodeList::One | OpcodeList::Two,
                    FIX_BAD_PIXELS_CONSTANT | FIX_BAD_PIXELS_LIST,
                ) => parse_bad_pixels(op.id, op.params).map(|fix| {
                    out.mosaic.push(MosaicOp::BadPixels(which, fix));
                }),
                (OpcodeList::Three, WARP_RECTILINEAR | WARP_FISHEYE) if follows_warp_2 => {
                    log::info!("DNG {name}: {op} left for the WarpRectilinear2 before it");
                    continue;
                }
                (OpcodeList::Three, WARP_RECTILINEAR | WARP_RECTILINEAR_2) => {
                    parse_warp(op.id, op.params).map(|warp| {
                        after_warp_2 = op.id == WARP_RECTILINEAR_2 && op.optional();
                        if warp.is_identity() {
                            log::debug!("DNG {name}: {op} moves nothing");
                        } else if out.warps.len() == MAX_WARPS {
                            log::warn!("DNG {name}: {op} past the {MAX_WARPS}th warp, left");
                        } else {
                            out.warps.push(warp);
                        }
                    })
                }
                _ => {
                    log_left(&op, name);
                    continue;
                }
            };
            if read.is_none() {
                log::warn!("DNG {name}: {op} out of shape, left undone");
            }
        }
    }
    out
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

/// A `MapTable` (`id` 7) or `MapPolynomial` (`id` 8)'s parameters, or
/// `None` when they are out of shape: a zero pitch or plane count, a
/// degree past [`MAX_DEGREE`], a table of no entry or of more than
/// [`MAX_TABLE`], a byte count not the count's, or a coefficient that
/// is not a number. An empty area is read as the whole image.
pub fn parse_remap(id: u32, params: &[u8]) -> Option<Remap> {
    let mut r = Reader {
        bytes: params,
        at: 0,
    };
    let mut map = Remap {
        top: r.u32()?,
        left: r.u32()?,
        bottom: r.u32()?,
        right: r.u32()?,
        plane: r.u32()?,
        planes: r.u32()?,
        row_pitch: r.u32()?,
        col_pitch: r.u32()?,
        curve: Curve::Table(Vec::new()),
    };
    if map.bottom <= map.top || map.right <= map.left {
        (map.top, map.left, map.bottom, map.right) = (0, 0, u32::MAX, u32::MAX);
    }
    if map.planes == 0 || map.row_pitch == 0 || map.col_pitch == 0 {
        return None;
    }
    let count = r.u32()? as usize;
    let rest = r.bytes.len() - r.at;
    map.curve = if id == MAP_POLYNOMIAL {
        if count > MAX_DEGREE || rest != (count + 1) * 8 {
            return None;
        }
        let k = (0..=count).map(|_| r.f64()).collect::<Option<Vec<_>>>()?;
        if k.iter().any(|k| !k.is_finite()) {
            return None;
        }
        Curve::Polynomial(k)
    } else {
        if count == 0 || count > MAX_TABLE || rest != count * 2 {
            return None;
        }
        Curve::Table((0..count).map(|_| r.u16()).collect::<Option<Vec<_>>>()?)
    };
    Some(map)
}

/// A `WarpRectilinear` (`id` 1) or `WarpRectilinear2` (`id` 14)'s
/// parameters, or `None` when they are out of shape: a plane count not
/// one or three, a byte count not the planes' (the SDK's checks), a
/// center outside the image, a valid range not within 0..1 and rising,
/// or a coefficient that is not a number.
pub fn parse_warp(id: u32, params: &[u8]) -> Option<Warp> {
    let mut r = Reader {
        bytes: params,
        at: 0,
    };
    let planes = r.u32()?;
    // A plane each of red, green and blue, or one for all three: the
    // develop's camera RGB has no fourth, which the SDK would refuse.
    if planes != 1 && planes != 3 {
        return None;
    }
    let second = id == WARP_RECTILINEAR_2;
    let per_plane = if second { RADIAL_TERMS + 4 } else { 6 };
    let tail = if second { 20 } else { 16 };
    if params.len() != 4 + planes as usize * per_plane * 8 + tail {
        return None;
    }
    let mut sets = Vec::with_capacity(planes as usize);
    for _ in 0..planes {
        let mut plane = WarpPlane::identity();
        if second {
            for k in &mut plane.radial {
                *k = r.f64()?;
            }
        } else {
            for power in [0, 2, 4, 6] {
                plane.radial[power] = r.f64()?;
            }
        }
        plane.tangential = [r.f64()?, r.f64()?];
        if second {
            plane.valid = (r.f64()?, r.f64()?);
        }
        let (lo, hi) = plane.valid;
        // Written so a NaN fails every test.
        if !(0.0 <= lo && lo < hi && hi <= 1.0)
            || !plane
                .radial
                .iter()
                .chain(&plane.tangential)
                .all(|k| k.is_finite())
        {
            return None;
        }
        sets.push(plane);
    }
    let warp = Warp {
        planes: sets,
        center_x: r.f64()?,
        center_y: r.f64()?,
        reciprocal: second && r.u32()? != 0,
    };
    let inside = |c: f64| (0.0..=1.0).contains(&c);
    (inside(warp.center_x) && inside(warp.center_y)).then_some(warp)
}

/// A `FixBadPixelsConstant` (`id` 4) or `FixBadPixelsList` (`id` 5)'s
/// parameters, or `None` when the byte count is not the one they say.
pub fn parse_bad_pixels(id: u32, params: &[u8]) -> Option<BadPixels> {
    let mut r = Reader {
        bytes: params,
        at: 0,
    };
    if id == FIX_BAD_PIXELS_CONSTANT {
        let fix = BadPixels::Constant {
            value: r.u32()?,
            phase: r.u32()?,
        };
        return (params.len() == 8).then_some(fix);
    }
    let phase = r.u32()?;
    let (points, rects) = (r.u32()?, r.u32()?);
    // Checked: a count that wrapped could match a short list.
    let size = u64::from(points)
        .checked_mul(8)
        .and_then(|p| p.checked_add(u64::from(rects).checked_mul(16)?))
        .and_then(|n| n.checked_add(12))?;
    if size != params.len() as u64 {
        return None;
    }
    let points = (0..points)
        .map(|_| Some([r.i32()?, r.i32()?]))
        .collect::<Option<Vec<_>>>()?;
    let rects = (0..rects)
        .map(|_| Some([r.i32()?, r.i32()?, r.i32()?, r.i32()?]))
        .collect::<Option<Vec<_>>>()?;
    Some(BadPixels::List {
        phase,
        points,
        rects,
    })
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

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
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

/// A warp's parameters as a file lays them out: `WarpRectilinear2`'s
/// when `second`, else `WarpRectilinear`'s, whose radial terms are the
/// even powers to 6.
#[cfg(test)]
pub(crate) fn warp_bytes(warp: &Warp, second: bool) -> Vec<u8> {
    let mut out = (warp.planes.len() as u32).to_be_bytes().to_vec();
    for p in &warp.planes {
        let mut values: Vec<f64> = if second {
            p.radial.to_vec()
        } else {
            [0, 2, 4, 6].iter().map(|&k| p.radial[k]).collect()
        };
        values.extend(p.tangential);
        if second {
            values.extend([p.valid.0, p.valid.1]);
        }
        for v in values {
            out.extend(v.to_be_bytes());
        }
    }
    out.extend(warp.center_x.to_be_bytes());
    out.extend(warp.center_y.to_be_bytes());
    if second {
        out.extend(u32::from(warp.reciprocal).to_be_bytes());
    }
    out
}

/// A `Remap`'s parameters as a file lays them out, for the opcode its
/// curve is.
#[cfg(test)]
pub(crate) fn remap_bytes(map: &Remap) -> Vec<u8> {
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
    ] {
        out.extend(v.to_be_bytes());
    }
    match &map.curve {
        Curve::Polynomial(k) => {
            out.extend((k.len() as u32 - 1).to_be_bytes());
            for k in k {
                out.extend(k.to_be_bytes());
            }
        }
        Curve::Table(t) => {
            out.extend((t.len() as u32).to_be_bytes());
            for t in t {
                out.extend(t.to_be_bytes());
            }
        }
    }
    out
}

/// A `Remap` of `curve` over the whole image, every sample of `planes`
/// planes from `plane`.
#[cfg(test)]
pub(crate) fn whole_remap(plane: u32, planes: u32, curve: Curve) -> Remap {
    Remap {
        top: 0,
        left: 0,
        bottom: u32::MAX,
        right: u32::MAX,
        plane,
        planes,
        row_pitch: 1,
        col_pitch: 1,
        curve,
    }
}

/// A `FixBadPixelsList`'s parameters as a file lays them out.
#[cfg(test)]
pub(crate) fn bad_list_bytes(phase: u32, points: &[[i32; 2]], rects: &[[i32; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [phase, points.len() as u32, rects.len() as u32] {
        out.extend(v.to_be_bytes());
    }
    for v in points.iter().flatten().chain(rects.iter().flatten()) {
        out.extend(v.to_be_bytes());
    }
    out
}

/// A one-plane warp about the center with the radial terms `terms`, by
/// power.
#[cfg(test)]
pub(crate) fn radial_warp(terms: &[(usize, f64)]) -> Warp {
    let mut plane = WarpPlane::identity();
    plane.radial = [0.0; RADIAL_TERMS];
    for &(power, k) in terms {
        plane.radial[power] = k;
    }
    Warp {
        planes: vec![plane],
        center_x: 0.5,
        center_y: 0.5,
        reciprocal: false,
    }
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

    /// Both warps read back as written: `WarpRectilinear`'s four terms
    /// land on the even powers, `WarpRectilinear2`'s fifteen on every
    /// power with their valid range and reciprocal flag, three sets
    /// read as three.
    #[test]
    fn the_warps_read_back_as_written() {
        let first = radial_warp(&[(0, 0.987), (2, 0.0346), (4, -0.0619), (6, 0.0408)]);
        let mut second = radial_warp(&[(0, 0.99), (1, 0.0004), (3, 0.2), (14, -0.2)]);
        second.planes[0].tangential = [1e-4, -3e-4];
        second.planes[0].valid = (0.0, 0.9);
        second.reciprocal = true;
        let mut three = second.clone();
        three.planes = vec![
            second.planes[0].clone(),
            WarpPlane::identity(),
            radial_warp(&[(0, 1.001)]).planes[0].clone(),
        ];
        (three.center_x, three.center_y) = (0.73, 0.69);
        let list = list_bytes(&[
            (WARP_RECTILINEAR, 1, warp_bytes(&first, false)),
            (
                FIX_BAD_PIXELS_CONSTANT,
                1,
                [0u32, 1].iter().flat_map(|v| v.to_be_bytes()).collect(),
            ),
            (WARP_RECTILINEAR_2, 0, warp_bytes(&second, true)),
            (WARP_RECTILINEAR_2, 0, warp_bytes(&three, true)),
        ]);
        let read = corrections(None, None, Some(&list));
        assert_eq!(read.warps, vec![first, second, three]);
        // A bad-pixel list after the demosaic is left, as the SDK
        // refuses one on more than one plane.
        assert!(read.mosaic.is_empty());
    }

    /// The SDK's rule: a `WarpRectilinear` straight after an optional
    /// `WarpRectilinear2` is the older readers' copy and is left out; one
    /// after a required `WarpRectilinear2`, or with another opcode
    /// between, is applied. An identity warp is read and dropped, and
    /// still stands for the rule.
    #[test]
    fn a_warp_after_an_optional_warp_2_is_its_copy() {
        let lens = radial_warp(&[(0, 0.99), (2, 0.02)]);
        let older = radial_warp(&[(0, 0.98), (2, 0.03)]);
        let ca = {
            let mut w = radial_warp(&[(0, 1.0)]);
            w.planes = vec![
                radial_warp(&[(0, 1.001)]).planes[0].clone(),
                WarpPlane::identity(),
                radial_warp(&[(0, 0.999)]).planes[0].clone(),
            ];
            w
        };
        let phone = list_bytes(&[
            (WARP_RECTILINEAR_2, 1, warp_bytes(&ca, true)),
            (WARP_RECTILINEAR_2, 1, warp_bytes(&lens, true)),
            (WARP_RECTILINEAR, 1, warp_bytes(&older, false)),
        ]);
        assert_eq!(
            corrections(None, None, Some(&phone)).warps,
            vec![ca, lens.clone()]
        );

        let required = list_bytes(&[
            (WARP_RECTILINEAR_2, 0, warp_bytes(&lens, true)),
            (WARP_RECTILINEAR, 1, warp_bytes(&older, false)),
        ]);
        assert_eq!(corrections(None, None, Some(&required)).warps.len(), 2);

        let identity = radial_warp(&[(0, 1.0)]);
        let nothing = list_bytes(&[
            (WARP_RECTILINEAR_2, 1, warp_bytes(&identity, true)),
            (WARP_RECTILINEAR, 1, warp_bytes(&older, false)),
            (WARP_RECTILINEAR, 1, warp_bytes(&older, false)),
        ]);
        assert_eq!(corrections(None, None, Some(&nothing)).warps, vec![older]);
    }

    /// The bad-pixel opcodes read from the first two lists, each with its
    /// list, and the second list's in its file order with its gain maps:
    /// a constant before a gain map stays before it.
    #[test]
    fn the_mosaic_opcodes_keep_their_lists_and_order() {
        let listed = bad_list_bytes(1, &[[10, 20], [-1, 4]], &[[0, 0, 2, 3]]);
        let constant: Vec<u8> = [65535u32, 0].iter().flat_map(|v| v.to_be_bytes()).collect();
        let map = even_map(4, 4, 2, 2, |_, _| 1.5);
        let one = list_bytes(&[(FIX_BAD_PIXELS_LIST, 0, listed)]);
        let two = list_bytes(&[
            (FIX_BAD_PIXELS_CONSTANT, 1, constant),
            (GAIN_MAP, 0, gain_map_bytes(&map)),
        ]);
        let read = corrections(Some(&one), Some(&two), None);
        assert_eq!(read.gain_maps().collect::<Vec<_>>(), vec![&map]);
        let list = BadPixels::List {
            phase: 1,
            points: vec![[10, 20], [-1, 4]],
            rects: vec![[0, 0, 2, 3]],
        };
        assert_eq!(list.phase(), 1);
        assert_eq!(
            read.mosaic,
            vec![
                MosaicOp::BadPixels(OpcodeList::One, list),
                MosaicOp::BadPixels(
                    OpcodeList::Two,
                    BadPixels::Constant {
                        value: 65535,
                        phase: 0
                    }
                ),
                MosaicOp::GainMap(map),
            ]
        );
    }

    /// The second list's sample maps read back as written, in their
    /// place among the mosaic's opcodes; the first list's are left, as
    /// they run on the stored values, and the third's as misplaced.
    #[test]
    fn the_sample_maps_read_back_as_written() {
        let poly = whole_remap(0, 3, Curve::Polynomial(vec![0.01, 1.5, -0.4]));
        let table = Remap {
            top: 2,
            left: 1,
            bottom: 6,
            right: 7,
            plane: 1,
            planes: 1,
            row_pitch: 2,
            col_pitch: 1,
            curve: Curve::Table(vec![0, 100, 65535]),
        };
        let one = list_bytes(&[(MAP_POLYNOMIAL, 0, remap_bytes(&poly))]);
        let two = list_bytes(&[
            (MAP_POLYNOMIAL, 0, remap_bytes(&poly)),
            (MAP_TABLE, 1, remap_bytes(&table)),
        ]);
        let read = corrections(Some(&one), Some(&two), Some(&one));
        assert_eq!(
            read.mosaic,
            vec![MosaicOp::Remap(poly), MosaicOp::Remap(table)]
        );
        assert_eq!(read.gain_maps().count(), 0);
        assert!(read.warps.is_empty());
        // An empty area is the whole image.
        let mut empty = whole_remap(0, 1, Curve::Table(vec![7]));
        (empty.top, empty.left, empty.bottom, empty.right) = (0, 0, 0, 0);
        let read = parse_remap(MAP_TABLE, &remap_bytes(&empty)).unwrap();
        assert_eq!((read.top, read.left), (0, 0));
        assert_eq!((read.bottom, read.right), (u32::MAX, u32::MAX));
    }

    /// A sample map out of shape is passed over: a degree past the
    /// SDK's, a table of no entry, a byte count not the count's, a
    /// coefficient that is not a number, a zero pitch.
    #[test]
    fn damaged_sample_maps_are_passed_over() {
        let nine = whole_remap(0, 1, Curve::Polynomial(vec![1.0; 10]));
        assert_eq!(parse_remap(MAP_POLYNOMIAL, &remap_bytes(&nine)), None);
        let none = whole_remap(0, 1, Curve::Table(Vec::new()));
        assert_eq!(parse_remap(MAP_TABLE, &remap_bytes(&none)), None);
        let fine = whole_remap(0, 1, Curve::Polynomial(vec![0.0, 1.0]));
        let mut bytes = remap_bytes(&fine);
        assert!(parse_remap(MAP_POLYNOMIAL, &bytes).is_some());
        bytes.push(0);
        assert_eq!(parse_remap(MAP_POLYNOMIAL, &bytes), None);
        let nan = whole_remap(0, 1, Curve::Polynomial(vec![0.0, f64::NAN]));
        assert_eq!(parse_remap(MAP_POLYNOMIAL, &remap_bytes(&nan)), None);
        let mut still = fine.clone();
        still.col_pitch = 0;
        assert_eq!(parse_remap(MAP_POLYNOMIAL, &remap_bytes(&still)), None);
    }

    /// A curve is read at a sample: the polynomial by its powers, held
    /// to 0..1; the table by the 16-bit value, its last entry past the
    /// end.
    #[test]
    fn a_curve_is_read_at_a_sample() {
        let poly = Curve::Polynomial(vec![0.1, 2.0, -1.0]);
        assert!((poly.at(0.5) - 0.85).abs() < 1e-6);
        assert_eq!(poly.at(1.0), 1.0);
        assert_eq!(Curve::Polynomial(vec![-0.5]).at(0.3), 0.0);
        let table = Curve::Table(vec![0, 32768, 65535]);
        assert_eq!(table.at(0.0), 0.0);
        assert_eq!(table.at(1.0 / 65535.0), 32768.0 / 65535.0);
        assert_eq!(table.at(0.5), 1.0);
        assert_eq!(table.at(1.0), 1.0);
    }

    /// A file's correction is read with at most [`MAX_WARPS`] warps: a
    /// list of a thousand would cost minutes a develop.
    #[test]
    fn the_warps_are_capped() {
        let lens = radial_warp(&[(0, 0.99), (2, 0.02)]);
        let many: Vec<_> = (0..20)
            .map(|_| (WARP_RECTILINEAR, 0, warp_bytes(&lens, false)))
            .collect();
        let read = corrections(None, None, Some(&list_bytes(&many)));
        assert_eq!(read.warps.len(), MAX_WARPS);
    }

    /// Damaged warps and bad-pixel lists are passed over, never a panic:
    /// a plane count the develop has no planes for, a byte count that
    /// does not match, a center off the image, a valid range backward,
    /// a NaN, and point counts whose bytes would wrap.
    #[test]
    fn damaged_warps_and_lists_are_passed_over() {
        let good = radial_warp(&[(0, 0.99), (2, 0.02)]);
        assert!(parse_warp(WARP_RECTILINEAR, &warp_bytes(&good, false)).is_some());
        let mut two = good.clone();
        two.planes.push(WarpPlane::identity());
        assert!(parse_warp(WARP_RECTILINEAR, &warp_bytes(&two, false)).is_none());
        let bytes = warp_bytes(&good, true);
        assert!(parse_warp(WARP_RECTILINEAR_2, &bytes[..bytes.len() - 1]).is_none());
        assert!(parse_warp(WARP_RECTILINEAR, &bytes).is_none());
        let mut off = good.clone();
        off.center_x = 1.5;
        assert!(parse_warp(WARP_RECTILINEAR_2, &warp_bytes(&off, true)).is_none());
        let mut backward = good.clone();
        backward.planes[0].valid = (0.8, 0.2);
        assert!(parse_warp(WARP_RECTILINEAR_2, &warp_bytes(&backward, true)).is_none());
        let mut nan = good.clone();
        nan.planes[0].radial[3] = f64::NAN;
        assert!(parse_warp(WARP_RECTILINEAR_2, &warp_bytes(&nan, true)).is_none());
        let mut nan_range = good;
        nan_range.planes[0].valid = (0.0, f64::NAN);
        assert!(parse_warp(WARP_RECTILINEAR_2, &warp_bytes(&nan_range, true)).is_none());
        assert!(parse_warp(WARP_RECTILINEAR, &[]).is_none());

        let list = bad_list_bytes(0, &[[1, 2]], &[]);
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_LIST, &list).is_some());
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_LIST, &list[..list.len() - 2]).is_none());
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_CONSTANT, &[0; 7]).is_none());
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_CONSTANT, &[0; 12]).is_none());
        // 2^29 points are 2^32 bytes, which a 32-bit sum wraps to 12.
        let mut wrapped = bad_list_bytes(0, &[], &[]);
        wrapped[4..8].copy_from_slice(&(1u32 << 29).to_be_bytes());
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_LIST, &wrapped).is_none());
        let mut huge = wrapped;
        huge[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse_bad_pixels(FIX_BAD_PIXELS_LIST, &huge).is_none());

        // In a list, each is logged and the rest read.
        let lens = radial_warp(&[(0, 0.99), (2, 0.02)]);
        let list = list_bytes(&[
            (WARP_RECTILINEAR_2, 1, vec![0, 0, 0, 2]),
            (WARP_RECTILINEAR, 1, warp_bytes(&lens, false)),
        ]);
        assert_eq!(corrections(None, None, Some(&list)).warps, vec![lens]);
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
