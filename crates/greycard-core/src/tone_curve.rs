//! A camera profile's tone curve: the `ProfileToneCurve` a DCP may
//! carry, a list of (input, output) points over 0..1 that Adobe's
//! renders tone the picture with. The DNG specification (1.7.1,
//! `ProfileToneCurve`) says readers should interpolate between the
//! points with a cubic spline; the method is Adobe's, and this is ours,
//! written from the specification.
//!
//! This product includes DNG technology under license by Adobe.
//!
//! The spline is the natural cubic spline through the points: one cubic
//! between each pair, the whole twice continuously differentiable, its
//! second derivative zero at both ends. The curve's second derivatives
//! at the points come from the tridiagonal system that continuity
//! gives, solved by elimination down the diagonal and substitution back
//! up (the natural cubic spline as Burden and Faires, *Numerical
//! Analysis*, set it out). Before the first point and past the last the
//! curve holds the end point's value.
//!
//! The spline is sampled once into a table that the CPU and the shader
//! read by linear interpolation, so the two read the same numbers.
//! [`TABLE_STEPS`] says how fine and why.
//!
//! A curve is used only when it is one: at least two points, each a
//! finite number in 0..1, the inputs strictly rising, and no more than
//! [`MAX_POINTS`] of them. Nothing asks for (0, 0) or (1, 1). Points
//! bunched close can make a cubic swing past 0..1 between them; the
//! table is held to 0..1, the range a display value has, and a table
//! that is not all finite numbers (spacing near the floats' limit) is
//! no curve.

/// Steps of the table over 0..1, with a sample at each end
/// (`TABLE_STEPS + 1` samples). Linear interpolation between samples
/// a step `h` apart is off a smooth curve by at most `h² / 8` times its
/// second derivative: at 4096 steps that is under 1e-8 for a slope that
/// changes by one per unit, and under a 16-bit level for any curve a
/// profile draws, so the table is as good as the spline wherever a
/// picture lands. A power of two keeps `x · TABLE_STEPS` exact for an
/// input on the grid, and the table is 16 KB for the shader.
pub const TABLE_STEPS: usize = 4096;

/// The most points a curve may have: a profile's curve has tens to a
/// few hundred; more is a damaged or hostile file, read as no curve so
/// a count in the file allocates nothing.
pub const MAX_POINTS: usize = 65_536;

/// A profile's tone curve: its points and the table sampled from the
/// spline through them.
#[derive(Debug, Clone, PartialEq)]
pub struct ToneCurve {
    points: Vec<[f32; 2]>,
    /// The curve at `i / TABLE_STEPS`, `TABLE_STEPS + 1` samples, each
    /// in 0..1.
    table: Vec<f32>,
    /// The first input at which the table reaches its largest value,
    /// and that value.
    white: [f32; 2],
}

impl ToneCurve {
    /// The curve through `points`, or `None` when they are not one (see
    /// the module).
    pub fn new(points: &[[f32; 2]]) -> Option<ToneCurve> {
        let usable = (2..=MAX_POINTS).contains(&points.len())
            && points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && points.windows(2).all(|w| w[0][0] < w[1][0]);
        if !usable {
            return None;
        }
        let spline = Spline::through(points);
        let mut table = Vec::with_capacity(TABLE_STEPS + 1);
        for i in 0..=TABLE_STEPS {
            let y = spline.at(i as f64 / TABLE_STEPS as f64);
            if !y.is_finite() {
                return None;
            }
            table.push(y.clamp(0.0, 1.0) as f32);
        }
        // The first sample at the largest value: where the curve's
        // white is.
        let (at, top) =
            table.iter().enumerate().fold(
                (0, f32::MIN),
                |(at, top), (i, &v)| {
                    if v > top { (i, v) } else { (at, top) }
                },
            );
        Some(ToneCurve {
            points: points.to_vec(),
            table,
            white: [at as f32 / TABLE_STEPS as f32, top],
        })
    }

    /// Where the curve first reaches its largest output, and that
    /// output: the input the picture's white is at under this curve.
    pub fn white(&self) -> [f32; 2] {
        self.white
    }

    /// The points the curve was made from.
    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }

    /// The table: the curve at `i / TABLE_STEPS` for `i` in
    /// `0..=TABLE_STEPS`, what the shader reads.
    pub fn table(&self) -> &[f32] {
        &self.table
    }

    /// The curve at `x`, held to 0..1 (a NaN is the bottom), read from
    /// the table by linear interpolation.
    pub fn at(&self, x: f32) -> f32 {
        let x = if x > 0.0 { x.min(1.0) } else { 0.0 };
        let y = x * TABLE_STEPS as f32;
        let i = (y as usize).min(TABLE_STEPS - 1);
        let f = y - i as f32;
        self.table[i] * (1.0 - f) + self.table[i + 1] * f
    }
}

/// A natural cubic spline: the points and the second derivative there.
struct Spline {
    x: Vec<f64>,
    y: Vec<f64>,
    m: Vec<f64>,
}

impl Spline {
    /// The natural cubic spline through `points`, whose inputs rise.
    ///
    /// Between points `i` and `i + 1`, `h_i` apart, continuity of the
    /// slope gives for each inner point
    /// `h_{i-1} m_{i-1} + 2 (h_{i-1} + h_i) m_i + h_i m_{i+1} = 6 (d_i - d_{i-1})`,
    /// `d_i` the chord's slope, with `m` zero at both ends: a
    /// tridiagonal system, diagonally dominant, solved without pivots.
    fn through(points: &[[f32; 2]]) -> Spline {
        let x: Vec<f64> = points.iter().map(|p| f64::from(p[0])).collect();
        let y: Vec<f64> = points.iter().map(|p| f64::from(p[1])).collect();
        let n = x.len();
        let mut m = vec![0.0; n];
        if n > 2 {
            let h: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
            let d: Vec<f64> = (0..n - 1).map(|i| (y[i + 1] - y[i]) / h[i]).collect();
            // The inner rows, 1..n-1: below, on, and above the diagonal,
            // and the right side, eliminated downward as they are made.
            let inner = n - 2;
            let mut on = vec![0.0; inner];
            let mut rhs = vec![0.0; inner];
            for k in 0..inner {
                let i = k + 1;
                on[k] = 2.0 * (h[i - 1] + h[i]);
                rhs[k] = 6.0 * (d[i] - d[i - 1]);
                if k > 0 {
                    // The row above, scaled to clear this row's left.
                    let w = h[i - 1] / on[k - 1];
                    on[k] -= w * h[i - 1];
                    rhs[k] -= w * rhs[k - 1];
                }
            }
            // Back up from the last inner row.
            for k in (0..inner).rev() {
                let i = k + 1;
                let next = if k + 1 < inner { m[i + 1] } else { 0.0 };
                m[i] = (rhs[k] - h[i] * next) / on[k];
            }
        }
        Spline { x, y, m }
    }

    /// The spline at `t`: the end point's value outside the points.
    fn at(&self, t: f64) -> f64 {
        let n = self.x.len();
        if t <= self.x[0] {
            return self.y[0];
        }
        if t >= self.x[n - 1] {
            return self.y[n - 1];
        }
        // The piece holding `t`: the last point at or under it.
        let i = self
            .x
            .partition_point(|&x| x <= t)
            .saturating_sub(1)
            .min(n - 2);
        let h = self.x[i + 1] - self.x[i];
        let (a, b) = (self.x[i + 1] - t, t - self.x[i]);
        (self.m[i] * a * a * a + self.m[i + 1] * b * b * b) / (6.0 * h)
            + (self.y[i] / h - self.m[i] * h / 6.0) * a
            + (self.y[i + 1] / h - self.m[i + 1] * h / 6.0) * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The natural spline through (0, 0), (0.5, 0.75), (1, 1), worked by
    /// hand: one inner point, `h = 0.5` both sides, chords of slope 1.5
    /// and 0.5, so `2 (0.5 + 0.5) m = 6 (0.5 - 1.5)` and `m = -3` in the
    /// middle. On the first piece the curve is then
    /// `-3 b³ / 3 + (0 - 0) a + (1.5 + 0.25) b`, `b = x`, `a = 0.5 - x`:
    /// `S(x) = 1.75 x - x³`, so `S(0.25) = 0.421875`; on the second,
    /// `a = 1 - x`, `b = x - 0.5`, it is `-a³ + 1.75 a + 2 b`, so
    /// `S(0.75) = 0.921875`.
    #[test]
    fn the_spline_is_the_natural_one_worked_by_hand() {
        let s = Spline::through(&[[0.0, 0.0], [0.5, 0.75], [1.0, 1.0]]);
        assert_eq!(s.m, vec![0.0, -3.0, 0.0]);
        let want = |x: f64| {
            if x <= 0.5 {
                1.75 * x - x * x * x
            } else {
                // The second piece: `b = x - 0.5`, `a = 1 - x`.
                let (a, b) = (1.0 - x, x - 0.5);
                -a * a * a + 1.75 * a + 2.0 * b
            }
        };
        assert!((s.at(0.25) - 0.421875).abs() < 1e-12);
        assert!((s.at(0.75) - 0.921875).abs() < 1e-12);
        for i in 0..=100 {
            let x = i as f64 / 100.0;
            assert!((s.at(x) - want(x)).abs() < 1e-12, "{x}");
        }
        // Four points, two inner: the system by hand. Points at 0, 1/3,
        // 2/3, 1 with outputs 0, 0.5, 0.8, 1: chords 1.5, 0.9, 0.6, so
        // `(4/3) m1 + (1/3) m2 = 6 (0.9 - 1.5) = -3.6` and
        // `(1/3) m1 + (4/3) m2 = 6 (0.6 - 0.9) = -1.8`: `m1 = -2.52`,
        // `m2 = -0.72`.
        let third = 1.0f32 / 3.0;
        let s = Spline::through(&[[0.0, 0.0], [third, 0.5], [2.0 * third, 0.8], [1.0, 1.0]]);
        assert!((s.m[1] + 2.52).abs() < 1e-5, "{:?}", s.m);
        assert!((s.m[2] + 0.72).abs() < 1e-5, "{:?}", s.m);
        // Through every point, with its slope continuous across each.
        for (i, p) in [[0.0, 0.0], [third, 0.5], [2.0 * third, 0.8], [1.0, 1.0]]
            .iter()
            .enumerate()
        {
            assert!(
                (s.at(f64::from(p[0])) - f64::from(p[1])).abs() < 1e-6,
                "{i}"
            );
        }
        let slope = |x: f64, e: f64| (s.at(x + e) - s.at(x - e)) / (2.0 * e);
        for x in [f64::from(third), f64::from(2.0 * third)] {
            assert!((slope(x - 1e-4, 1e-6) - slope(x + 1e-4, 1e-6)).abs() < 1e-3);
        }
    }

    /// Two points are the line between them; outside the points the
    /// curve holds the end's value; the table runs over 0..1 end to
    /// end and `at` reads it between samples.
    #[test]
    fn two_points_are_a_line_and_the_ends_hold() {
        let c = ToneCurve::new(&[[0.25, 0.1], [0.75, 0.9]]).unwrap();
        assert_eq!(c.table().len(), TABLE_STEPS + 1);
        assert_eq!(c.at(0.0), 0.1);
        assert_eq!(c.at(1.0), 0.9);
        assert!((c.at(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(c.at(f32::NAN), 0.1, "a NaN is the bottom");
        assert_eq!(c.at(7.0), 0.9);
        assert_eq!(c.white(), [0.75, 0.9]);
    }

    /// A curve that is not one is no curve: too few or too many points,
    /// a point out of 0..1 or not a number, inputs not rising.
    #[test]
    fn what_is_not_a_curve_is_none() {
        assert!(ToneCurve::new(&[[0.0, 0.0]]).is_none());
        assert!(ToneCurve::new(&[[0.0, 0.0], [1.0, 1.1]]).is_none());
        assert!(ToneCurve::new(&[[0.0, f32::NAN], [1.0, 1.0]]).is_none());
        assert!(ToneCurve::new(&[[0.5, 0.0], [0.5, 1.0]]).is_none());
        assert!(ToneCurve::new(&[[0.6, 0.0], [0.5, 1.0]]).is_none());
        let many: Vec<[f32; 2]> = (0..=MAX_POINTS)
            .map(|i| {
                let v = i as f32 / MAX_POINTS as f32;
                [v, v]
            })
            .collect();
        assert!(ToneCurve::new(&many).is_none(), "one over the most");
        assert!(ToneCurve::new(&many[..MAX_POINTS]).is_some());
    }

    /// Points bunched close swing the spline past 0..1; the table is
    /// held there, and a curve whose points sit at the floats' limit
    /// is either held there too or no curve, never a NaN.
    #[test]
    fn a_bunched_curve_is_held_to_its_range() {
        let c = ToneCurve::new(&[[0.0, 0.0], [0.01, 0.3], [0.02, 0.31], [1.0, 1.0]]).unwrap();
        assert!(c.table().iter().all(|v| (0.0..=1.0).contains(v)));
        let tiny = f32::from_bits(1);
        if let Some(c) = ToneCurve::new(&[[0.0, 0.0], [tiny, 1.0], [2.0 * tiny, 0.0], [1.0, 1.0]]) {
            assert!(c.table().iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    /// The white is where the curve first reaches its top: the end for
    /// a curve that rises to (1, 1), sooner for one that tops out early,
    /// and the end's value for one that never reaches one.
    #[test]
    fn the_white_is_where_the_curve_first_tops_out() {
        let c = ToneCurve::new(&[[0.0, 0.0], [0.5, 0.7], [1.0, 1.0]]).unwrap();
        assert_eq!(c.white(), [1.0, 1.0]);
        let early = ToneCurve::new(&[[0.0, 0.0], [0.85, 1.0], [1.0, 1.0]]).unwrap();
        assert!(
            early.white()[1] == 1.0 && early.white()[0] <= 0.86,
            "{:?}",
            early.white()
        );
        let dim = ToneCurve::new(&[[0.0, 0.0], [1.0, 0.97]]).unwrap();
        assert_eq!(dim.white(), [1.0, 0.97]);
    }
}
