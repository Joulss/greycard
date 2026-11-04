//! A panorama's geometry: where each frame lands on one canvas.
//!
//! A frame's pixels go through three maps on the way to the canvas,
//! composed so the frame is sampled once. The lens's (its distortion
//! and lateral aberration, from the profile, [`FrameLens`]) takes the
//! sensor to the frame as a pinhole camera would have seen it; the
//! frame's turn (a rotation about the lens, [`Model::Turned`], or for
//! a frame whose focal length is not known a homography onto the
//! reference, [`Model::Planar`]) takes that onto the reference's view;
//! and the projection lays the view on the canvas: a plane for a field
//! up to about ninety degrees, a cylinder past it.
//!
//! The turns come from the frames' pairs ([`Link`]): each pair's
//! points, from its features and its homography fit, say where one
//! frame's pixel is in the other. [`adjust_turned`] and
//! [`adjust_planar`] fit every frame's turn to every pair at once
//! (Levenberg-Marquardt, the reference held), so what a chain of
//! neighbors gets wrong is spread over the chain, and a pair that
//! closes a loop or skips a frame pulls the chain onto itself. The
//! focal length is fitted with the turns: the lens's model holds the
//! radius at half the shorter side, not at the center, so the
//! corrected frame's focal length in pixels is not the EXIF's.
//!
//! Written from Szeliski, "Image Alignment and Stitching: A Tutorial"
//! (Foundations and Trends in Computer Graphics and Vision 2(1), 2006),
//! its sections 2 and 6, for the motion models, the projections, and
//! the global adjustment; and Brown and Lowe, "Automatic Panoramic
//! Image Stitching using Invariant Features" (IJCV 74(1), 2007), for
//! the rotations about one center and the leveling. Nothing ported.
//!
//! Coordinates are continuous, a pixel's center at its index plus a
//! half.

use rayon::prelude::*;

use crate::develop::lens::LensCorrection;
use crate::register::Homography;

/// A 3x3 matrix, row-major.
pub type Mat3 = [f64; 9];

pub const IDENTITY: Mat3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut m = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            m[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
        }
    }
    m
}

pub fn transpose(a: &Mat3) -> Mat3 {
    [a[0], a[3], a[6], a[1], a[4], a[7], a[2], a[5], a[8]]
}

#[inline]
pub fn times(a: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [
        a[0] * v[0] + a[1] * v[1] + a[2] * v[2],
        a[3] * v[0] + a[4] * v[1] + a[5] * v[2],
        a[6] * v[0] + a[7] * v[1] + a[8] * v[2],
    ]
}

/// The rotation of `w`'s length about `w`, radians (Rodrigues).
pub fn rotation(w: [f64; 3]) -> Mat3 {
    let t = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    if t < 1e-15 {
        return [
            1.0, -w[2], w[1], //
            w[2], 1.0, -w[0], //
            -w[1], w[0], 1.0,
        ];
    }
    let k = [w[0] / t, w[1] / t, w[2] / t];
    let (s, c) = t.sin_cos();
    let v = 1.0 - c;
    [
        c + k[0] * k[0] * v,
        k[0] * k[1] * v - k[2] * s,
        k[0] * k[2] * v + k[1] * s,
        k[1] * k[0] * v + k[2] * s,
        c + k[1] * k[1] * v,
        k[1] * k[2] * v - k[0] * s,
        k[2] * k[0] * v - k[1] * s,
        k[2] * k[1] * v + k[0] * s,
        c + k[2] * k[2] * v,
    ]
}

/// The nearest rotation to `m` (its polar factor), by averaging it
/// with its inverse transpose until it holds still.
pub fn orthonormal(m: &Mat3) -> Mat3 {
    let mut r = *m;
    let det = det3(&r);
    if det.abs() < 1e-12 || !det.is_finite() {
        return IDENTITY;
    }
    let s = det.abs().cbrt() * det.signum();
    r.iter_mut().for_each(|v| *v /= s);
    for _ in 0..30 {
        let Some(inv) = inverse3(&r) else {
            return IDENTITY;
        };
        let it = transpose(&inv);
        let next: Mat3 = std::array::from_fn(|i| 0.5 * (r[i] + it[i]));
        let moved: f64 = next.iter().zip(&r).map(|(a, b)| (a - b).abs()).sum();
        r = next;
        if moved < 1e-14 {
            break;
        }
    }
    r
}

pub fn det3(m: &Mat3) -> f64 {
    let [a, b, c, d, e, f, g, h, i] = *m;
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

pub fn inverse3(m: &Mat3) -> Option<Mat3> {
    let det = det3(m);
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let [a, b, c, d, e, f, g, h, i] = *m;
    let adj = [
        e * i - f * h,
        c * h - b * i,
        b * f - c * e,
        f * g - d * i,
        a * i - c * g,
        c * d - a * f,
        d * h - e * g,
        b * g - a * h,
        a * e - b * d,
    ];
    Some(adj.map(|v| v / det))
}

/// The unit quaternion `[w, x, y, z]` of rotation `r`, `w` never
/// negative (Shepperd's method: the largest of the four found first,
/// the rest from it, so no square root of a near-zero is taken).
pub fn quaternion(r: &Mat3) -> [f64; 4] {
    let t = r[0] + r[4] + r[8];
    let q = if t > 0.0 {
        let s = 2.0 * (t + 1.0).sqrt();
        [
            0.25 * s,
            (r[7] - r[5]) / s,
            (r[2] - r[6]) / s,
            (r[3] - r[1]) / s,
        ]
    } else if r[0] > r[4] && r[0] > r[8] {
        let s = 2.0 * (1.0 + r[0] - r[4] - r[8]).sqrt();
        [
            (r[7] - r[5]) / s,
            0.25 * s,
            (r[1] + r[3]) / s,
            (r[2] + r[6]) / s,
        ]
    } else if r[4] > r[8] {
        let s = 2.0 * (1.0 + r[4] - r[0] - r[8]).sqrt();
        [
            (r[2] - r[6]) / s,
            (r[1] + r[3]) / s,
            0.25 * s,
            (r[5] + r[7]) / s,
        ]
    } else {
        let s = 2.0 * (1.0 + r[8] - r[0] - r[4]).sqrt();
        [
            (r[3] - r[1]) / s,
            (r[2] + r[6]) / s,
            (r[5] + r[7]) / s,
            0.25 * s,
        ]
    };
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    let sign = if q[0] < 0.0 { -1.0 } else { 1.0 };
    q.map(|v| sign * v / n)
}

/// The rotation of quaternion `q` `[w, x, y, z]`, made unit first.
pub fn of_quaternion(q: [f64; 4]) -> Mat3 {
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    let [w, x, y, z] = q.map(|v| v / n);
    [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - z * w),
        2.0 * (x * z + y * w),
        2.0 * (x * y + z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - x * w),
        2.0 * (x * z - y * w),
        2.0 * (y * z + x * w),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

/// A frame's lens: the corrections a develop would make of it, those
/// that move a pixel, applied in the order the develop applies them.
#[derive(Debug, Clone)]
pub struct FrameLens {
    pub corrections: Vec<LensCorrection>,
    pub width: usize,
    pub height: usize,
}

impl FrameLens {
    /// A lens that moves nothing.
    pub fn none(width: usize, height: usize) -> Self {
        Self {
            corrections: Vec::new(),
            width,
            height,
        }
    }

    /// The corrections' moving parts: the distortion and the lateral
    /// aberration; the vignetting is the stage list's, applied to the
    /// samples before the frame comes here.
    pub fn of(corrections: &[LensCorrection], width: usize, height: usize) -> Self {
        Self {
            corrections: corrections
                .iter()
                .filter(|c| c.moves())
                .map(|c| LensCorrection {
                    vignetting: None,
                    ..*c
                })
                .collect(),
            width,
            height,
        }
    }

    pub fn moves(&self) -> bool {
        !self.corrections.is_empty()
    }

    /// Where on the sensor a corrected point's light for `channel` is:
    /// the green's in f64, so its inverse settles at a full frame's
    /// coordinates, the red's and the blue's through the aberration.
    #[inline]
    pub fn source(&self, channel: usize, x: f64, y: f64) -> (f64, f64) {
        if channel == 1 {
            let (w, h) = (self.width as f64, self.height as f64);
            let (mut u, mut v) = (x, y);
            for c in self.corrections.iter().rev() {
                (u, v) = c.green_source_of_f64(u, v, w, h, 1.0);
            }
            return (u, v);
        }
        let (w, h) = (self.width as f32, self.height as f32);
        let (mut u, mut v) = (x as f32, y as f32);
        for c in self.corrections.iter().rev() {
            (u, v) = c.source_of(channel, u, v, w, h, 1.0);
        }
        (f64::from(u), f64::from(v))
    }

    /// The corrected point whose green light is at sensor point
    /// `(sx, sy)`, by Newton's method; none where it does not settle,
    /// or where it settles past where the distortion's polynomial
    /// folds back (the map turned over there, or the point on the far
    /// side of the middle), which a sensor point a strong profile
    /// cannot reach would otherwise find.
    pub fn corrected(&self, sx: f64, sy: f64) -> Option<(f64, f64)> {
        if !self.moves() {
            return Some((sx, sy));
        }
        let (mx, my) = (self.width as f64 / 2.0, self.height as f64 / 2.0);
        let (mut x, mut y) = (sx, sy);
        for _ in 0..40 {
            let (u, v) = self.source(1, x, y);
            let (ex, ey) = (u - sx, v - sy);
            let d = 0.5;
            let (ux, vx) = self.source(1, x + d, y);
            let (uy, vy) = self.source(1, x, y + d);
            let (a, b, c, e) = ((ux - u) / d, (uy - u) / d, (vx - v) / d, (vy - v) / d);
            let det = a * e - b * c;
            if ex.hypot(ey) < 1e-4 {
                let same_side = (x - mx) * (sx - mx) + (y - my) * (sy - my) >= 0.0;
                return (det > 0.0 && same_side).then_some((x, y));
            }
            if det.abs() < 1e-9 {
                return None;
            }
            x -= (e * ex - b * ey) / det;
            y -= (-c * ex + a * ey) / det;
            if !(x.is_finite() && y.is_finite()) {
                return None;
            }
        }
        None
    }

    /// Points along the sensor's edge, `n` a side, in corrected
    /// coordinates: the frame's outline as the canvas sees it.
    pub fn outline(&self, n: usize) -> Vec<(f64, f64)> {
        let (w, h) = (self.width as f64, self.height as f64);
        let mut out = Vec::with_capacity(4 * n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            for p in [(t * w, 0.0), (w, t * h), (w - t * w, h), (0.0, h - t * h)] {
                if let Some(q) = self.corrected(p.0, p.1) {
                    out.push(q);
                }
            }
        }
        out
    }
}

/// Which way a cylinder stands, in the reference's sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Along the sensor's height: frames turned left and right.
    Down,
    /// Along its width: frames turned up and down (a camera held on
    /// its side for a level panorama has its sensor's width upright).
    Across,
}

/// How the view is laid on the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    Planar,
    Cylinder(Axis),
}

/// Where every frame of a panorama is, onto the reference's view.
#[derive(Debug, Clone, PartialEq)]
pub enum Model {
    /// Each frame turned about the lens: its rotation from its own
    /// camera's rays to the reference's, the focal length in pixels of
    /// the corrected frame, and the projection.
    Turned {
        focal: f64,
        rotations: Vec<Mat3>,
        projection: Projection,
    },
    /// Each frame's corrected pixels onto the reference's: for a
    /// panorama of frames whose focal length is not known, planar.
    Planar { homographies: Vec<Homography> },
}

impl Model {
    /// The frame nearest the middle of the turn: the one whose rotation
    /// is nearest the mean of them all (the largest trace against
    /// their sum, which is the least distance to their mean), or for a
    /// planar model the one whose middle lands nearest the mean of the
    /// middles. Of a set shot right to left, or with a frame shot
    /// again at its end, the middle of the list is not the middle of
    /// the picture; this is.
    pub fn middle(&self, width: usize, height: usize) -> usize {
        // Scores within a rounding of the least are a tie, and the
        // first frame of a tie wins: two frames score the same but
        // for the last bit, which differs between machines.
        let nearest = |score: &dyn Fn(usize) -> f64, n: usize| {
            let scores: Vec<f64> = (0..n).map(score).collect();
            let least = scores.iter().copied().fold(f64::INFINITY, f64::min);
            let near = 1e-9 * least.abs().max(1.0);
            scores.iter().position(|&s| s <= least + near).unwrap_or(0)
        };
        match self {
            Model::Turned { rotations, .. } => {
                let mut sum = [0.0; 9];
                for r in rotations {
                    for (s, v) in sum.iter_mut().zip(r) {
                        *s += v;
                    }
                }
                let trace =
                    |k: usize| -> f64 { -(0..9).map(|i| sum[i] * rotations[k][i]).sum::<f64>() };
                nearest(&trace, rotations.len())
            }
            Model::Planar { homographies } => {
                let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
                let middles: Vec<(f64, f64)> =
                    homographies.iter().map(|h| h.apply(cx, cy)).collect();
                let n = middles.len().max(1) as f64;
                let mx = middles.iter().map(|p| p.0).sum::<f64>() / n;
                let my = middles.iter().map(|p| p.1).sum::<f64>() / n;
                let far = |k: usize| (middles[k].0 - mx).hypot(middles[k].1 - my);
                nearest(&far, middles.len())
            }
        }
    }

    /// The model turned onto frame `reference`'s view: each frame's
    /// rotation (or homography) composed with the inverse of that
    /// frame's, so it is the identity. The pairs' fit is unchanged by
    /// it: one turn of the whole.
    pub fn onto(self, reference: usize) -> Model {
        match self {
            Model::Turned {
                focal,
                rotations,
                projection,
            } => {
                let back = transpose(&rotations[reference]);
                let rotations = rotations
                    .iter()
                    .enumerate()
                    .map(|(k, r)| {
                        if k == reference {
                            IDENTITY
                        } else {
                            mul(&back, r)
                        }
                    })
                    .collect();
                Model::Turned {
                    focal,
                    rotations,
                    projection,
                }
            }
            Model::Planar { homographies } => {
                let Some(back) = homographies[reference].inverse() else {
                    return Model::Planar { homographies };
                };
                let homographies = homographies
                    .iter()
                    .enumerate()
                    .map(|(k, h)| {
                        if k == reference {
                            Homography::IDENTITY
                        } else {
                            back.compose(h)
                        }
                    })
                    .collect();
                Model::Planar { homographies }
            }
        }
    }
}

/// A panorama's frames as they lie: the model, the frames' size, and
/// the canvas's offset from the projection's origin.
#[derive(Debug, Clone)]
pub struct Placement {
    pub model: Model,
    pub width: usize,
    pub height: usize,
}

impl Placement {
    fn center(&self) -> (f64, f64) {
        (self.width as f64 / 2.0, self.height as f64 / 2.0)
    }

    /// The canvas point of frame `k`'s corrected point `(x, y)`; none
    /// where it does not land (behind the camera).
    pub fn to_canvas(&self, k: usize, x: f64, y: f64) -> Option<(f64, f64)> {
        let (cx, cy) = self.center();
        match &self.model {
            Model::Planar { homographies } => {
                let h = &homographies[k];
                (h.depth(x, y) > 0.0).then(|| h.apply(x, y))
            }
            Model::Turned {
                focal,
                rotations,
                projection,
            } => {
                let f = *focal;
                let d = times(&rotations[k], [(x - cx) / f, (y - cy) / f, 1.0]);
                project(d, f, (cx, cy), *projection)
            }
        }
    }

    /// Frame `k`'s corrected point for canvas point `(x, y)`; none
    /// where the canvas point is behind the frame.
    #[inline]
    pub fn to_frame(&self, k: usize, x: f64, y: f64) -> Option<(f64, f64)> {
        let (cx, cy) = self.center();
        match &self.model {
            Model::Planar { homographies } => {
                // Kept beside the forward maps would be quicker; a
                // planar panorama is the rare one.
                let back = homographies[k].inverse()?;
                (back.depth(x, y) > 0.0).then(|| back.apply(x, y))
            }
            Model::Turned {
                focal,
                rotations,
                projection,
            } => {
                let f = *focal;
                let d = unproject(x, y, f, (cx, cy), *projection)?;
                let r = times(&transpose(&rotations[k]), d);
                (r[2] > 1e-9).then(|| (f * r[0] / r[2] + cx, f * r[1] / r[2] + cy))
            }
        }
    }

    /// The angles the frames span, horizontally and vertically in the
    /// reference's sensor, degrees; none for a planar model, which
    /// knows no angles.
    pub fn span(&self, lens: &FrameLens) -> Option<(f64, f64)> {
        let Model::Turned {
            focal, rotations, ..
        } = &self.model
        else {
            return None;
        };
        let (cx, cy) = self.center();
        let outline = lens.outline(32);
        let mut theta = (f64::MAX, f64::MIN);
        let mut phi = (f64::MAX, f64::MIN);
        for r in rotations {
            let mid = times(r, [0.0, 0.0, 1.0]);
            let t0 = mid[0].atan2(mid[2]);
            for &(x, y) in &outline {
                let d = times(r, [(x - cx) / focal, (y - cy) / focal, 1.0]);
                // Unwrapped about the frame's own middle.
                let t = t0 + wrap(d[0].atan2(d[2]) - t0);
                // The elevation, not the turn about x: a ray far to the side
                // is no higher for it.
                let p = d[1].atan2(d[0].hypot(d[2]));
                theta = (theta.0.min(t), theta.1.max(t));
                phi = (phi.0.min(p), phi.1.max(p));
            }
        }
        Some((
            (theta.1 - theta.0).to_degrees(),
            (phi.1 - phi.0).to_degrees(),
        ))
    }
}

/// An angle onto -pi to pi.
fn wrap(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    a - t * ((a + std::f64::consts::PI) / t).floor()
}

/// The canvas point of the ray `d` (in the reference's camera).
#[inline]
fn project(d: [f64; 3], f: f64, (cx, cy): (f64, f64), p: Projection) -> Option<(f64, f64)> {
    match p {
        Projection::Planar => (d[2] > 1e-9).then(|| (f * d[0] / d[2] + cx, f * d[1] / d[2] + cy)),
        Projection::Cylinder(Axis::Down) => {
            let r = d[0].hypot(d[2]);
            (r > 1e-12).then(|| (f * d[0].atan2(d[2]) + cx, f * d[1] / r + cy))
        }
        Projection::Cylinder(Axis::Across) => {
            let r = d[1].hypot(d[2]);
            (r > 1e-12).then(|| (f * d[0] / r + cx, f * d[1].atan2(d[2]) + cy))
        }
    }
}

/// The ray of canvas point `(x, y)`.
#[inline]
fn unproject(x: f64, y: f64, f: f64, (cx, cy): (f64, f64), p: Projection) -> Option<[f64; 3]> {
    let (u, v) = ((x - cx) / f, (y - cy) / f);
    match p {
        Projection::Planar => Some([u, v, 1.0]),
        Projection::Cylinder(Axis::Down) => {
            if u.abs() > std::f64::consts::PI {
                return None;
            }
            let (s, c) = u.sin_cos();
            Some([s, v, c])
        }
        Projection::Cylinder(Axis::Across) => {
            if v.abs() > std::f64::consts::PI {
                return None;
            }
            let (s, c) = v.sin_cos();
            Some([u, s, c])
        }
    }
}

/// The widest field a plane is laid on, degrees either way; past it a
/// cylinder.
pub const PLANAR_FIELD: f64 = 90.0;
/// The most a cylinder holds across its axis, degrees: past it the
/// canvas stretches the top and bottom past use.
pub const CYLINDER_ACROSS: f64 = 120.0;
/// And about its axis.
pub const CYLINDER_AROUND: f64 = 360.0;

/// The projection a field of `horizontal` by `vertical` degrees wants:
/// a plane up to [`PLANAR_FIELD`] both ways, else a cylinder whose
/// axis stands across the wider span. An error says the limit the
/// field is past.
pub fn projection_for(horizontal: f64, vertical: f64) -> Result<Projection, String> {
    if horizontal <= PLANAR_FIELD && vertical <= PLANAR_FIELD {
        return Ok(Projection::Planar);
    }
    let (around, across, axis) = if horizontal >= vertical {
        (horizontal, vertical, Axis::Down)
    } else {
        (vertical, horizontal, Axis::Across)
    };
    if around >= CYLINDER_AROUND {
        return Err(format!(
            "the frames go {around:.0} degrees around: a cylinder holds up to {CYLINDER_AROUND:.0}"
        ));
    }
    if across > CYLINDER_ACROSS {
        return Err(format!(
            "the frames span {across:.0} degrees across the turn: a cylinder holds up to \
             {CYLINDER_ACROSS:.0}"
        ));
    }
    Ok(Projection::Cylinder(axis))
}

/// Points two frames share: each `(a's, b's)`, corrected coordinates.
#[derive(Debug, Clone)]
pub struct Link {
    pub a: usize,
    pub b: usize,
    /// How strongly the pair is held: its features' inliers.
    pub strength: usize,
    pub points: Vec<((f64, f64), (f64, f64))>,
    /// Frame `a`'s corrected coordinates to `b`'s, as the pair alone
    /// says them: where a chain starts from.
    pub homography: Homography,
}

/// A grid of `n` by `n` points over frame `a`'s corrected frame that
/// `h` puts inside frame `b`, each with where it lands: the points a
/// pair's homography stands for in the adjustment.
pub fn grid_points(
    h: &Homography,
    width: usize,
    height: usize,
    n: usize,
) -> Vec<((f64, f64), (f64, f64))> {
    let (w, hh) = (width as f64, height as f64);
    let mut out = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let p = (
                (i as f64 + 0.5) / n as f64 * w,
                (j as f64 + 0.5) / n as f64 * hh,
            );
            if h.depth(p.0, p.1) <= 0.0 {
                continue;
            }
            let q = h.apply(p.0, p.1);
            if q.0 >= 0.0 && q.1 >= 0.0 && q.0 <= w && q.1 <= hh {
                out.push((p, q));
            }
        }
    }
    out
}

/// The frames reached from `reference` through `links`, strongest
/// first (a spanning tree by strength), each with the link it was
/// reached through and from which frame: `None` for the reference
/// and for a frame no link reaches.
fn tree(links: &[Link], n: usize, reference: usize) -> Vec<Option<(usize, usize)>> {
    let mut reached = vec![false; n];
    let mut from: Vec<Option<(usize, usize)>> = vec![None; n];
    reached[reference] = true;
    loop {
        let next = links
            .iter()
            .enumerate()
            .filter(|(_, l)| reached[l.a] != reached[l.b])
            .max_by_key(|(i, l)| (l.strength, std::cmp::Reverse(*i)));
        let Some((i, l)) = next else {
            break;
        };
        let (known, new) = if reached[l.a] { (l.a, l.b) } else { (l.b, l.a) };
        reached[new] = true;
        from[new] = Some((i, known));
    }
    from
}

/// Whether every frame is joined to the reference by some chain of
/// links: the frame that is not, if any.
pub fn unjoined(links: &[Link], n: usize, reference: usize) -> Option<usize> {
    let from = tree(links, n, reference);
    (0..n).find(|&k| k != reference && from[k].is_none())
}

/// The rotation a homography between two corrected frames stands for,
/// at focal length `f`: `K^-1 H K` made a rotation. `h` takes frame
/// `a`'s points to `b`'s; the rotation takes `b`'s rays to `a`'s.
pub fn rotation_of(h: &Homography, f: f64, (cx, cy): (f64, f64)) -> Mat3 {
    let k = [f, 0.0, cx, 0.0, f, cy, 0.0, 0.0, 1.0];
    let ki = [1.0 / f, 0.0, -cx / f, 0.0, 1.0 / f, -cy / f, 0.0, 0.0, 1.0];
    // b = K R_ba K^-1 a, so R_ba = K^-1 H K, and a's rays from b's are
    // its transpose.
    let r = orthonormal(&mul(&ki, &mul(&h.m, &k)));
    transpose(&r)
}

/// What an adjustment came to: each link's root-mean-square distance
/// between its points, in pixels, after it.
#[derive(Debug, Clone)]
pub struct Adjusted {
    pub model: Model,
    pub residuals: Vec<f64>,
}

/// Every frame's turn, and the one focal length when `fit_focal`,
/// fitted to every link at once, the reference held, from `focal` and
/// the turns a spanning tree of the links' own homographies gives.
/// Each link counts alike, however many of its points land. The
/// projection is planar until [`projection_for`] says otherwise.
pub fn adjust_turned(
    links: &[Link],
    n: usize,
    reference: usize,
    focal: f64,
    fit_focal: bool,
    (width, height): (usize, usize),
) -> Adjusted {
    let c = (width as f64 / 2.0, height as f64 / 2.0);
    // The chain first: each frame's rotation from the one it hangs on.
    let from = tree(links, n, reference);
    let mut start = vec![IDENTITY; n];
    let mut done = vec![false; n];
    done[reference] = true;
    while done.iter().any(|d| !d) {
        let mut moved = false;
        for k in 0..n {
            if done[k] {
                continue;
            }
            let Some((li, known)) = from[k] else {
                done[k] = true;
                continue;
            };
            if !done[known] {
                continue;
            }
            let l = &links[li];
            // `rotation_of` takes b's rays to a's.
            let r = rotation_of(&l.homography, focal, c);
            start[k] = if l.a == known {
                mul(&start[known], &r)
            } else {
                mul(&start[known], &transpose(&r))
            };
            done[k] = true;
            moved = true;
        }
        if !moved {
            break;
        }
    }
    let free: Vec<usize> = (0..n).filter(|&k| k != reference).collect();
    // The turns' parameters, then the focal length's when it is fitted.
    let unpack = |p: &[f64]| -> (f64, Vec<Mat3>) {
        let f = if fit_focal {
            focal * p[3 * free.len()].exp()
        } else {
            focal
        };
        let mut rs = start.clone();
        for (i, &k) in free.iter().enumerate() {
            let w = [p[3 * i], p[3 * i + 1], p[3 * i + 2]];
            rs[k] = mul(&rotation(w), &start[k]);
        }
        (f, rs)
    };
    let residuals_by = |p: &[f64], weighted: bool| -> Vec<f64> {
        let (f, rs) = unpack(p);
        let mut out = Vec::new();
        for l in links {
            let back = transpose(&rs[l.a]);
            let w = if weighted {
                1.0 / (l.points.len().max(1) as f64).sqrt()
            } else {
                1.0
            };
            for &(pa, pb) in &l.points {
                let d = times(&rs[l.b], [(pb.0 - c.0) / f, (pb.1 - c.1) / f, 1.0]);
                let r = times(&back, d);
                if r[2] <= 1e-9 {
                    out.push(1e3);
                    out.push(1e3);
                    continue;
                }
                out.push(w * (f * r[0] / r[2] + c.0 - pa.0));
                out.push(w * (f * r[1] / r[2] + c.1 - pa.1));
            }
        }
        out
    };
    let residuals = |p: &[f64]| residuals_by(p, true);
    let count = 3 * free.len() + usize::from(fit_focal);
    let p = levenberg_marquardt(vec![0.0; count], &residuals, 60);
    let (f, rotations) = unpack(&p);
    let r = residuals_by(&p, false);
    Adjusted {
        model: Model::Turned {
            focal: f,
            rotations,
            projection: Projection::Planar,
        },
        residuals: per_link(links, &r),
    }
}

/// Every frame's homography onto the reference, fitted to every link
/// at once, the reference held: for frames whose focal length is not
/// known.
pub fn adjust_planar(
    links: &[Link],
    n: usize,
    reference: usize,
    width: usize,
    height: usize,
) -> Adjusted {
    let from = tree(links, n, reference);
    let mut start = vec![Homography::IDENTITY; n];
    let mut done = vec![false; n];
    done[reference] = true;
    while done.iter().any(|d| !d) {
        let mut moved = false;
        for k in 0..n {
            if done[k] {
                continue;
            }
            let Some((li, known)) = from[k] else {
                done[k] = true;
                continue;
            };
            if !done[known] {
                continue;
            }
            let l = &links[li];
            // `l.homography` takes a's points to b's; a frame's
            // homography takes its points to the reference's.
            start[k] = if l.a == known {
                start[known].compose(&l.homography.inverse().unwrap_or_default())
            } else {
                start[known].compose(&l.homography)
            };
            done[k] = true;
            moved = true;
        }
        if !moved {
            break;
        }
    }
    // The parameters are a perturbation in coordinates centered and
    // scaled to about one, so every one moves the frame about alike.
    let s = 2.0 / (width + height) as f64;
    let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
    let norm = Homography {
        m: [s, 0.0, -s * cx, 0.0, s, -s * cy, 0.0, 0.0, 1.0],
    };
    let denorm = norm.inverse().expect("a scaling");
    let free: Vec<usize> = (0..n).filter(|&k| k != reference).collect();
    let unpack = |p: &[f64]| -> Vec<Homography> {
        let mut hs = start.clone();
        for (i, &k) in free.iter().enumerate() {
            let d = &p[8 * i..8 * i + 8];
            let step = Homography {
                m: [
                    1.0 + d[0],
                    d[1],
                    d[2],
                    d[3],
                    1.0 + d[4],
                    d[5],
                    d[6],
                    d[7],
                    1.0,
                ],
            };
            hs[k] = denorm.compose(&step).compose(&norm).compose(&start[k]);
        }
        hs
    };
    let residuals = |p: &[f64]| -> Vec<f64> {
        let hs = unpack(p);
        let mut out = Vec::new();
        for l in links {
            let back = hs[l.a].inverse().unwrap_or_default();
            let m = back.compose(&hs[l.b]);
            for &(pa, pb) in &l.points {
                let q = m.apply(pb.0, pb.1);
                out.push(q.0 - pa.0);
                out.push(q.1 - pa.1);
            }
        }
        out
    };
    let p = levenberg_marquardt(vec![0.0; 8 * free.len()], &residuals, 60);
    let r = residuals(&p);
    Adjusted {
        model: Model::Planar {
            homographies: unpack(&p),
        },
        residuals: per_link(links, &r),
    }
}

/// Each link's root-mean-square distance from its share of the
/// residuals, two a point.
fn per_link(links: &[Link], r: &[f64]) -> Vec<f64> {
    let mut at = 0;
    links
        .iter()
        .map(|l| {
            let k = 2 * l.points.len();
            let s: f64 = r[at..at + k].iter().map(|v| v * v).sum();
            at += k;
            (s / l.points.len().max(1) as f64).sqrt()
        })
        .collect()
}

/// Levenberg-Marquardt on `residuals` from `start`, a numerical
/// Jacobian, for at most `iterations` steps.
// Matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
fn levenberg_marquardt(
    start: Vec<f64>,
    residuals: &(dyn Fn(&[f64]) -> Vec<f64> + Sync),
    iterations: usize,
) -> Vec<f64> {
    let n = start.len();
    if n == 0 {
        return start;
    }
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    let mut p = start;
    let mut r = residuals(&p);
    let mut c = cost(&r);
    let mut lambda = 1e-3;
    for _ in 0..iterations {
        let step = 1e-6;
        let columns: Vec<Vec<f64>> = (0..n)
            .into_par_iter()
            .map(|j| {
                let mut q = p.clone();
                q[j] += step;
                residuals(&q)
                    .iter()
                    .zip(&r)
                    .map(|(a, b)| (a - b) / step)
                    .collect()
            })
            .collect();
        let mut jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for i in 0..n {
            for k in i..n {
                let s: f64 = columns[i].iter().zip(&columns[k]).map(|(a, b)| a * b).sum();
                jtj[i][k] = s;
                jtj[k][i] = s;
            }
            jtr[i] = columns[i].iter().zip(&r).map(|(a, b)| a * b).sum();
        }
        let mut improved = false;
        for _ in 0..10 {
            let mut a = jtj.clone();
            for i in 0..n {
                a[i][i] += lambda * jtj[i][i].max(1e-9);
            }
            let Some(dp) = solve_dense(a, jtr.iter().map(|v| -v).collect()) else {
                lambda *= 10.0;
                continue;
            };
            let q: Vec<f64> = p.iter().zip(&dp).map(|(a, b)| a + b).collect();
            let rq = residuals(&q);
            let cq = cost(&rq);
            if cq < c {
                let gain = (c - cq) / c.max(1e-300);
                p = q;
                r = rq;
                c = cq;
                lambda = (lambda / 10.0).max(1e-12);
                improved = gain > 1e-12;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    p
}

/// `a x = b` for a small dense system, by Gaussian elimination.
#[allow(clippy::needless_range_loop)]
fn solve_dense(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|i, j| a[*i][c].abs().total_cmp(&a[*j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f == 0.0 {
                continue;
            }
            for k in c..n {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// The eigenvector of the symmetric `m` with the smallest eigenvalue,
/// and the two smallest eigenvalues, by Jacobi's rotations.
// Matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
fn smallest_eigen(m: &Mat3) -> ([f64; 3], f64, f64) {
    let mut a = [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]];
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-15 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for k in 0..3 {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[k][q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let (apk, aqk) = (a[p][k], a[q][k]);
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            for k in 0..3 {
                let (vkp, vkq) = (v[k][p], v[k][q]);
                v[k][p] = c * vkp - s * vkq;
                v[k][q] = s * vkp + c * vkq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]));
    let i = order[0];
    ([v[0][i], v[1][i], v[2][i]], a[i][i], a[order[1]][order[1]])
}

/// The rotations turned so the panorama stands level: the frames'
/// axes across the turn (each camera's x for a cylinder down the
/// frames, its y for one across them) lie in one plane when the camera
/// turned about one axis, and that plane's normal is made the
/// cylinder's axis. Without it a camera tipped down as it turned lays
/// its horizon on the canvas as a wave. Frames turned too little to
/// say which plane are left as they are.
pub fn leveled(rotations: &[Mat3], axis: Axis) -> Vec<Mat3> {
    let (across, along) = match axis {
        Axis::Down => (0, 1),
        Axis::Across => (1, 0),
    };
    let mut c = [0.0; 9];
    let mut mean_along = [0.0; 3];
    for r in rotations {
        let x = [r[across], r[3 + across], r[6 + across]];
        for i in 0..3 {
            for j in 0..3 {
                c[i * 3 + j] += x[i] * x[j];
            }
            mean_along[i] += r[3 * i + along];
        }
    }
    let (mut up, least, next) = smallest_eigen(&c);
    if !(next > 1e-3 * rotations.len() as f64 && least < 0.5 * next) {
        return rotations.to_vec();
    }
    if (0..3).map(|i| up[i] * mean_along[i]).sum::<f64>() < 0.0 {
        up = up.map(|v| -v);
    }
    // The turn that takes `up` onto the canvas's axis.
    let target = match axis {
        Axis::Down => [0.0, 1.0, 0.0],
        Axis::Across => [1.0, 0.0, 0.0],
    };
    let cross = [
        up[1] * target[2] - up[2] * target[1],
        up[2] * target[0] - up[0] * target[2],
        up[0] * target[1] - up[1] * target[0],
    ];
    let sin = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let cos: f64 = (0..3).map(|i| up[i] * target[i]).sum();
    if sin < 1e-12 {
        return rotations.to_vec();
    }
    let angle = sin.atan2(cos);
    let turn = rotation(cross.map(|v| v / sin * angle));
    rotations.iter().map(|r| mul(&turn, r)).collect()
}

/// The turned model leveled ([`leveled`]) and every frame's rotation
/// turned so the canvas's middle is straight ahead about the
/// cylinder's axis: a panorama that goes far one way from its
/// reference has its seam of the angles behind it, not across it. A
/// planar model is as it was.
pub fn centered(model: Model, lens: &FrameLens) -> Model {
    let Model::Turned {
        focal,
        rotations,
        projection: Projection::Cylinder(axis),
    } = model
    else {
        return model;
    };
    let rotations = leveled(&rotations, axis);
    let (cx, cy) = (lens.width as f64 / 2.0, lens.height as f64 / 2.0);
    let outline = lens.outline(16);
    let angle = |d: [f64; 3]| match axis {
        Axis::Down => d[0].atan2(d[2]),
        Axis::Across => d[1].atan2(d[2]),
    };
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for r in &rotations {
        let t0 = angle(times(r, [0.0, 0.0, 1.0]));
        for &(x, y) in &outline {
            let t = t0 + wrap(angle(times(r, [(x - cx) / focal, (y - cy) / focal, 1.0])) - t0);
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    let middle = 0.5 * (lo + hi);
    let turn = match axis {
        Axis::Down => rotation([0.0, -middle, 0.0]),
        Axis::Across => rotation([middle, 0.0, 0.0]),
    };
    Model::Turned {
        focal,
        rotations: rotations.iter().map(|r| mul(&turn, r)).collect(),
        projection: Projection::Cylinder(axis),
    }
}

/// A rectangle of the canvas, pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub fn area(&self) -> usize {
        self.width * self.height
    }

    /// Where `self` and `other` both are.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = (self.x + self.width).min(other.x + other.width);
        let b = (self.y + self.height).min(other.y + other.height);
        (r > x && b > y).then_some(Rect {
            x,
            y,
            width: r - x,
            height: b - y,
        })
    }
}

/// The canvas: its size, and where its corner is in the projection's
/// coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canvas {
    pub x0: f64,
    pub y0: f64,
    pub width: usize,
    pub height: usize,
}

/// Each frame's outline on the canvas, in the projection's
/// coordinates: its least and greatest x and y. None for a frame no
/// point of which lands.
fn extents(placement: &Placement, lens: &FrameLens, n: usize) -> Vec<(f64, f64, f64, f64)> {
    let outline = lens.outline(64);
    (0..n)
        .map(|k| {
            outline
                .iter()
                .filter_map(|&(x, y)| placement.to_canvas(k, x, y))
                .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, (x, y)| {
                    (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y))
                })
        })
        .collect()
}

/// The canvas that holds every frame, and each frame's bounding
/// rectangle on it.
pub fn canvas(placement: &Placement, lens: &FrameLens, n: usize) -> Option<(Canvas, Vec<Rect>)> {
    let boxes = extents(placement, lens, n);
    if boxes.iter().any(|b| !(b.0 <= b.2 && b.1 <= b.3)) {
        return None;
    }
    let x0 = boxes.iter().map(|b| b.0).fold(f64::MAX, f64::min).floor();
    let y0 = boxes.iter().map(|b| b.1).fold(f64::MAX, f64::min).floor();
    let x1 = boxes.iter().map(|b| b.2).fold(f64::MIN, f64::max).ceil();
    let y1 = boxes.iter().map(|b| b.3).fold(f64::MIN, f64::max).ceil();
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return None;
    }
    let canvas = Canvas {
        x0,
        y0,
        width: (x1 - x0) as usize,
        height: (y1 - y0) as usize,
    };
    Some((canvas, rects_on(&boxes, &canvas)))
}

/// Each frame's bounding rectangle on a canvas already settled: a
/// merge made again lays its frames on the canvas its tag keeps.
/// None when a frame does not land.
pub fn boxes_on(
    placement: &Placement,
    lens: &FrameLens,
    n: usize,
    canvas: &Canvas,
) -> Option<Vec<Rect>> {
    let boxes = extents(placement, lens, n);
    if boxes.iter().any(|b| !(b.0 <= b.2 && b.1 <= b.3)) {
        return None;
    }
    Some(rects_on(&boxes, canvas))
}

fn rects_on(boxes: &[(f64, f64, f64, f64)], canvas: &Canvas) -> Vec<Rect> {
    let (x0, y0, width, height) = (canvas.x0, canvas.y0, canvas.width, canvas.height);
    boxes
        .iter()
        .map(|b| {
            let x = ((b.0 - x0).floor().max(0.0)) as usize;
            let y = ((b.1 - y0).floor().max(0.0)) as usize;
            let r = (((b.2 - x0).ceil().max(0.0)) as usize).min(width);
            let bt = (((b.3 - y0).ceil().max(0.0)) as usize).min(height);
            Rect {
                x: x.min(width),
                y: y.min(height),
                width: r.saturating_sub(x),
                height: bt.saturating_sub(y),
            }
        })
        .collect()
}

/// The largest rectangle of `covered` (`width` by `height`) that is
/// covered throughout, by the histogram of each row's runs.
// Grid and matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
pub fn largest_rectangle(covered: &[bool], width: usize, height: usize) -> Rect {
    let mut heights = vec![0usize; width];
    let mut best = Rect::default();
    for y in 0..height {
        for x in 0..width {
            heights[x] = if covered[y * width + x] {
                heights[x] + 1
            } else {
                0
            };
        }
        // The largest rectangle under the histogram, by a stack of
        // rising bars.
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for x in 0..=width {
            let hgt = if x < width { heights[x] } else { 0 };
            let mut start = x;
            while let Some(&(sx, sh)) = stack.last() {
                if sh < hgt {
                    break;
                }
                stack.pop();
                let area = sh * (x - sx);
                if area > best.area() {
                    best = Rect {
                        x: sx,
                        y: y + 1 - sh,
                        width: x - sx,
                        height: sh,
                    };
                }
                start = sx;
            }
            stack.push((start, hgt));
        }
    }
    best
}

/// How a frame is resampled onto the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    Bilinear,
    /// Catmull-Rom, four taps a side.
    Cubic,
    /// Lanczos with three lobes, six taps a side.
    Lanczos3,
}

impl Kernel {
    /// Its name in a merge's tag.
    pub fn name(self) -> &'static str {
        match self {
            Kernel::Bilinear => "bilinear",
            Kernel::Cubic => "cubic",
            Kernel::Lanczos3 => "lanczos3",
        }
    }

    /// The kernel of that name.
    pub fn named(name: &str) -> Option<Kernel> {
        [Kernel::Bilinear, Kernel::Cubic, Kernel::Lanczos3]
            .into_iter()
            .find(|k| k.name() == name)
    }

    fn reach(self) -> isize {
        match self {
            Kernel::Bilinear => 1,
            Kernel::Cubic => 2,
            Kernel::Lanczos3 => 3,
        }
    }

    /// The weights of the taps about a position `t` (0 to 1) past the
    /// sample before it: the taps from `1 - reach` to `reach` samples
    /// on. Lanczos's from three sines a position rather than two a tap:
    /// `sin(pi (t - o)) = (-1)^o sin(pi t)`, and the third-period sine
    /// of `t - o` by the angle-difference rule.
    #[inline]
    fn taps(self, t: f64) -> [f64; 6] {
        let mut w = [0.0f64; 6];
        let reach = self.reach();
        if self == Kernel::Lanczos3 {
            use std::f64::consts::PI;
            // cos and sin of pi o / 3, o from -2 to 3.
            const COS: [f64; 6] = [-0.5, 0.5, 1.0, 0.5, -0.5, -1.0];
            const SIN: [f64; 6] = [
                -0.866_025_403_784_438_6,
                -0.866_025_403_784_438_6,
                0.0,
                0.866_025_403_784_438_6,
                0.866_025_403_784_438_6,
                0.0,
            ];
            let s1 = (PI * t).sin();
            let (s3, c3) = (PI * t / 3.0).sin_cos();
            for (i, w) in w.iter_mut().enumerate() {
                let o = i as f64 - 2.0;
                let a = t - o;
                if a.abs() < 1e-9 {
                    *w = 1.0;
                    continue;
                }
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                let third = s3 * COS[i] - c3 * SIN[i];
                *w = 3.0 * sign * s1 * third / (PI * PI * a * a);
            }
            return w;
        }
        for (i, w) in w.iter_mut().enumerate().take((2 * reach) as usize) {
            *w = self.weight(t - (i as isize - reach + 1) as f64);
        }
        w
    }

    #[inline]
    fn weight(self, t: f64) -> f64 {
        let a = t.abs();
        match self {
            Kernel::Bilinear => (1.0 - a).max(0.0),
            Kernel::Cubic => {
                if a < 1.0 {
                    1.5 * a * a * a - 2.5 * a * a + 1.0
                } else if a < 2.0 {
                    -0.5 * a * a * a + 2.5 * a * a - 4.0 * a + 2.0
                } else {
                    0.0
                }
            }
            Kernel::Lanczos3 => {
                if a < 1e-9 {
                    1.0
                } else if a < 3.0 {
                    let p = std::f64::consts::PI * a;
                    3.0 * p.sin() * (p / 3.0).sin() / (p * p)
                } else {
                    0.0
                }
            }
        }
    }
}

/// One channel of interleaved RGB (`width` by `height`) at a
/// continuous point, by `kernel`, the edges clamped, the weights
/// normalized.
// Grid and matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
#[inline]
pub fn sample(
    rgb: &[f32],
    width: usize,
    height: usize,
    channel: usize,
    x: f64,
    y: f64,
    kernel: Kernel,
) -> f32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor() as isize, fy.floor() as isize);
    let (tx, ty) = (fx - ix as f64, fy - iy as f64);
    let reach = kernel.reach();
    let taps = (2 * reach) as usize;
    let (wx, wy) = (kernel.taps(tx), kernel.taps(ty));
    let (sx, sy): (f64, f64) = (wx[..taps].iter().sum(), wy[..taps].iter().sum());
    let (w, h) = (width as isize, height as isize);
    let mut out = 0.0f64;
    for j in 0..taps {
        if wy[j] == 0.0 {
            continue;
        }
        let row = (iy + j as isize - reach + 1).clamp(0, h - 1) as usize * width;
        let mut acc = 0.0f64;
        for i in 0..taps {
            let col = (ix + i as isize - reach + 1).clamp(0, w - 1) as usize;
            acc += wx[i] * f64::from(rgb[(row + col) * 3 + channel]);
        }
        out += wy[j] * acc;
    }
    (out / (sx * sy)) as f32
}

/// A frame's samples as the warp reads them: interleaved camera RGB,
/// `width` by `height`, at `scale` sensor pixels a sample (one for the
/// frame itself, more for a reduced copy).
pub struct Source<'a> {
    pub rgb: &'a [f32],
    pub width: usize,
    pub height: usize,
    pub scale: f64,
}

/// What a frame looks like over a rectangle of the canvas.
pub struct Warped {
    pub rect: Rect,
    /// Interleaved RGB, not a number where the frame is not.
    pub rgb: Vec<f32>,
    /// Each pixel's distance from the frame's edge on the sensor, in
    /// sensor pixels; zero or less where the frame is not.
    pub inside: Vec<f32>,
}

/// How [`warp`] resamples a frame.
#[derive(Debug, Clone, Copy)]
pub struct Resampling {
    /// Canvas pixels a pixel: one for the canvas itself.
    pub step: f64,
    pub kernel: Kernel,
    /// Each channel's multiplier on the pixel's brightness.
    pub gain: [f32; 3],
    /// Sensor pixels from the edge within which the green light is
    /// not the frame's.
    pub margin: f64,
}

/// Frame `k` resampled over `rect` of a canvas at the [`Resampling`]'s
/// `step` canvas pixels a pixel (one for the canvas itself), through
/// its lens and its placement, each channel at its own place, by its
/// `kernel`; each pixel's brightness multiplied by its `gain`'s
/// channel. A pixel whose green light falls within its `margin` sensor
/// pixels of the edge, or off it, is not the frame's.
pub fn warp(
    source: &Source<'_>,
    placement: &Placement,
    lens: &FrameLens,
    canvas: &Canvas,
    k: usize,
    rect: Rect,
    resampling: Resampling,
) -> Warped {
    let Resampling {
        step,
        kernel,
        gain,
        margin,
    } = resampling;
    let (sw, sh) = (lens.width as f64, lens.height as f64);
    // A distortion's polynomial folds back past some radius, and a
    // corrected point far outside the frame would find light on the
    // sensor there: none past the outline's farthest point.
    let (mx, my) = (sw / 2.0, sh / 2.0);
    let reach = lens.moves().then(|| {
        lens.outline(64)
            .iter()
            .map(|&(x, y)| (x - mx).hypot(y - my))
            .fold(0.0, f64::max)
            + 1.0
    });
    let mut rgb = vec![f32::NAN; rect.area() * 3];
    let mut inside = vec![0.0f32; rect.area()];
    rgb.par_chunks_mut(rect.width * 3)
        .zip(inside.par_chunks_mut(rect.width))
        .enumerate()
        .for_each(|(j, (row, ins))| {
            let cy = canvas.y0 + (rect.y + j) as f64 * step + 0.5 * step;
            for i in 0..rect.width {
                let cx = canvas.x0 + (rect.x + i) as f64 * step + 0.5 * step;
                let Some((x, y)) = placement.to_frame(k, cx, cy) else {
                    continue;
                };
                if reach.is_some_and(|r| (x - mx).hypot(y - my) > r) {
                    continue;
                }
                let (gx, gy) = lens.source(1, x, y);
                let d = gx.min(sw - gx).min(gy).min(sh - gy);
                ins[i] = d as f32;
                if d <= margin {
                    continue;
                }
                for c in 0..3 {
                    let (px, py) = if c == 1 {
                        (gx, gy)
                    } else {
                        lens.source(c, x, y)
                    };
                    row[i * 3 + c] = gain[c]
                        * sample(
                            source.rgb,
                            source.width,
                            source.height,
                            c,
                            px / source.scale,
                            py / source.scale,
                            kernel,
                        );
                }
            }
        });
    Warped { rect, rgb, inside }
}

/// `rgb` (`width` by `height`, interleaved) box-averaged by `factor`
/// both ways, a block that reaches past the edge averaged over what it
/// has; and each block's flag, whether any of its samples is clipped
/// (any channel at or over [`super::CLIP`], or not a number).
pub fn reduce_by(
    rgb: &[f32],
    width: usize,
    height: usize,
    factor: usize,
) -> (Vec<f32>, Vec<bool>, usize, usize) {
    let (w, h) = (width.div_ceil(factor), height.div_ceil(factor));
    let mut out = vec![0.0f32; w * h * 3];
    let mut clipped = vec![false; w * h];
    out.par_chunks_mut(w * 3)
        .zip(clipped.par_chunks_mut(w))
        .enumerate()
        .for_each(|(by, (row, clip))| {
            for bx in 0..w {
                let mut s = [0.0f64; 3];
                let mut n = 0usize;
                let mut hit = false;
                for y in by * factor..((by + 1) * factor).min(height) {
                    for x in bx * factor..((bx + 1) * factor).min(width) {
                        let p = &rgb[(y * width + x) * 3..(y * width + x) * 3 + 3];
                        if p.iter().any(|v| !v.is_finite() || *v >= super::CLIP) {
                            hit = true;
                        }
                        for c in 0..3 {
                            if p[c].is_finite() {
                                s[c] += f64::from(p[c]);
                            }
                        }
                        n += 1;
                    }
                }
                for c in 0..3 {
                    row[bx * 3 + c] = (s[c] / n.max(1) as f64) as f32;
                }
                clip[bx] = hit;
            }
        });
    (out, clipped, w, h)
}

/// Each channel's median ratio of `b` over `a` (both interleaved RGB
/// over one rectangle, not a number where a frame is not) over the
/// pixels both hold unclipped and well exposed, between
/// [`super::LOW`] and [`super::KNEE`]; with how many there were. None
/// with fewer than `least`.
pub fn overlap_ratio(a: &[f32], b: &[f32], least: usize) -> Option<([f64; 3], usize)> {
    let mut r: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (p, q) in a.chunks(3).zip(b.chunks(3)) {
        if !p.iter().chain(q).all(|v| v.is_finite() && *v > 0.0) {
            continue;
        }
        let (la, lb) = (super::luma(p), super::luma(q));
        if !(super::LOW..super::KNEE).contains(&la) || !(super::LOW..super::KNEE).contains(&lb) {
            continue;
        }
        if p.iter().chain(q).any(|v| *v >= super::CLIP) {
            continue;
        }
        for c in 0..3 {
            r[c].push(q[c] / p[c]);
        }
    }
    let n = r[1].len();
    if n < least {
        return None;
    }
    let mut out = [1.0; 3];
    for c in 0..3 {
        let mid = r[c].len() / 2;
        r[c].select_nth_unstable_by(mid, f32::total_cmp);
        out[c] = f64::from(r[c][mid]);
    }
    Some((out, n))
}

/// Each frame's per-channel gain onto the reference's scale from the
/// pairs' ratios (`(a, b, b over a, how many pixels)`): the log gains
/// that fit every pair best in least squares, each pair weighted by
/// the root of its count, the reference's held at one. A frame no pair
/// reaches keeps one.
// Grid and matrix code: the indices are the mathematics.
#[allow(clippy::needless_range_loop)]
pub fn gains(
    pairs: &[(usize, usize, [f64; 3], usize)],
    n: usize,
    reference: usize,
) -> Vec<[f64; 3]> {
    let free: Vec<usize> = (0..n).filter(|&k| k != reference).collect();
    let index = |k: usize| free.iter().position(|&f| f == k);
    let m = free.len();
    let mut out = vec![[1.0; 3]; n];
    if m == 0 {
        return out;
    }
    for c in 0..3 {
        let mut a = vec![vec![0.0; m]; m];
        let mut b = vec![0.0; m];
        for &(i, j, ratio, count) in pairs {
            // gain_b * b = gain_a * a: log gain_b - log gain_a = -log r.
            let w = (count as f64).sqrt();
            let target = -ratio[c].max(1e-9).ln();
            let (ia, ib) = (index(i), index(j));
            if let Some(ib) = ib {
                a[ib][ib] += w;
                b[ib] += w * target;
            }
            if let Some(ia) = ia {
                a[ia][ia] += w;
                b[ia] -= w * target;
            }
            if let (Some(ia), Some(ib)) = (ia, ib) {
                a[ia][ib] -= w;
                a[ib][ia] -= w;
            }
        }
        // A frame no pair touches stays where it is.
        for i in 0..m {
            if a[i][i] == 0.0 {
                a[i][i] = 1.0;
            }
        }
        if let Some(x) = solve_dense(a, b) {
            for (i, &k) in free.iter().enumerate() {
                out[k][c] = x[i].exp();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 600;
    const H: usize = 400;

    /// The middle of a turn shot right to left in uneven steps is the
    /// frame nearest the mean turn, not the list's middle; turned onto
    /// it, that frame is the identity and each frame keeps its turn
    /// from every other. A planar model's middle is the frame whose
    /// middle lands nearest the mean of the middles.
    #[test]
    fn the_middle_of_the_turn_and_the_frames_turned_onto_it() {
        let yaws = [0.5, 0.3, 0.0, -0.3];
        let rotations: Vec<Mat3> = yaws.iter().map(|&y| rotation([0.0, y, 0.0])).collect();
        let model = Model::Turned {
            focal: 500.0,
            rotations: rotations.clone(),
            projection: Projection::Planar,
        };
        assert_eq!(model.middle(W, H), 2);
        let Model::Turned {
            rotations: onto, ..
        } = model.onto(2)
        else {
            unreachable!()
        };
        assert_eq!(onto[2], IDENTITY);
        for (a, b) in [(0, 1), (1, 3), (0, 3)] {
            let before = mul(&transpose(&rotations[a]), &rotations[b]);
            let after = mul(&transpose(&onto[a]), &onto[b]);
            for (x, y) in before.iter().zip(&after) {
                assert!((x - y).abs() < 1e-12);
            }
        }
        let shift = |dx: f64| Homography {
            m: [1.0, 0.0, dx, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let planar = Model::Planar {
            homographies: vec![shift(500.0), shift(300.0), shift(0.0), shift(-300.0)],
        };
        assert_eq!(planar.middle(W, H), 2);
        let Model::Planar { homographies } = planar.onto(1) else {
            unreachable!()
        };
        assert_eq!(homographies[1], Homography::IDENTITY);
        assert!((homographies[3].apply(0.0, 0.0).0 + 600.0).abs() < 1e-9);
    }

    /// Two frames score the same middle but for rounding, and the
    /// first wins however the last bit falls: a score a rounding worse
    /// than the least is still a tie, one clearly worse is not.
    #[test]
    fn a_tie_for_the_middle_goes_to_the_first_frame() {
        let mut second = rotation([0.0, 0.3, 0.0]);
        second[0] += 1e-13;
        let model = Model::Turned {
            focal: 500.0,
            rotations: vec![IDENTITY, second],
            projection: Projection::Planar,
        };
        assert_eq!(model.middle(W, H), 0);
        let model = Model::Turned {
            focal: 500.0,
            rotations: vec![rotation([0.0, 0.3, 0.0]), IDENTITY],
            projection: Projection::Planar,
        };
        assert_eq!(model.middle(W, H), 0);
        let shift = |dx: f64| Homography {
            m: [1.0, 0.0, dx, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let planar = Model::Planar {
            homographies: vec![shift(300.0 + 1e-10), shift(0.0)],
        };
        assert_eq!(planar.middle(W, H), 0);
        let planar = Model::Planar {
            homographies: vec![shift(300.0), shift(0.0), shift(150.0)],
        };
        assert_eq!(planar.middle(W, H), 2);
    }

    /// A frame turned 40 degrees about the vertical lands on a cylinder
    /// where the angle says, and back; the planar homography of the
    /// turn is the rotation's, and the adjustment recovers the turn and
    /// the focal length from the points of that homography alone.
    #[test]
    fn the_cylinder_on_a_known_rotation() {
        let f = 500.0;
        let turn = 40f64.to_radians();
        let r = rotation([0.0, turn, 0.0]);
        let placement = Placement {
            model: Model::Turned {
                focal: f,
                rotations: vec![IDENTITY, r],
                projection: Projection::Cylinder(Axis::Down),
            },
            width: W,
            height: H,
        };
        let (cx, cy) = (W as f64 / 2.0, H as f64 / 2.0);
        // The turned frame's middle is 40 degrees round the cylinder.
        let (x, y) = placement.to_canvas(1, cx, cy).unwrap();
        assert!((x - (cx + f * turn)).abs() < 1e-9, "{x}");
        assert!((y - cy).abs() < 1e-9);
        // And back, anywhere on the frame.
        for &(px, py) in &[(10.0, 10.0), (590.0, 200.0), (300.0, 390.0)] {
            let (u, v) = placement.to_canvas(1, px, py).unwrap();
            let (bx, by) = placement.to_frame(1, u, v).unwrap();
            assert!((bx - px).abs() < 1e-6 && (by - py).abs() < 1e-6);
        }
        // A point on the frame's horizon stays on the canvas's.
        let (_, y) = placement.to_canvas(1, 50.0, cy).unwrap();
        assert!((y - cy).abs() < 1e-9);

        // The homography that turn makes between the two frames, and
        // the adjustment from its points at a focal length 15% off.
        let k = [f, 0.0, cx, 0.0, f, cy, 0.0, 0.0, 1.0];
        let ki = inverse3(&k).unwrap();
        // b = K R^T K^-1 a: frame 1's rays are R^T of the reference's.
        let h = Homography {
            m: mul(&k, &mul(&transpose(&r), &ki)),
        }
        .normalized();
        let back = rotation_of(&h, f, (cx, cy));
        for (a, b) in back.iter().zip(&r) {
            assert!((a - b).abs() < 1e-9, "{back:?}");
        }
        let link = Link {
            a: 0,
            b: 1,
            strength: 100,
            points: grid_points(&h, W, H, 10),
            homography: h,
        };
        assert!(link.points.len() > 20, "{}", link.points.len());
        let adjusted = adjust_turned(&[link], 2, 0, f * 1.15, true, (W, H));
        let Model::Turned {
            focal, rotations, ..
        } = adjusted.model
        else {
            unreachable!()
        };
        assert!((focal - f).abs() < 0.01 * f, "{focal}");
        let got = rotations[1];
        assert!(
            got.iter().zip(&r).all(|(a, b)| (a - b).abs() < 1e-3),
            "{got:?}"
        );
        assert!(adjusted.residuals[0] < 0.05, "{:?}", adjusted.residuals);
    }

    /// A chain of three frames whose middle link is a little wrong, and
    /// a link from the first to the last that is right: the adjustment
    /// spreads the error, and lands nearer the truth than the chain.
    #[test]
    fn the_adjustment_spreads_a_chains_drift() {
        let f = 500.0;
        let (cx, cy) = (W as f64 / 2.0, H as f64 / 2.0);
        let k = [f, 0.0, cx, 0.0, f, cy, 0.0, 0.0, 1.0];
        let ki = inverse3(&k).unwrap();
        let truth = [
            IDENTITY,
            rotation([0.0, 0.3, 0.0]),
            rotation([0.0, 0.6, 0.02]),
        ];
        let between = |a: &Mat3, b: &Mat3| {
            // a's points to b's: b = K Rb^T Ra K^-1 a.
            Homography {
                m: mul(&k, &mul(&transpose(b), &mul(a, &ki))),
            }
            .normalized()
        };
        let link = |a: usize, b: usize, h: Homography| Link {
            a,
            b,
            strength: 50,
            points: grid_points(&h, W, H, 10),
            homography: h,
        };
        // The second link a shade off: as if the third frame were
        // turned 0.62 instead of 0.6.
        let off = rotation([0.0, 0.62, 0.02]);
        let links = vec![
            link(0, 1, between(&truth[0], &truth[1])),
            link(1, 2, between(&truth[1], &off)),
            link(0, 2, between(&truth[0], &truth[2])),
        ];
        // The focal length held, as it is known here: fitted beside the
        // turns, it takes up part of a loop's disagreement itself.
        let adjusted = adjust_turned(&links, 3, 0, f, false, (W, H));
        let Model::Turned { rotations, .. } = adjusted.model else {
            unreachable!()
        };
        let err = |r: &Mat3| {
            let d = mul(&transpose(r), &truth[2]);
            ((d[0] + d[4] + d[8] - 1.0) / 2.0).clamp(-1.0, 1.0).acos()
        };
        assert!(err(&rotations[2]) < 0.015, "{}", err(&rotations[2]));
        // Both pairs that disagree carry part of it.
        assert!(adjusted.residuals[1] > 0.1 && adjusted.residuals[2] > 0.1);
    }

    #[test]
    fn the_field_picks_the_projection() {
        assert_eq!(projection_for(80.0, 60.0), Ok(Projection::Planar));
        assert_eq!(
            projection_for(150.0, 60.0),
            Ok(Projection::Cylinder(Axis::Down))
        );
        assert_eq!(
            projection_for(60.0, 150.0),
            Ok(Projection::Cylinder(Axis::Across))
        );
        let past = projection_for(200.0, 130.0).unwrap_err();
        assert!(past.contains("a cylinder holds up to 120"), "{past}");
    }

    #[test]
    fn the_largest_rectangle_of_a_coverage() {
        let (w, h) = (12, 8);
        let mut covered = vec![false; w * h];
        for y in 1..7 {
            for x in 2..11 {
                covered[y * w + x] = true;
            }
        }
        // A bite out of the top right.
        covered[w + 9] = false;
        covered[w + 10] = false;
        assert_eq!(
            largest_rectangle(&covered, w, h),
            Rect {
                x: 2,
                y: 2,
                width: 9,
                height: 5
            }
        );
    }

    /// The lens's distortion and the frame's turn are one map: a
    /// frame whose samples are their own sensor positions, warped onto
    /// the canvas, holds at each canvas point the sensor point that the
    /// turn undone and then the distortion put there, worked by hand;
    /// Newton's inverse takes that sensor point back to the corrected
    /// one, and the corrected one back to the canvas point; and the
    /// outline lies on the sensor's edge.
    #[test]
    fn the_lens_and_the_turn_are_one_map() {
        let k1 = -0.03f64;
        let lens = FrameLens::of(
            &[LensCorrection {
                distortion: Some(crate::develop::lens::Distortion::Poly3 { k1: k1 as f32 }),
                ..Default::default()
            }],
            W,
            H,
        );
        let f = 500.0;
        let r = rotation([0.05, 0.3, 0.02]);
        let placement = Placement {
            model: Model::Turned {
                focal: f,
                rotations: vec![IDENTITY, r],
                projection: Projection::Planar,
            },
            width: W,
            height: H,
        };
        let (cx, cy) = (W as f64 / 2.0, H as f64 / 2.0);
        let half = cx.min(cy);
        // The turn undone, then the distortion, by hand.
        let corrected_of = |x: f64, y: f64| {
            let d = times(&transpose(&r), [(x - cx) / f, (y - cy) / f, 1.0]);
            (f * d[0] / d[2] + cx, f * d[1] / d[2] + cy)
        };
        let distorted = |(x, y): (f64, f64)| {
            let (u, v) = ((x - cx) / half, (y - cy) / half);
            let g = 1.0 - k1 + k1 * (u * u + v * v);
            (cx + u * g * half, cy + v * g * half)
        };

        let mut rgb = vec![0.0f32; W * H * 3];
        for (i, p) in rgb.chunks_mut(3).enumerate() {
            p[0] = (i % W) as f32 + 0.5;
            p[1] = (i / W) as f32 + 0.5;
        }
        let (canvas, boxes) = canvas(&placement, &lens, 2).unwrap();
        let warped = warp(
            &Source {
                rgb: &rgb,
                width: W,
                height: H,
                scale: 1.0,
            },
            &placement,
            &lens,
            &canvas,
            1,
            boxes[1],
            Resampling {
                step: 1.0,
                kernel: Kernel::Bilinear,
                gain: [1.0; 3],
                margin: 2.0,
            },
        );
        let mut checked = 0;
        for (n, p) in warped.rgb.chunks(3).enumerate() {
            if !p[0].is_finite() || n % 7 != 0 {
                continue;
            }
            let (i, j) = (n % warped.rect.width, n / warped.rect.width);
            let at = (
                canvas.x0 + (warped.rect.x + i) as f64 + 0.5,
                canvas.y0 + (warped.rect.y + j) as f64 + 0.5,
            );
            let corrected = corrected_of(at.0, at.1);
            let sensor = distorted(corrected);
            let got = (f64::from(p[0]), f64::from(p[1]));
            assert!(
                (got.0 - sensor.0).hypot(got.1 - sensor.1) < 1e-3,
                "{at:?}: {got:?} against {sensor:?}"
            );
            let back = lens.corrected(sensor.0, sensor.1).unwrap();
            assert!(
                (back.0 - corrected.0).hypot(back.1 - corrected.1) < 1e-3,
                "{back:?} against {corrected:?}"
            );
            let again = placement.to_canvas(1, back.0, back.1).unwrap();
            assert!((again.0 - at.0).hypot(again.1 - at.1) < 1e-3, "{again:?}");
            checked += 1;
        }
        assert!(checked > 1000, "{checked}");

        let outline = lens.outline(16);
        assert_eq!(outline.len(), 64);
        for (x, y) in outline {
            let (sx, sy) = lens.source(1, x, y);
            let edge = sx
                .abs()
                .min((sx - W as f64).abs())
                .min(sy.abs())
                .min((sy - H as f64).abs());
            assert!(edge < 1e-3, "{sx}, {sy}");
        }

        // A profile strong enough that no corrected point reaches the
        // sensor's corner: the inverse finds none there, not the point
        // past the fold that also lands on it.
        let folded = FrameLens::of(
            &[LensCorrection {
                distortion: Some(crate::develop::lens::Distortion::Poly3 { k1: -0.08 }),
                ..Default::default()
            }],
            W,
            H,
        );
        assert_eq!(folded.corrected(0.5, 0.5), None);
        assert!(folded.corrected(cx + 10.0, cy + 10.0).is_some());
    }

    /// At a full frame's size, where one f32 step is half a
    /// thousandth of a pixel, every point of the sensor, its edges and
    /// corners too, has its corrected point, to a ten-thousandth, for a
    /// wide lens's barrel.
    #[test]
    fn the_inverse_settles_at_a_full_frames_size() {
        let (w, h) = (6000usize, 4000usize);
        let lens = FrameLens::of(
            &[LensCorrection {
                distortion: Some(crate::develop::lens::Distortion::Ptlens {
                    a: 0.012,
                    b: -0.041,
                    c: 0.015,
                }),
                ..Default::default()
            }],
            w,
            h,
        );
        let mut missed = Vec::new();
        for j in 0..=40 {
            for i in 0..=60 {
                let (sx, sy) = (i as f64 * w as f64 / 60.0, j as f64 * h as f64 / 40.0);
                match lens.corrected(sx, sy) {
                    Some((x, y)) => {
                        let (u, v) = lens.source(1, x, y);
                        assert!((u - sx).hypot(v - sy) < 1e-4, "{sx}, {sy}");
                    }
                    None => missed.push((sx, sy)),
                }
            }
        }
        assert!(missed.is_empty(), "{} missed: {missed:?}", missed.len());
        assert_eq!(lens.outline(64).len(), 256);
    }

    /// A camera tipped down as it turned about the vertical: placed as
    /// it turned, its horizon is a wave on the cylinder; leveled and
    /// centered, a straight line.
    #[test]
    fn a_tipped_turn_levels_to_a_straight_horizon() {
        let f = 500.0;
        let tip = rotation([0.15, 0.0, 0.0]);
        let world: Vec<Mat3> = [-0.6f64, -0.2, 0.2, 0.6]
            .iter()
            .map(|&yaw| mul(&rotation([0.0, yaw, 0.0]), &tip))
            .collect();
        // Each frame's rays onto the reference's (the second frame).
        let rotations: Vec<Mat3> = world
            .iter()
            .map(|w| mul(&transpose(&world[1]), w))
            .collect();
        let lens = FrameLens::none(W, H);
        let (cx, cy) = (W as f64 / 2.0, H as f64 / 2.0);
        // The horizon's points each frame sees, on the frame.
        let horizon: Vec<(usize, f64, f64)> = (0..world.len())
            .flat_map(|k| {
                let w = world[k];
                (0..200).filter_map(move |i| {
                    let a = -1.2 + 2.4 * i as f64 / 199.0;
                    let d = times(&transpose(&w), [a.sin(), 0.0, a.cos()]);
                    let (x, y) = (f * d[0] / d[2] + cx, f * d[1] / d[2] + cy);
                    (d[2] > 0.0 && (0.0..W as f64).contains(&x) && (0.0..H as f64).contains(&y))
                        .then_some((k, x, y))
                })
            })
            .collect();
        assert!(horizon.len() > 200, "{}", horizon.len());
        let spread = |model: Model| {
            let placement = Placement {
                model,
                width: W,
                height: H,
            };
            let ys: Vec<f64> = horizon
                .iter()
                .map(|&(k, x, y)| placement.to_canvas(k, x, y).unwrap().1)
                .collect();
            let lo = ys.iter().copied().fold(f64::MAX, f64::min);
            let hi = ys.iter().copied().fold(f64::MIN, f64::max);
            hi - lo
        };
        let turned = Model::Turned {
            focal: f,
            rotations,
            projection: Projection::Cylinder(Axis::Down),
        };
        let wave = spread(turned.clone());
        assert!(wave > 20.0, "{wave}");
        let level = spread(centered(turned, &lens));
        assert!(level < 1e-6, "{level}");
    }

    /// The kernels pass a constant and a ramp through, between and on
    /// the samples; Lanczos's taps from three sines are its weights.
    #[test]
    fn the_kernels_keep_a_ramp() {
        for t in [0.0, 0.1, 0.5, 0.77, 0.999] {
            let taps = Kernel::Lanczos3.taps(t);
            for (i, w) in taps.iter().enumerate() {
                let want = Kernel::Lanczos3.weight(t - (i as f64 - 2.0));
                assert!((w - want).abs() < 1e-12, "{t} {i}: {w} against {want}");
            }
        }
        let (w, h) = (16, 8);
        let rgb: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let x = (i % w) as f32;
                [1.0, x, 2.0 * x]
            })
            .collect();
        for kernel in [Kernel::Bilinear, Kernel::Cubic, Kernel::Lanczos3] {
            for x in [5.5, 6.25, 7.9] {
                let v = sample(&rgb, w, h, 1, x, 4.0, kernel);
                // Lanczos's lobes do not sum a ramp exactly: a part in
                // fifty of a sample's step between the samples.
                let tolerance = if kernel == Kernel::Lanczos3 {
                    0.03
                } else {
                    2e-3
                };
                assert!(
                    (v - (x as f32 - 0.5)).abs() < tolerance,
                    "{kernel:?} {x} {v}"
                );
                assert!((sample(&rgb, w, h, 0, x, 4.3, kernel) - 1.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn the_gains_from_the_pairs() {
        // Frame 1 reads 10% bright in red against 0, frame 2 20% dark
        // against 1.
        let pairs = vec![(0, 1, [1.1, 1.0, 1.0], 1000), (1, 2, [0.8, 0.8, 0.8], 1000)];
        let g = gains(&pairs, 3, 1);
        assert_eq!(g[1], [1.0; 3]);
        assert!((g[0][0] - 1.1).abs() < 1e-9, "{g:?}");
        assert!((g[2][1] - 1.25).abs() < 1e-9, "{g:?}");
    }
}
