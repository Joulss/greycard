//! A mask's Refine edge: its shapes composed into one raster on the
//! masks' grid, that raster's edge drawn onto the picture's by the
//! guided filter (`greycard_core::develop::refine_edge`), and the
//! result read in place of the shapes wherever the mask is.
//!
//! One raster made one way, at the masks' fixed width whatever the
//! picture's size, and read through the same bilinear on every path:
//! so the viewport and an export of any size read one refined mask at
//! one place, as they read a brush's raster. The edge is found on that
//! grid, 2048 texels across, about four pixels a texel at 45 MP; a
//! Sky's wide raster is read at it too.
//!
//! The picture it is made against is made only for an edit that
//! refines a mask ([`refines`]), always of the picture from before the
//! Detail section and the sharpen: the one the range shapes read while
//! that section does something, so a refined window takes in what an
//! unrefined one does, and a refined edge is the picture's own, with
//! no Dehaze or Clarity halo, whatever another mask holds.

use std::sync::Arc;

use greycard_core::develop::refine_edge::refine_edge_fast;
use greycard_core::image::WorkingImage;
use greycard_edit::Mask;
use greycard_edit::brush::{RASTER_WIDTH, Raster, raster_height};
use half::f16;
use rayon::prelude::*;

use crate::finish::{RasterRef, oklab};

/// The window's radius at a Refine edge of one, in texels of the
/// masks' grid ([`RASTER_WIDTH`] across): about one and a half
/// percent of the frame's width each way, 130 pixels on a 45 MP frame.
/// The amount runs the radius from [`LEAST_RADIUS`] up to it.
const MOST_RADIUS: f32 = 32.0;
/// The window's radius at the least amount that does anything: two
/// texels, about the softness a learned mask's edge has already.
const LEAST_RADIUS: f32 = 2.0;
/// The regularizer, in Oklab units squared, of the picture as shown
/// (its baseline and exposure on): a window whose picture varies by
/// less than about its square root in each channel, 0.03, reads as
/// flat and the mask is smoothed there; one that varies by more holds
/// the mask's edge on its own. A just noticeable difference in Oklab
/// is about 0.02, so texture at that level is not an edge and a
/// boundary the eye sees as one is.
const EPS: f32 = 9e-4;
/// The blocks the window's model is made on are about a quarter of
/// its radius across ([`refine_edge_fast`]), never under two texels:
/// at one, the full grid, the filter takes four times as long.
const RADIUS_PER_BLOCK: usize = 4;

/// The picture a refined mask is made against: the developed picture
/// before its sharpen and its geometry (in the masks' own positions),
/// averaged into the masks' grid, in Oklab at no exposure. Carried
/// beside the guide plane ([`crate::finish::Guide::picture`]), so it
/// goes wherever that goes.
#[derive(Clone, PartialEq)]
pub struct MaskPicture {
    pub width: usize,
    pub height: usize,
    /// The picture's height over its width.
    pub aspect: f32,
    /// Oklab L, a and b a texel, rows top down.
    pub lab: Vec<[f16; 3]>,
}

impl std::fmt::Debug for MaskPicture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaskPicture({}x{})", self.width, self.height)
    }
}

impl MaskPicture {
    /// None made: no mask is refined against it.
    pub const NONE: Self = Self {
        width: 0,
        height: 0,
        aspect: 1.0,
        lab: Vec::new(),
    };

    /// What it holds, in bytes.
    pub fn bytes(&self) -> u64 {
        (self.lab.len() * std::mem::size_of::<[f16; 3]>()) as u64
    }

    /// Whether it is the picture `aspect` (height over width) tall, so
    /// a mask placed on that picture can be refined against it: a
    /// develop still on the frame's last turn is not.
    pub fn fits(&self, aspect: f32) -> bool {
        !self.lab.is_empty() && (self.aspect - aspect).abs() < 1e-3
    }
}

/// The masks' grid for a picture `w` by `h`: [`RASTER_WIDTH`] across
/// and as tall as the picture is for it.
fn grid(w: usize, h: usize) -> (usize, usize) {
    (
        RASTER_WIDTH,
        raster_height(RASTER_WIDTH, h as f32 / w as f32),
    )
}

/// The source span of texel `i` of `n` over `len` pixels: the pixels
/// whose centers fall in it, at least the one under its center.
fn span(i: usize, n: usize, len: usize) -> (usize, usize) {
    let a = ((i * len) / n).min(len - 1);
    let b = (((i + 1) * len) / n).clamp(a + 1, len);
    (a, b)
}

/// One grid row's texels from `rows`, the picture's rows `y0..y1`
/// (RGB, `w` across), each texel the mean of its span, in Oklab.
fn grid_row(out: &mut [[f16; 3]], rows: &[f32], w: usize) {
    let n = rows.len() / (w * 3);
    let gw = out.len();
    for (gx, o) in out.iter_mut().enumerate() {
        let (x0, x1) = span(gx, gw, w);
        let mut acc = [0.0f32; 3];
        for y in 0..n {
            let row = &rows[(y * w + x0) * 3..(y * w + x1) * 3];
            for px in row.as_chunks::<3>().0 {
                acc[0] += px[0];
                acc[1] += px[1];
                acc[2] += px[2];
            }
        }
        let count = ((x1 - x0) * n) as f32;
        *o = oklab(acc.map(|v| v / count)).map(f16::from_f32);
    }
}

/// `image` averaged into the masks' grid, [`RASTER_WIDTH`] across and
/// as tall as the picture is for it: each texel the mean of the
/// pixels whose centers fall in it, or the pixel under its center
/// where the picture is smaller than the grid. Then to Oklab.
pub fn mask_picture(image: &WorkingImage) -> MaskPicture {
    let (w, h) = (image.width, image.height);
    if w == 0 || h == 0 {
        return MaskPicture::NONE;
    }
    let (gw, gh) = grid(w, h);
    let mut lab = vec![[f16::ZERO; 3]; gw * gh];
    lab.par_chunks_mut(gw).enumerate().for_each(|(gy, out)| {
        let (y0, y1) = span(gy, gh, h);
        grid_row(out, &image.data[y0 * w * 3..y1 * w * 3], w);
    });
    MaskPicture {
        width: gw,
        height: gh,
        aspect: h as f32 / w as f32,
        lab,
    }
}

/// Whether `edit` refines a mask: an enabled adjustment, with a shape,
/// whose Refine edge is on. Only then is a picture made to refine
/// against.
pub fn refines(edit: &greycard_edit::Edit) -> bool {
    edit.adjustments
        .iter()
        .take(crate::finish::MAX_LOCALS)
        .any(|a| a.enabled && a.mask.refine > 0.0 && !a.mask.is_empty())
}

/// The window's radius in texels for a Refine edge of `amount`, and
/// the blocks its model is made on: nothing at none.
fn radius(amount: f32) -> Option<(usize, usize)> {
    (amount > 0.0).then(|| {
        let r = (LEAST_RADIUS + (MOST_RADIUS - LEAST_RADIUS) * amount.min(1.0)).round() as usize;
        (r, (r / RADIUS_PER_BLOCK).max(2))
    })
}

/// `mask` refined against `picture`: its shapes composed at every
/// texel of the masks' grid as the finish composes them at a pixel
/// (its brushes and learned shapes from `rasters`, its range shapes
/// from the texel's own color at the global `exposure`), then the
/// guided filter at the mask's Refine edge, its regularizer brought to
/// the picture shown `stops` over the one made (the baseline and the
/// exposure: Oklab scales by the gain's cube root, its squares by the
/// gain to the two thirds). None when the mask is not refined, or the
/// picture is not this mask's (`MaskPicture::fits`).
pub fn refined(
    mask: &Mask,
    rasters: &[Option<RasterRef>],
    picture: &MaskPicture,
    aspect: f32,
    exposure: f32,
    stops: f32,
) -> Option<RasterRef> {
    let (r, s) = radius(mask.refine)?;
    if mask.is_empty() || !picture.fits(aspect) {
        return None;
    }
    let (w, h) = (picture.width, picture.height);
    let lab: Vec<[f32; 3]> = picture.lab.par_iter().map(|v| v.map(f16::to_f32)).collect();
    // The range shapes read a texel as `finish::sample` reads a pixel:
    // Oklab at the exposure, which scales all three by the gain's cube
    // root, from the texel's own color.
    let reads = mask.reads_picture();
    let gain = exposure.exp2().cbrt();
    let composed: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|k| {
            let (x, y) = (k % w, k / w);
            let u = (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32 * picture.aspect;
            let s = reads.then(|| {
                let [l, a, b] = lab[k].map(|c| c * gain);
                greycard_edit::mask::Sample { lightness: l, a, b }
            });
            let raster = |i: usize| rasters.get(i).and_then(|r| r.as_ref());
            mask.at_cut(
                u,
                v,
                s,
                |i, u, v| raster(i).map_or(0.0, |r| r.0.at(u, v)),
                |i, u, v| raster(i).map_or(0.0, |r| r.0.cut_at(u, v)),
            )
        })
        .collect();
    let eps = EPS * (-2.0 * stops / 3.0).exp2();
    let out = refine_edge_fast(&lab, &composed, w, h, r, eps, s);
    let data = out.iter().map(|v| (v * 255.0).round() as u8).collect();
    Some(RasterRef(Arc::new(Raster::from_data(
        picture.aspect,
        w,
        data,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{Format, Pixels, Settings, render};
    use crate::finish::{Source, guide_plane};
    use greycard_edit::mask::{Component, Shape};
    use greycard_edit::{Adjustment, Edit, Look, NoStores};

    /// A frame 600 by 400, dark green on the left, light red on the
    /// right, the boundary a ramp 4 pixels wide at x = 300.
    fn two_sides() -> WorkingImage {
        let (w, h) = (600usize, 400usize);
        let data = (0..w * h)
            .flat_map(|k| {
                let t = (((k % w) as f32 + 0.5 - 298.0) / 4.0).clamp(0.0, 1.0);
                let dark = [0.03, 0.06, 0.02];
                let light = [0.6, 0.25, 0.2];
                [0, 1, 2].map(|c| dark[c] + (light[c] - dark[c]) * t)
            })
            .collect();
        WorkingImage {
            width: w,
            height: h,
            data,
        }
    }

    /// A mask whose edge is hard, vertical at `u`, everything right of it.
    fn right_of(u: f32, refine: f32) -> Mask {
        Mask {
            components: vec![Component {
                shape: Shape::Linear {
                    from: [u + 1e-4, 0.3],
                    to: [u - 1e-4, 0.3],
                },
                ..Default::default()
            }],
            refine,
            ..Default::default()
        }
    }

    /// The refined mask's half-way point along the middle row, in the
    /// frame's width.
    fn half_way(r: &RasterRef) -> f32 {
        let row = r.0.height / 2;
        let at = (0..r.0.width)
            .find(|&x| r.0.data()[row * r.0.width + x] >= 128)
            .unwrap();
        (at as f32 + 0.5) / r.0.width as f32
    }

    /// The picture on the masks' grid is the picture: each side's
    /// color where it is, at the grid's own width.
    #[test]
    fn the_mask_picture_is_the_picture_on_the_masks_grid() {
        let p = mask_picture(&two_sides());
        assert_eq!((p.width, p.height), (RASTER_WIDTH, 1365));
        let at = |x: usize| p.lab[600 * p.width + x].map(f16::to_f32);
        let dark = oklab([0.03, 0.06, 0.02]);
        let light = oklab([0.6, 0.25, 0.2]);
        for c in 0..3 {
            assert!((at(10)[c] - dark[c]).abs() < 2e-3, "{:?}", at(10));
            assert!((at(2040)[c] - light[c]).abs() < 2e-3, "{:?}", at(2040));
        }
    }

    /// A hard edge drawn a little short of the picture's boundary, 20
    /// texels of the grid, is drawn toward it, the further the larger
    /// the Refine edge: at the most, over half the way.
    #[test]
    fn a_hard_edge_moves_toward_the_pictures_boundary() {
        let picture = mask_picture(&two_sides());
        let aspect = 400.0 / 600.0;
        let edge = |refine: f32| {
            refined(&right_of(0.49, refine), &[None], &picture, aspect, 0.0, 0.0)
                .map(|r| half_way(&r))
        };
        assert_eq!(edge(0.0), None, "none at nothing");
        let at = [0.2, 0.6, 1.0].map(|a| edge(a).unwrap());
        assert!(at[0] >= 0.4895 && at[1] > at[0] && at[2] > at[1], "{at:?}");
        assert!(at[2] > 0.495 && at[2] < 0.5, "{at:?}");
        // Not this picture's mask: a frame of another shape.
        assert!(refined(&right_of(0.49, 0.5), &[None], &picture, 1.5, 0.0, 0.0).is_none());
    }

    /// The regularizer follows the picture as shown: refined against a
    /// picture three stops over (Oklab twice as large, the cube root
    /// of eight) is the same mask as against the one made, shown three
    /// stops over.
    #[test]
    fn the_regularizer_follows_the_exposure() {
        let picture = mask_picture(&two_sides());
        let brighter = MaskPicture {
            lab: picture
                .lab
                .iter()
                .map(|v| v.map(|c| c * f16::from_f32(2.0)))
                .collect(),
            ..picture.clone()
        };
        let aspect = 400.0 / 600.0;
        let mask = right_of(0.49, 0.6);
        let shown = refined(&mask, &[None], &picture, aspect, 0.0, 3.0).unwrap();
        let made = refined(&mask, &[None], &brighter, aspect, 0.0, 0.0).unwrap();
        let unshown = refined(&mask, &[None], &picture, aspect, 0.0, 0.0).unwrap();
        let worst = |a: &RasterRef, b: &RasterRef| {
            a.0.data()
                .iter()
                .zip(b.0.data())
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap()
        };
        assert!(worst(&shown, &made) <= 1, "{}", worst(&shown, &made));
        assert!(worst(&shown, &unshown) > 1, "the exposure moved nothing");
    }

    /// At the default the export is byte for byte what it was: a mask
    /// at Refine edge 0 makes the same file whether there is a picture
    /// to refine against or not, and one turned up makes another.
    #[test]
    fn the_default_leaves_the_export_byte_identical() {
        let image = two_sides();
        let mut edit = Edit::default();
        let mut look = Look::default();
        look.light.exposure = 1.0;
        edit.adjustments.push(Adjustment {
            id: 1,
            mask: right_of(0.49, 0.0),
            look,
            ..Default::default()
        });
        let settings = Settings {
            format: Format::Tiff,
            ..Settings::default()
        };
        let without = guide_plane(&image);
        let with = without.with_picture(Some(Arc::new(mask_picture(&image))));
        let pixels = |edit: &Edit, guide: &crate::finish::Guide| {
            let r = render(
                &image,
                edit,
                &NoStores,
                (600, 400),
                &settings,
                &Default::default(),
                f32::INFINITY,
                Some(guide),
                Source::SCENE,
                None,
                None,
            );
            let Pixels::Sixteen(p) = r.pixels else {
                unreachable!()
            };
            p
        };
        let plain = pixels(&edit, &with);
        assert!(plain == pixels(&edit, &without));
        edit.adjustments[0].mask.refine = 0.5;
        assert!(plain != pixels(&edit, &with));
        assert!(plain == pixels(&edit, &without), "no picture, no refine");
    }

    /// The finish reads a refined mask in place of its shapes: its
    /// invert and its range shapes are in the raster, not read again.
    #[test]
    fn the_finish_reads_the_refined_raster() {
        let image = two_sides();
        let picture = mask_picture(&image);
        let mut a = Adjustment {
            id: 1,
            mask: right_of(0.49, 0.4),
            ..Default::default()
        };
        a.mask.invert = true;
        let mut local = crate::finish::Local::of(&a, vec![None], None);
        local.refine(&picture, 400.0 / 600.0, 0.0, 0.0);
        let r = local.refined.clone().expect("refined");
        for (u, v) in [(0.1, 0.2), (0.49, 0.33), (0.9, 0.6)] {
            assert_eq!(local.weight_sampled(u, v, None), r.0.at(u, v));
        }
        assert!(local.weight_sampled(0.1, 0.3, None) > 0.99, "inverted");
        assert!(local.weight_sampled(0.9, 0.3, None) < 0.01, "inverted");
    }
}
