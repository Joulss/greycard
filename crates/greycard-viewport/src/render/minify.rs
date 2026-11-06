//! The view below 1:1. A target pixel there covers more than one
//! source pixel, and one tap of the source at its center aliases fine
//! detail (foliage, branches, fabric) that the export's resize
//! averages away. The shader averages a square of bilinear taps
//! spread evenly over the pixel's footprint instead, of the picture,
//! the range masks' picture, and the sharpen's and the Detail's
//! planes: all of them per source pixel, and all of them resized with
//! the picture on the export's side. The cap and the count are the
//! window's; the rest is the shader's reference, for the tests.

#[cfg(test)]
use greycard_core::image::WorkingImage;

/// The most taps a side. At a fit of a 45 MP frame into a view 2000
/// pixels wide, a zoom of 0.24, the footprint is 4.1 source pixels a
/// side: four taps a side land just over a source pixel apart, each a
/// bilinear of the four about it, so every source pixel is read. A
/// fifth leaves 0.04 less of a grating's swing at the worst frequency
/// (`what_the_cap_costs`) and costs a lavapipe frame a fifth more.
/// Further out, between whole ratios, the taps leave gaps and some
/// aliasing comes back, never more than one tap's.
pub const MOST_TAPS: u32 = 4;

/// Taps a side over a target pixel at `zoom` display pixels per
/// source pixel: one at 1:1 and above, which is the single tap at the
/// pixel's center the view has always taken; below it as many as the
/// footprint is source pixels across, rounded up, at most
/// [`MOST_TAPS`].
#[cfg(test)]
pub fn taps(zoom: f32) -> u32 {
    taps_at_most(zoom, MOST_TAPS)
}

/// [`taps`] with another cap, for measuring one.
pub fn taps_at_most(zoom: f32, most: u32) -> u32 {
    if zoom >= 1.0 || zoom.is_nan() || zoom <= 0.0 {
        return 1;
    }
    ((1.0 / zoom).ceil() as u32).clamp(1, most.max(1))
}

/// Where the taps sit in a target pixel, `n` a side: target pixels
/// from its center, evenly spread, a tap at the middle of each of
/// `n` by `n` equal cells.
#[cfg(test)]
pub fn offsets(n: u32) -> impl Iterator<Item = (f32, f32)> {
    let n = n.max(1);
    let at = move |i: u32| (i as f32 + 0.5) / n as f32 - 0.5;
    (0..n).flat_map(move |j| (0..n).map(move |i| (at(i), at(j))))
}

/// `image` at a source position by the bilinear the sampler gives:
/// pixel centers at halves, the edges clamped.
#[cfg(test)]
pub fn bilinear(image: &WorkingImage, (x, y): (f32, f32)) -> [f32; 3] {
    let (w, h) = (image.width as i64, image.height as i64);
    let (gx, gy) = (x - 0.5, y - 0.5);
    let (lx, ly) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - lx, gy - ly);
    let at = |i: i64, j: i64| image.pixel(i.clamp(0, w - 1) as usize, j.clamp(0, h - 1) as usize);
    let (x0, y0) = (lx as i64, ly as i64);
    let (t00, t10, t01, t11) = (
        at(x0, y0),
        at(x0 + 1, y0),
        at(x0, y0 + 1),
        at(x0 + 1, y0 + 1),
    );
    std::array::from_fn(|c| {
        let top = t00[c] + (t10[c] - t00[c]) * fx;
        let bottom = t01[c] + (t11[c] - t01[c]) * fx;
        top + (bottom - top) * fy
    })
}

/// The mean of `image` over the target pixel centered at `at`: `n` by
/// `n` taps at [`offsets`], each taken to the source by `to_source`
/// (the view's map from a target position to a source one) and read
/// by [`bilinear`]. One tap is the bilinear at the center.
#[cfg(test)]
pub fn footprint_mean(
    image: &WorkingImage,
    at: (f32, f32),
    n: u32,
    to_source: impl Fn((f32, f32)) -> (f32, f32),
) -> [f32; 3] {
    let mut acc = [0.0f32; 3];
    let mut count = 0.0f32;
    for (dx, dy) in offsets(n) {
        let t = bilinear(image, to_source((at.0 + dx, at.1 + dy)));
        for c in 0..3 {
            acc[c] += t[c];
        }
        count += 1.0;
    }
    acc.map(|v| v / count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tap_at_1_to_1_and_above_and_the_footprint_below() {
        assert_eq!(taps(1.0), 1);
        assert_eq!(taps(2.5), 1);
        assert_eq!(taps(0.0), 1);
        assert_eq!(taps(f32::NAN), 1);
        assert_eq!(taps(0.75), 2);
        assert_eq!(taps(0.5), 2);
        assert_eq!(taps(0.34), 3);
        assert_eq!(taps(0.25), 4);
        assert_eq!(taps(0.2422), MOST_TAPS);
        assert_eq!(taps(0.05), MOST_TAPS);
        assert_eq!(taps_at_most(0.2422, 8), 5);
    }

    #[test]
    fn the_taps_split_the_pixel_into_equal_cells() {
        let two: Vec<_> = offsets(2).collect();
        assert_eq!(
            two,
            vec![(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)]
        );
        assert_eq!(offsets(1).collect::<Vec<_>>(), vec![(0.0, 0.0)]);
    }

    /// A one-pixel checkerboard at a quarter is its flat mean; at one
    /// tap it is not. The view sits a quarter of a source pixel off
    /// the board's corners: on them each bilinear tap would read a 2x2
    /// mean by itself, and taps in the wrong places would pass.
    #[test]
    fn a_checkerboard_at_a_quarter_is_its_mean() {
        let (w, h) = (64usize, 48usize);
        let image = WorkingImage {
            width: w,
            height: h,
            data: (0..w * h)
                .map(|i| (i % w + i / w).is_multiple_of(2))
                .flat_map(|even| [if even { 0.1f32 } else { 0.7 }; 3])
                .collect(),
        };
        let zoom = 0.25;
        let to_source = |(x, y): (f32, f32)| (x / zoom + 0.25, y / zoom + 0.25);
        for y in 0..(h as f32 * zoom) as usize - 1 {
            for x in 0..(w as f32 * zoom) as usize - 1 {
                let at = (x as f32 + 0.5, y as f32 + 0.5);
                let m = footprint_mean(&image, at, taps(zoom), to_source);
                assert!((m[0] - 0.4).abs() < 1e-5, "{x},{y}: {m:?}");
                let one = footprint_mean(&image, at, 1, to_source);
                assert!((one[0] - 0.4).abs() > 0.05, "{x},{y}: {one:?}");
            }
        }
    }

    /// What is left of gratings past the view's limit at `zoom`
    /// through each of `counts` taps a side, the worst over four
    /// phases: in two bands, just past the limit up to twice it, and
    /// from there to the source's own limit.
    fn grating_swings(zoom: f32, counts: &[u32]) -> (Vec<f32>, Vec<f32>) {
        let width = 1024usize;
        let mut near = vec![0.0f32; counts.len()];
        let mut far = vec![0.0f32; counts.len()];
        let mut f = zoom / 2.0 + 0.001;
        while f <= 0.5 {
            for phase in [0.0f32, 0.25, 0.5, 0.75] {
                let image = WorkingImage {
                    width,
                    height: 1,
                    data: (0..width)
                        .flat_map(|x| {
                            let v =
                                0.5 + 0.5 * (std::f32::consts::TAU * (f * x as f32 + phase)).cos();
                            [v; 3]
                        })
                        .collect(),
                };
                for (k, &n) in counts.iter().enumerate() {
                    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                    for x in 4..(width as f32 * zoom) as usize - 4 {
                        let at = (x as f32 + 0.5, 0.5);
                        let m = footprint_mean(&image, at, n, |(x, _)| (x / zoom, 0.5));
                        lo = lo.min(m[0]);
                        hi = hi.max(m[0]);
                    }
                    let band = if f < zoom { &mut near } else { &mut far };
                    band[k] = band[k].max(hi - lo);
                }
            }
            f += 0.002;
        }
        (near, far)
    }

    /// What the cap costs. A perfect resize shows a grating past the
    /// view's limit flat, so the swing left of one is aliasing, with
    /// the box's own leak just past the limit, which no count of taps
    /// removes. At the fit that matters most, a 45 MP frame drawn 2000
    /// pixels wide, held to the cap costing at most five hundredths of
    /// a swing over the uncapped count in either band, and to leaving
    /// under a third of what one tap leaves past twice the limit; at
    /// an eighth, a compare view's tile, printed. For the notes.
    #[test]
    #[ignore = "a measurement"]
    fn what_the_cap_costs() {
        let counts = [1u32, 2, 3, 4, 5, 6, 8, 12];
        for zoom in [2000.0 / 8256.0f32, 0.125] {
            let uncapped = taps_at_most(zoom, 64);
            let (near, far) = grating_swings(zoom, &counts);
            for (k, n) in counts.iter().enumerate() {
                eprintln!(
                    "{n} taps a side at {zoom:.4}{}: what is left of a grating's swing at worst, \
                     {:.3} just past the view's limit, {:.3} past twice it",
                    if *n == uncapped { " (uncapped)" } else { "" },
                    near[k],
                    far[k]
                );
            }
            if zoom < 0.2 {
                continue;
            }
            let at = |n: u32| counts.iter().position(|c| *c == n).unwrap();
            let (cap, free) = (at(MOST_TAPS), at(uncapped));
            assert!(
                near[cap] <= near[free] + 0.05,
                "{} against {}",
                near[cap],
                near[free]
            );
            assert!(
                far[cap] <= far[free] + 0.05,
                "{} against {}",
                far[cap],
                far[free]
            );
            assert!(
                far[cap] < far[0] / 3.0,
                "{} against one tap's {}",
                far[cap],
                far[0]
            );
        }
    }
}
