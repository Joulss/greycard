//! A camera profile's own tone curve, in the display curve's place
//! (`Camera::tone_curve`): the curve a DCP's author rendered it with,
//! applied as the DNG specification's processing model places it. The
//! method is Adobe's, from the specification (1.7.1,
//! `ProfileToneCurve`); the code is ours, written from it.
//!
//! In that model the picture is in linear ProPhoto, exposed by the
//! file's `BaselineExposure`, the profile's `BaselineExposureOffset`,
//! and the user's exposure, each channel clipped at one, and the
//! profile's look table read; then the curve, put on the largest and
//! the smallest channel with the middle one placed between the two
//! results as it sat between them before ([`hue_preserving_tone`]), which keeps
//! the hue of the RGB hexcone and lets the curve's contrast raise
//! saturation as a curve per channel does. Here the picture arrives in
//! the working space after the Light section's shape, at our baseline
//! and the user's exposure; the curve reads it at Adobe's exposure
//! (`baseline::adobe_lift`, the look table read there too) with the
//! profile's offset on. The clip at one is the model's: what is over
//! the curve's white is white, so the scene's light over white is the
//! Highlights' and Whites' to bring under. Out comes display light in
//! ProPhoto, taken back to the working space for the point curves and
//! the output as the display curve's output is.
//!
//! The specification's `DefaultBlackRender` Auto asks for a black
//! subtraction, which the develop never does; it is left out.

use std::sync::{Arc, LazyLock};

use greycard_core::tone_curve::ToneCurve;

/// A 3x3 matrix by its rows.
type Rows = [[f32; 3]; 3];

/// The working space to linear ProPhoto and back, rows.
static PROPHOTO: LazyLock<(Rows, Rows)> = LazyLock::new(greycard_core::color::prophoto_matrices);

const MID_GREY: f32 = 0.18;

/// A profile's curve ready for the finish: the curve, the profile's
/// `BaselineExposureOffset`, and the gain the curve reads the picture
/// at, once placed at a source ([`ProfileTone::placed`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileTone {
    curve: Arc<ToneCurve>,
    /// `BaselineExposureOffset`, stops.
    offset: f32,
    /// `2^(offset + lift)`: the lift is Adobe's `BaselineExposure`
    /// less ours ([`crate::finish::Source::adobe_lift`]), nothing until
    /// placed.
    gain: f32,
}

impl ProfileTone {
    /// The curve with the profile's `offset` in stops, which
    /// `dcp::sane_offset` has held to ten either way.
    pub fn new(curve: Arc<ToneCurve>, offset: Option<f32>) -> ProfileTone {
        let offset = offset
            .and_then(greycard_core::dcp::sane_offset)
            .unwrap_or(0.0);
        ProfileTone {
            curve,
            offset,
            gain: offset.exp2(),
        }
    }

    /// This curve read where Adobe would read it on a picture of
    /// `source` under `light`: at Adobe's `BaselineExposure` and the
    /// profile's offset, rather than at our baseline
    /// ([`crate::finish::Source::adobe_lift`]). The finish, the
    /// dropper, and the viewport each place the curve they are handed.
    pub fn placed(&self, source: crate::finish::Source, light: &greycard_edit::Light) -> Self {
        ProfileTone {
            gain: (self.offset + source.adobe_lift(light)).exp2(),
            ..self.clone()
        }
    }

    /// The curve `edit` renders with in the display curve's place: its
    /// switch on, and the profile it names, read through `loader`,
    /// carrying one. `None` otherwise, which is the display curve.
    pub fn of(edit: &greycard_edit::Edit, loader: &dyn greycard_edit::Loader) -> Option<Self> {
        if !edit.camera.wants_tone_curve() {
            return None;
        }
        let profile = edit.camera.profile(loader)?;
        let curve = profile.tone_curve.clone()?;
        Some(ProfileTone::new(curve, profile.baseline_exposure_offset))
    }

    /// Where the curve reaches its white, in stops over mid grey of the
    /// exposed scene: what the Whites aim at and a merge's pull is
    /// placed under, as the display curve's white is. The white is where
    /// the curve first reaches its largest output ([`ToneCurve::white`]),
    /// which is one only for a curve that rises to its end and ends
    /// at one: a curve that reaches one at 0.85 has its white there,
    /// and one that ends at 0.97 at its end. A curve flat from the
    /// start has its white a table step up, so the stops are finite.
    pub fn white_stops(&self) -> f32 {
        let at = self.curve.white()[0].max(1.0 / greycard_core::tone_curve::TABLE_STEPS as f32);
        (at / (self.gain * MID_GREY)).log2()
    }

    /// The table the shader reads, as [`ToneCurve::table`].
    pub fn table(&self) -> &[f32] {
        self.curve.table()
    }

    /// The gain before the curve, for the shader.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// The working space to linear ProPhoto and back, rows, for the
    /// shader.
    pub fn matrices() -> &'static (Rows, Rows) {
        &PROPHOTO
    }

    /// A shaped scene pixel in the working space to display light in
    /// the working space.
    #[inline]
    pub fn apply(&self, c: [f32; 3]) -> [f32; 3] {
        let (to, from) = &*PROPHOTO;
        let p = mul(to, c).map(|v| {
            let v = v * self.gain;
            // Clipped at one, as the processing model's exposure is;
            // a NaN is black.
            if v > 0.0 { v.min(1.0) } else { 0.0 }
        });
        mul(from, hue_preserving_tone(p, |x| self.curve.at(x)))
    }
}

fn mul(m: &[[f32; 3]; 3], c: [f32; 3]) -> [f32; 3] {
    m.map(|r| r[0] * c[0] + r[1] * c[1] + r[2] * c[2])
}

/// The curve `f` on the largest and the smallest of the three
/// channels, and the middle one placed between the two results in the
/// proportion it held between them before; equal channels all take
/// the curve. A neutral is the curve, and a color keeps its hexcone
/// hue. Without sorting: each channel is placed by where it sits
/// between the two ends, which puts the ends themselves on their own
/// results.
#[inline]
fn hue_preserving_tone(c: [f32; 3], f: impl Fn(f32) -> f32) -> [f32; 3] {
    let hi = c[0].max(c[1]).max(c[2]);
    let lo = c[0].min(c[1]).min(c[2]);
    let (top, bottom) = (f(hi), f(lo));
    if hi <= lo {
        return [top; 3];
    }
    let span = hi - lo;
    c.map(|v| {
        if v >= hi {
            top
        } else if v <= lo {
            bottom
        } else {
            bottom + (top - bottom) * ((v - lo) / span)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s_curve() -> ProfileTone {
        let points: Vec<[f32; 2]> = (0..=32)
            .map(|i| {
                let x = i as f32 / 32.0;
                [x, x * x * (3.0 - 2.0 * x) * 0.5 + x.sqrt() * 0.5]
            })
            .collect();
        ProfileTone::new(Arc::new(ToneCurve::new(&points).unwrap()), Some(-0.5))
    }

    /// A neutral is the curve at its exposure, white at the curve's
    /// white and over it.
    #[test]
    fn a_neutral_is_the_curve() {
        let t = s_curve();
        let curve = ToneCurve::new(t.curve.points()).unwrap();
        for v in [0.0, 0.01, 0.05, 0.18, 0.4, 0.9, 1.3, 1.5, 4.0] {
            let out = t.apply([v; 3]);
            let want = curve.at(v * t.gain);
            for k in out {
                assert!((k - want).abs() < 2e-5, "{v}: {out:?} {want}");
            }
        }
        let white = MID_GREY * t.white_stops().exp2();
        assert!((white - 2f32.powf(0.5)).abs() < 1e-5);
        assert!((t.apply([white; 3])[1] - 1.0).abs() < 2e-5);
    }

    /// Seven cases by hand: the largest and the smallest
    /// through the curve, the middle between them by where it was.
    #[test]
    fn the_middle_channel_keeps_its_place() {
        let f = |x: f32| x.sqrt();
        for c in [
            [0.8, 0.4, 0.1],
            [0.4, 0.1, 0.8],
            [0.8, 0.1, 0.4],
            [0.8, 0.2, 0.2],
            [0.4, 0.8, 0.1],
            [0.1, 0.4, 0.8],
            [0.1, 0.8, 0.4],
            [0.3, 0.3, 0.3],
        ] {
            let out = hue_preserving_tone(c, f);
            let (hi, lo) = (
                c.iter().copied().fold(0.0, f32::max),
                c.iter().copied().fold(1.0, f32::min),
            );
            for k in 0..3 {
                let want = if hi == lo {
                    f(hi)
                } else {
                    f(lo) + (f(hi) - f(lo)) * (c[k] - lo) / (hi - lo)
                };
                assert!((out[k] - want).abs() < 1e-6, "{c:?} {out:?}");
            }
        }
    }

    /// The Whites aim where the curve first tops out: at 0.85 for a
    /// curve that reaches one there, at its end for one that ends at
    /// 0.97, and a neutral there comes out at the curve's top.
    #[test]
    fn the_white_is_where_the_curve_tops_out() {
        for (points, at, top) in [
            (vec![[0.0, 0.0], [0.4, 0.6], [0.85, 1.0]], 0.85f32, 1.0f32),
            (vec![[0.0, 0.0], [0.5, 0.55], [1.0, 0.97]], 1.0, 0.97),
        ] {
            let t = ProfileTone::new(Arc::new(ToneCurve::new(&points).unwrap()), Some(-0.5));
            let white = MID_GREY * t.white_stops().exp2();
            let want = at * 2f32.powf(0.5);
            assert!((white - want).abs() < 2e-3 * want, "{white} {want}");
            let out = t.apply([white; 3]);
            assert!((out[1] - top).abs() < 1e-4, "{out:?} {top}");
            // Under it is under the top; over it, the top still.
            assert!(t.apply([white * 0.9; 3])[1] < top - 1e-3);
            assert!((t.apply([white * 1.5; 3])[1] - top).abs() < 1e-4);
        }
    }

    /// Nothing comes out that is not a number, whatever comes in.
    #[test]
    fn what_is_not_a_number_comes_out_black_or_white() {
        let t = s_curve();
        for c in [
            [f32::NAN, 0.2, 0.3],
            [f32::INFINITY, 0.2, 0.3],
            [f32::NEG_INFINITY; 3],
            [-1.0, 5.0, 0.0],
        ] {
            let out = t.apply(c);
            assert!(out.iter().all(|v| v.is_finite()), "{c:?} {out:?}");
        }
    }

    /// Every ordering of three channels and every tie: the largest and
    /// the smallest take the curve, the middle sits between the results
    /// as it sat between the inputs, and equal channels are the curve.
    #[test]
    fn the_hue_step_on_every_ordering_and_tie() {
        let f = |x: f32| x.sqrt();
        let (lo, mid, hi) = (0.04f32, 0.25f32, 0.64f32);
        let (b, t) = (f(lo), f(hi));
        let m = b + (t - b) * (mid - lo) / (hi - lo);
        let close = |a: [f32; 3], w: [f32; 3]| a.iter().zip(w).all(|(a, w)| (a - w).abs() < 1e-6);
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let v = [lo, mid, hi];
            let want = [b, m, t];
            let c = order.map(|i| v[i]);
            let w = order.map(|i| want[i]);
            assert!(
                close(hue_preserving_tone(c, f), w),
                "{c:?}: {:?}",
                hue_preserving_tone(c, f)
            );
        }
        // Ties: two at the top, two at the bottom, all three.
        assert!(close(hue_preserving_tone([hi, hi, lo], f), [t, t, b]));
        assert!(close(hue_preserving_tone([lo, hi, lo], f), [b, t, b]));
        assert!(close(hue_preserving_tone([mid, mid, mid], f), [f(mid); 3]));
        assert!(close(hue_preserving_tone([0.0, 0.0, 0.0], f), [0.0; 3]));
    }
}
