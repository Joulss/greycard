//! The grey axis of a fitted look: what it does to the 33 greys from
//! black to white. A camera's style renders its greys alike from dark
//! to light: neutral, or with one cast throughout, and lightness
//! rising. A fit whose cast changes along the axis (neutral in the
//! darks and magenta from mid-grey up, say), or whose lightness folds
//! back on itself, has carried what its data said about some other
//! color onto greys it never saw, and its table is not written.

use crate::color::{decode3, oklab};

/// Steps of the ramp, black and white among them: the table's own
/// nodes on its diagonal.
pub const STEPS: usize = 33;

/// The most a written table's cast may change along the grey axis:
/// the range of a, and of b, in Oklab over the steps between black
/// and white, once the one cast in proportion to lightness that fits
/// them best is taken off. A channel gain on the greys, the camera's
/// own cast, scores nothing. The user's 26 tables measure 0.0016 to
/// 0.0136; one shoot under warm light, fitted without the hold, 0.018
/// per channel and 0.026 under AgX.
pub const DRIFT_MAX: f32 = 0.015;

/// The largest fall in Oklab lightness from one step of the ramp to
/// the next that a written table may have. Tables of the current fit
/// fall by none.
pub const FALL_MAX: f32 = 0.005;

/// What a look does to the grey ramp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreyAxis {
    /// The cast that fits the steps best, as a and b per unit of
    /// lightness: what the look does to every grey alike.
    pub cast: [f32; 2],
    /// The larger of the ranges of a and of b over the steps between
    /// black and white with [`GreyAxis::cast`] taken off: how much the
    /// cast changes along the axis.
    pub drift: f32,
    /// The largest fall in output lightness from one step to the next;
    /// zero when lightness only rises.
    pub fall: f32,
    /// The grey, from 0 to 1, where that fall starts.
    pub fall_at: f32,
}

impl GreyAxis {
    /// Whether a table with this axis may be written.
    pub fn holds(&self) -> bool {
        self.drift <= DRIFT_MAX && self.fall <= FALL_MAX
    }
}

/// The grey ramp through `look` (encoded in, encoded out). Black and
/// white are left out of the cast and its drift: the fit pins white,
/// and black has no chroma to show.
pub fn grey_axis(look: impl Fn([f32; 3]) -> [f32; 3]) -> GreyAxis {
    let last = (STEPS - 1) as f32;
    let lab: Vec<[f32; 3]> = (0..STEPS)
        .map(|k| oklab(decode3(look([k as f32 / last; 3]))))
        .collect();
    let mut fall = 0.0f32;
    let mut fall_at = 0.0f32;
    for (k, w) in lab.windows(2).enumerate() {
        if w[0][0] - w[1][0] > fall {
            fall = w[0][0] - w[1][0];
            fall_at = k as f32 / last;
        }
    }
    let inner = &lab[1..STEPS - 1];
    let ll: f32 = inner.iter().map(|p| p[0] * p[0]).sum();
    let cast = if ll > 0.0 {
        [1, 2].map(|c| inner.iter().map(|p| p[0] * p[c]).sum::<f32>() / ll)
    } else {
        [0.0; 2]
    };
    let drift = [1, 2]
        .map(|c| {
            let left = inner.iter().map(|p| p[c] - cast[c - 1] * p[0]);
            let (lo, hi) = left.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
            hi - lo
        })
        .into_iter()
        .fold(0.0f32, f32::max);
    GreyAxis {
        cast,
        drift,
        fall,
        fall_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::encode3;

    #[test]
    fn identity_a_tone_curve_and_a_cast_hold() {
        let id = grey_axis(|x| x);
        assert!(id.drift < 1e-4, "{id:?}");
        assert_eq!(id.fall, 0.0);
        assert!(id.holds());
        // A contrast curve: a style's greys.
        let styled = grey_axis(|x| x.map(|v| v * v * (3.0 - 2.0 * v)));
        assert!(styled.holds(), "{styled:?}");
        // A camera's warm cast, a gain on red and blue: one cast along
        // the whole axis, so no drift, and the cast is read.
        let warm = grey_axis(|x| {
            let l = decode3(x);
            encode3([l[0] * 1.06, l[1], l[2] * 0.92].map(|v| v.min(1.0)))
        });
        assert!(warm.drift < 0.002, "{warm:?}");
        assert!(warm.cast[1] > 0.005, "{warm:?}");
        assert!(warm.holds());
    }

    /// The bad table's shape: greys from the middle up pushed magenta,
    /// neutral below.
    #[test]
    fn a_tint_that_changes_along_the_axis_and_a_fold_are_refused() {
        let magenta = |x: [f32; 3]| {
            let g = x[1];
            let t = ((g - 0.5) / 0.25).clamp(0.0, 1.0);
            [g + 0.05 * t, g - 0.15 * t, g + 0.01 * t].map(|v| v.clamp(0.0, 1.0))
        };
        let axis = grey_axis(magenta);
        assert!(axis.drift > 0.05, "{axis:?}");
        assert!(!axis.holds());
        // Lightness turned back on itself, the channels together.
        let folded = grey_axis(|x| x.map(|v| if v > 0.8 { 0.8 - 0.5 * (v - 0.8) } else { v }));
        assert!(folded.drift < 1e-3, "{folded:?}");
        assert!(folded.fall > FALL_MAX, "{folded:?}");
        assert!(folded.fall_at > 0.8, "{folded:?}");
        assert!(!folded.holds());
    }
}
