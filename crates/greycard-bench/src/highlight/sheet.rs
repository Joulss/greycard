//! Picture sheets: the truth, the clipped input and every method, all
//! through one tone mapping, as a small full frame and 1:1 crops of the
//! clipped regions, labeled on an HTML page.
//!
//! The mapping, the same for every panel of a frame: an exposure that
//! puts the median luminance of the truth's clipped pixels at 1; then
//! Reinhard on luminance, `y / (1 + y)`, the pixel scaled to it so its
//! hue is kept (that median lands at 0.5, and a highlight four times
//! brighter at 0.8); then Rec.2020 to sRGB, clamped, sRGB's curve.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::RgbImage;

use greycard_core::RgbSpace;
use greycard_core::color::{CAT, WORKING_SPACE, srgb_encode};

use super::clipmap::{ClipMap, SizeBin};
use super::score::luma;

/// The width of a full frame panel.
pub const THUMB_WIDTH: usize = 480;
/// A crop's side, in pixels at 1:1.
pub const CROP: usize = 200;

/// The exposure of a frame's sheet: the reciprocal of the truth's median
/// luminance where the input was clipped.
pub fn exposure(map: &ClipMap, truth: &[f32]) -> f32 {
    let mut y: Vec<f32> = (0..map.regime.len())
        .filter(|&i| (1..=3).contains(&map.regime[i]))
        .step_by(7)
        .map(|i| luma([truth[3 * i], truth[3 * i + 1], truth[3 * i + 2]]))
        .collect();
    if y.is_empty() {
        return 1.0;
    }
    let k = y.len() / 2;
    let m = *y.select_nth_unstable_by(k, f32::total_cmp).1;
    if m > 0.0 { 1.0 / m } else { 1.0 }
}

struct Tone {
    exposure: f32,
    to_srgb: [[f32; 3]; 3],
}

impl Tone {
    fn new(exposure: f32) -> Self {
        let to_srgb = WORKING_SPACE
            .to_space_matrix(&RgbSpace::SRGB, CAT)
            .expect("the working space reaches sRGB")
            .rows
            .map(|r| r.map(|v| v as f32));
        Self { exposure, to_srgb }
    }

    fn map(&self, px: [f32; 3]) -> [u8; 3] {
        let px = px.map(|v| v * self.exposure);
        let y = luma(px);
        let px = if y > 0.0 {
            let k = (y / (1.0 + y)) / y;
            px.map(|v| v * k)
        } else {
            [0.0; 3]
        };
        let m = &self.to_srgb;
        std::array::from_fn(|c| {
            let v = m[c][0] * px[0] + m[c][1] * px[1] + m[c][2] * px[2];
            (srgb_encode(v.clamp(0.0, 1.0)) * 255.0).round() as u8
        })
    }
}

/// The whole frame, box-averaged (in linear light) to `THUMB_WIDTH`.
fn thumbnail(rgb: &[f32], w: usize, h: usize, tone: &Tone) -> RgbImage {
    let f = w.div_ceil(THUMB_WIDTH).max(1);
    let (tw, th) = (w / f, h / f);
    let mut img = RgbImage::new(tw as u32, th as u32);
    for ty in 0..th {
        for tx in 0..tw {
            let mut s = [0.0f32; 3];
            for y in ty * f..(ty + 1) * f {
                for x in tx * f..(tx + 1) * f {
                    let i = 3 * (y * w + x);
                    for (c, v) in s.iter_mut().enumerate() {
                        *v += rgb[i + c];
                    }
                }
            }
            let n = (f * f) as f32;
            img.put_pixel(tx as u32, ty as u32, image::Rgb(tone.map(s.map(|v| v / n))));
        }
    }
    img
}

fn crop(rgb: &[f32], w: usize, at: (usize, usize), tone: &Tone) -> RgbImage {
    let mut img = RgbImage::new(CROP as u32, CROP as u32);
    for y in 0..CROP {
        for x in 0..CROP {
            let i = 3 * ((at.1 + y) * w + at.0 + x);
            let px = [rgb[i], rgb[i + 1], rgb[i + 2]];
            img.put_pixel(x as u32, y as u32, image::Rgb(tone.map(px)));
        }
    }
    img
}

/// Up to two crop corners: on the largest region, then on the largest
/// specular or patch that the first does not cover.
pub fn crop_corners(map: &ClipMap) -> Vec<(usize, usize)> {
    let (w, h) = (map.width, map.height);
    if w < CROP || h < CROP {
        return Vec::new();
    }
    let corner = |b: [usize; 4]| {
        let cx = (b[0] + b[2]) / 2;
        let cy = (b[1] + b[3]) / 2;
        (
            cx.saturating_sub(CROP / 2).min(w - CROP),
            cy.saturating_sub(CROP / 2).min(h - CROP),
        )
    };
    let mut order: Vec<usize> = (0..map.regions.len()).collect();
    order.sort_by_key(|&k| std::cmp::Reverse(map.regions[k].pixels));
    let mut out: Vec<(usize, usize)> = Vec::new();
    let Some(&first) = order.first() else {
        return out;
    };
    out.push(corner(map.regions[first].bounds));
    let covered = |c: (usize, usize), b: [usize; 4]| {
        b[0] >= c.0 && b[2] <= c.0 + CROP && b[1] >= c.1 && b[3] <= c.1 + CROP
    };
    if let Some(&k) = order.iter().skip(1).find(|&&k| {
        let r = &map.regions[k];
        r.bin != SizeBin::Large && r.pixels >= 16 && !covered(out[0], r.bounds)
    }) {
        out.push(corner(map.regions[k].bounds));
    }
    out
}

/// One frame's sheet, written a panel at a time as each picture is
/// developed, so a large frame never holds every method's picture.
pub struct SheetWriter {
    dir: PathBuf,
    stem: String,
    width: usize,
    height: usize,
    tone: Tone,
    corners: Vec<(usize, usize)>,
    /// Per view (the full frame, then each crop): its figures.
    figures: Vec<Vec<String>>,
    head: String,
}

impl SheetWriter {
    /// A sheet for `map`'s frame, its tone set from `truth`; its files
    /// go under `dir` as `<stem>-<panel>-<view>.png`.
    pub fn new(dir: &Path, stem: &str, title: &str, map: &ClipMap, truth: &[f32]) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let exposure = exposure(map, truth);
        let corners = crop_corners(map);
        let mut head = String::new();
        let _ = writeln!(head, "<section><h2>{}</h2>", escape(title));
        let _ = writeln!(
            head,
            "<p>Exposure ×{exposure:.3} (the truth's median clipped luminance to 1), then y/(1+y) on luminance, hue kept, Rec.2020 to sRGB. Full frame box-averaged to {THUMB_WIDTH} px wide; crops 1:1, {CROP} px, at {corners:?}.</p>"
        );
        Ok(Self {
            dir: dir.to_path_buf(),
            stem: stem.to_string(),
            width: map.width,
            height: map.height,
            tone: Tone::new(exposure),
            figures: vec![Vec::new(); 1 + corners.len()],
            corners,
            head,
        })
    }

    /// Add a panel: `rgb` is working-space RGB of the frame's size.
    pub fn add(&mut self, label: &str, rgb: &[f32]) -> Result<()> {
        let n = self.figures[0].len();
        for v in 0..self.figures.len() {
            let name = format!("{}-{n}-{v}.png", self.stem);
            let img = if v == 0 {
                thumbnail(rgb, self.width, self.height, &self.tone)
            } else {
                crop(rgb, self.width, self.corners[v - 1], &self.tone)
            };
            img.save(self.dir.join(&name))
                .with_context(|| format!("writing {name}"))?;
            self.figures[v].push(format!(
                "<figure><img src=\"{name}\" loading=lazy><figcaption>{}</figcaption></figure>",
                escape(label)
            ));
        }
        Ok(())
    }

    /// The frame's section of the page.
    pub fn finish(self) -> String {
        let mut html = self.head;
        for row in self.figures {
            let _ = writeln!(html, "<div class=row>");
            for f in row {
                let _ = writeln!(html, "{f}");
            }
            let _ = writeln!(html, "</div>");
        }
        let _ = writeln!(html, "</section>");
        html
    }
}

/// The page round the frames' sections.
pub fn page(title: &str, intro: &str, sections: &[String]) -> String {
    let mut html = String::new();
    let _ = writeln!(
        html,
        "<!doctype html><html><head><meta charset=utf-8><title>{}</title><style>\
body{{background:#202020;color:#ddd;font:14px sans-serif;margin:16px}}\
.row{{display:flex;flex-wrap:wrap;gap:6px;margin-bottom:10px}}\
figure{{margin:0}}figcaption{{text-align:center;font-size:12px}}\
img{{display:block;image-rendering:pixelated}}</style></head><body><h1>{}</h1><p>{}</p>",
        escape(title),
        escape(title),
        escape(intro)
    );
    for s in sections {
        html.push_str(s);
    }
    html.push_str("</body></html>\n");
    html
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tone_keeps_grey_grey_and_rolls_highlights_off() {
        let t = Tone::new(1.0);
        let g = t.map([1.0, 1.0, 1.0]);
        assert_eq!(g[0], g[1]);
        assert_eq!(g[1], g[2]);
        // y = 1 lands at 0.5 linear.
        assert_eq!(g[0], (srgb_encode(0.5) * 255.0).round() as u8);
        let bright = t.map([16.0, 16.0, 16.0]);
        assert!(bright[0] > g[0] && bright[0] < 255);
        assert_eq!(t.map([0.0; 3]), [0, 0, 0]);
    }
}
