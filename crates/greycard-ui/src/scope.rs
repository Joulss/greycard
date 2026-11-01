//! The scopes drawn for the panel: the bins the viewport's compute
//! pass made (or its reference, `greycard_viewport::scope::bins`) as
//! Slint pictures, and where a color's marks sit on them.

pub(crate) use greycard_viewport::scope::{
    COLUMNS, Clipping, HIST, LEVELS, Reading, Scope, WHEEL, level, luma, readout, wheel_cell,
};
// The readout's test reads a picture of its own, with no GPU.
#[cfg(test)]
pub(crate) use greycard_viewport::scope::under_pointer;

/// The color of a chroma, at mid luma: the inverse of
/// [`chroma`](greycard_viewport::scope::chroma) with
/// Y a half, so the middle of the wheel is grey and its edge is
/// saturated.
fn color_of(cb: f32, cr: f32) -> [f32; 3] {
    [
        (0.5 + 1.5748 * cr).clamp(0.0, 1.0),
        (0.5 - 0.1873 * cb - 0.4681 * cr).clamp(0.0, 1.0),
        (0.5 + 1.8556 * cb).clamp(0.0, 1.0),
    ]
}

/// The color under the pointer on a scope, one mark of it: where, as
/// fractions of the scope's picture from its top left, and what it
/// marks, 0 to 2 a channel and 3 the luma. On the histogram a mark is
/// a line the height of the picture and its `y` says nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub x: f32,
    pub y: f32,
    pub channel: usize,
}

/// Where an encoded color lands on `scope`, `across` being how far
/// across the frame it is, from 0 to 1, which the waveform and the
/// parade place it by. Each mark sits in the middle of the cell the
/// binning puts the color in and the drawing draws that cell at, so
/// it is on the trace the pixel is part of: the histogram's bar for
/// each channel and the luma's level, the waveform's three channels
/// at the pixel's column, the parade's the same in their panels, and
/// the vectorscope's one cell.
pub fn marks(scope: Scope, c: [f32; 3], across: f32) -> Vec<Mark> {
    let (w, h) = scope.size();
    let (w, h) = (w as usize, h as usize);
    let column = ((across.clamp(0.0, 1.0) * COLUMNS as f32) as usize).min(COLUMNS - 1);
    // The picture's row a level is drawn on, as `waveform` draws it.
    let row = |v: f32| (h - 1 - (level(v) * h / LEVELS).min(h - 1)) as f32;
    let mid = |x: f32, y: f32| ((x + 0.5) / w as f32, (y + 0.5) / h as f32);
    match scope {
        Scope::Rgb => [c[0], c[1], c[2], luma(c)]
            .into_iter()
            .enumerate()
            .map(|(channel, v)| Mark {
                x: (level(v) as f32 + 0.5) / LEVELS as f32,
                y: 0.5,
                channel,
            })
            .collect(),
        Scope::Waveform | Scope::Parade => {
            let parade = scope == Scope::Parade;
            let panel = if parade { (w - 2) / 3 } else { w };
            (0..3)
                .map(|channel| {
                    let x0 = if parade { channel * (panel + 1) } else { 0 };
                    // The panel's pixel whose columns hold this one:
                    // the last x with `x * COLUMNS / panel <= column`.
                    let x = ((column + 1) * panel - 1) / COLUMNS;
                    let (x, y) = mid((x0 + x) as f32, row(c[channel]));
                    Mark { x, y, channel }
                })
                .collect()
        }
        Scope::Vector => {
            // The middle of the cell it bins to, which is where
            // `vector` draws that cell. A chroma off the grid bins
            // nowhere, so it has no mark: one on the edge cell would
            // sit on a cell the pixel never lit.
            let Some(cell) = wheel_cell(c) else {
                return Vec::new();
            };
            let n = WHEEL as f32;
            let cb = ((cell % WHEEL) as f32 + 0.5) / n - 0.5;
            let cr = 0.5 - ((cell / WHEEL) as f32 + 0.5) / n;
            let (cx, cy) = ((w - 1) as f32 / 2.0, (h - 1) as f32 / 2.0);
            let (x, y) = mid(
                (cx + 2.0 * cb * RADIUS).round(),
                (cy - 2.0 * cr * RADIUS).round(),
            );
            vec![Mark { x, y, channel: 3 }]
        }
    }
}

/// The panel's ground, which every scope is drawn on.
const GROUND: u8 = 0x14;
/// The vectorscope's radius, in pixels, at a chroma of a half.
const RADIUS: f32 = WHEEL as f32 / 2.0;

/// The scope as a picture for the panel. `bins` is what [`bins`] or
/// the compute pass made, the histogram first.
pub fn draw(scope: Scope, bins: &[u32]) -> slint::Image {
    let (w, h) = scope.size();
    let mut buf = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(w, h);
    let px = buf.make_mut_slice();
    for p in px.iter_mut() {
        *p = slint::Rgb8Pixel {
            r: GROUND,
            g: GROUND,
            b: GROUND,
        };
    }
    let (w, h) = (w as usize, h as usize);
    match scope {
        Scope::Rgb => histogram(px, w, h, bins),
        Scope::Waveform => waveform(px, w, h, bins, false),
        Scope::Parade => waveform(px, w, h, bins, true),
        Scope::Vector => vector(px, w, h, bins),
    }
    if let Some(path) = std::env::var_os("GREYCARD_UI_SCOPE")
        && let Some(img) = image::RgbImage::from_raw(w as u32, h as u32, buf.as_bytes().to_vec())
        && let Err(e) = img.save(&path)
    {
        tracing::warn!("scope dump {}: {e}", path.to_string_lossy());
    }
    slint::Image::from_rgb8(buf)
}

/// Brightness of a waveform cell holding `n` of `scale`: a square
/// root, which lifts the thin parts of a trace without letting the
/// stray pixels of the highlights shout over its body.
fn lit(n: u32, scale: f32) -> f32 {
    (n as f32 / scale).min(1.0).sqrt()
}

/// What a waveform is drawn to. The flattest part of a picture piles
/// up orders of magnitude more in one cell than a textured part does,
/// and scaling to the very top leaves everything else invisible; so
/// the densest few cells in a thousand clip instead, as they do on a
/// real waveform, and the rest of the trace has the range.
fn scale(cells: &[u32]) -> f32 {
    let mut counts: Vec<u32> = cells.iter().copied().filter(|&n| n != 0).collect();
    if counts.is_empty() {
        return 1.0;
    }
    counts.sort_unstable();
    let at = ((counts.len() as f32 * 0.995) as usize).min(counts.len() - 1);
    counts[at].max(1) as f32
}

/// Brightness of a vectorscope cell. A photograph is mostly neutral,
/// so one cell in the middle holds orders of magnitude more than the
/// colors around it; a logarithm, lifted, shows both.
fn lit_log(n: u32, peak: f32) -> f32 {
    if n == 0 {
        0.0
    } else {
        ((n as f32 + 1.0).ln() / (peak + 1.0).ln()).sqrt()
    }
}

fn add_at(px: &mut [slint::Rgb8Pixel], w: usize, h: usize, x: isize, y: isize, rgb: [u8; 3]) {
    if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
        return;
    }
    let p = &mut px[y as usize * w + x as usize];
    p.r = p.r.saturating_add(rgb[0]);
    p.g = p.g.saturating_add(rgb[1]);
    p.b = p.b.saturating_add(rgb[2]);
}

/// Bars per channel, additive, as an editor's histogram.
fn histogram(px: &mut [slint::Rgb8Pixel], w: usize, h: usize, bins: &[u32]) {
    // The end bins hold everything clipped; they set no scale.
    let peak = (0..3)
        .flat_map(|c| {
            bins[c * LEVELS + 1..c * LEVELS + LEVELS - 1]
                .iter()
                .copied()
        })
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let height = |n: u32| ((n as f32 / peak).min(1.0).sqrt() * h as f32).ceil() as usize;
    for x in 0..w {
        let bin = x * LEVELS / w;
        let hs = [
            height(bins[bin]),
            height(bins[LEVELS + bin]),
            height(bins[2 * LEVELS + bin]),
        ];
        for y in 0..h {
            let from_bottom = h - 1 - y;
            let on = [
                hs[0] > from_bottom,
                hs[1] > from_bottom,
                hs[2] > from_bottom,
            ];
            if !on.iter().any(|&o| o) {
                continue;
            }
            let p = &mut px[y * w + x];
            p.r = if on[0] { 0xd0 } else { 0x30 };
            p.g = if on[1] { 0xd0 } else { 0x30 };
            p.b = if on[2] { 0xd0 } else { 0x30 };
        }
    }
}

/// Levels up the picture against the image's columns across it. As a
/// parade the three channels get a panel each, side by side;
/// otherwise they lie over one another and a neutral frame is grey.
fn waveform(px: &mut [slint::Rgb8Pixel], w: usize, h: usize, bins: &[u32], parade: bool) {
    let wave = &bins[HIST..];
    let panel = if parade { (w - 2) / 3 } else { w };
    // Bin the levels into the picture's rows first, so the scale is
    // over what is drawn and not over the finer bins behind it.
    let mut cells = vec![0u32; 3 * panel * h];
    for ch in 0..3 {
        for x in 0..panel {
            let from = x * COLUMNS / panel;
            let to = (((x + 1) * COLUMNS) / panel).max(from + 1).min(COLUMNS);
            for column in from..to {
                let base = (ch * COLUMNS + column) * LEVELS;
                for l in 0..LEVELS {
                    let n = wave[base + l];
                    if n != 0 {
                        let row = h - 1 - (l * h / LEVELS).min(h - 1);
                        cells[(ch * h + row) * panel + x] += n;
                    }
                }
            }
        }
    }
    let peak = scale(&cells);
    // Over one another the three channels add, so a neutral picture
    // would be white everywhere at a panel's gain; give each a third
    // of the range and let the overlaps make the white.
    let gain = if parade { 200.0 } else { 110.0 };
    for ch in 0..3 {
        let x0 = if parade { ch * (panel + 1) } else { 0 };
        for row in 0..h {
            for x in 0..panel {
                let i = lit(cells[(ch * h + row) * panel + x], peak);
                if i == 0.0 {
                    continue;
                }
                let mut rgb = [0u8; 3];
                rgb[ch] = (i * gain) as u8;
                add_at(px, w, h, (x0 + x) as isize, row as isize, rgb);
            }
        }
    }
}

/// Chroma on a Cb/Cr wheel, colored by where it lands, over a
/// graticule with the skin-tone line.
fn vector(px: &mut [slint::Rgb8Pixel], w: usize, h: usize, bins: &[u32]) {
    let (cx, cy) = ((w - 1) as f32 / 2.0, (h - 1) as f32 / 2.0);
    let mut mark = |x: f32, y: f32, rgb: [u8; 3]| {
        add_at(px, w, h, x.round() as isize, y.round() as isize, rgb);
    };
    // The graticule: rings at a quarter and a half of chroma, the
    // axes, and the skin-tone line at 123 degrees from +Cb.
    for &(r, tone) in &[(RADIUS / 2.0, 0x0au8), (RADIUS, 0x16u8)] {
        let steps = (r * 8.0) as usize + 8;
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            mark(cx + r * a.cos(), cy - r * a.sin(), [tone; 3]);
        }
    }
    let mut i = 0;
    while i < RADIUS as usize {
        let d = i as f32;
        for (x, y) in [(cx + d, cy), (cx - d, cy), (cx, cy + d), (cx, cy - d)] {
            mark(x, y, [0x0e; 3]);
        }
        i += 3;
    }
    let skin = 123.0f32.to_radians();
    for i in 0..RADIUS as usize {
        let d = i as f32;
        mark(cx + d * skin.cos(), cy - d * skin.sin(), [0x40, 0x2a, 0x1e]);
    }
    // The trace. One cell is one pixel, so nothing falls between.
    let wheel = &bins[HIST..];
    let peak = wheel.iter().copied().max().unwrap_or(1).max(1) as f32;
    for v in 0..WHEEL {
        for u in 0..WHEEL {
            let i = lit_log(wheel[v * WHEEL + u], peak);
            if i == 0.0 {
                continue;
            }
            let cb = (u as f32 + 0.5) / WHEEL as f32 - 0.5;
            let cr = 0.5 - (v as f32 + 0.5) / WHEEL as f32;
            // At its own brightness, so the intensity alone says how
            // much landed here and the color only says where.
            let c = color_of(cb, cr);
            let top = c[0].max(c[1]).max(c[2]).max(1e-3);
            let rgb = [
                (c[0] / top * i * 255.0) as u8,
                (c[1] / top * i * 255.0) as u8,
                (c[2] / top * i * 255.0) as u8,
            ];
            add_at(
                px,
                w,
                h,
                (cx + 2.0 * cb * RADIUS).round() as isize,
                (cy - 2.0 * cr * RADIUS).round() as isize,
                rgb,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use greycard_viewport::scope::{bins, weighted_bins};
    use greycard_viewport::testing::{colors, flat, ramp};

    use super::*;

    fn pixels(image: &slint::Image) -> (Vec<slint::Rgb8Pixel>, usize, usize) {
        let rgb = image.to_rgb8().unwrap();
        (
            rgb.as_slice().to_vec(),
            rgb.width() as usize,
            rgb.height() as usize,
        )
    }

    #[test]
    fn the_histogram_picture_follows_the_bins() {
        // Red in the shadows, blue in the highlights, nothing else.
        let mut bins = vec![0u32; HIST];
        bins[20] = 1000;
        bins[512 + 200] = 500;
        let (px, w, _) = pixels(&draw(Scope::Rgb, &bins));
        let at = |x: usize, y: usize| px[y * w + x];
        // Column 20 is red to the top; column 200 blue to about
        // sqrt(1/2) of the height; column 100 is background.
        assert!(at(20, 1).r > 0xc0 && at(20, 1).b < 0x40);
        assert!(at(200, 79).b > 0xc0 && at(200, 79).r < 0x40);
        assert!(at(200, 10).b < 0x20, "blue does not reach the top");
        assert!(at(100, 79).r < 0x20 && at(100, 79).g < 0x20);
    }

    #[test]
    fn the_waveform_draws_a_ramp_on_the_diagonal() {
        let (w, h) = (256, 64);
        let bins = bins(Scope::Waveform, &ramp(w, h), w, h);
        let (px, pw, ph) = pixels(&draw(Scope::Waveform, &bins));
        let lit = |x: usize, y: usize| px[y * pw + x].g > GROUND + 0x20;
        // Dark on the left, bright on the right, and the other way up.
        assert!(lit(0, ph - 1), "the darkest column sits at the bottom");
        assert!(!lit(0, 0));
        assert!(lit(pw - 1, 0), "the brightest column sits at the top");
        assert!(!lit(pw - 1, ph - 1));
        // A grey ramp lights all three channels together.
        let p = px[(ph - 1) * pw];
        assert!(p.r.abs_diff(p.g) < 8 && p.g.abs_diff(p.b) < 8, "{p:?}");
    }

    #[test]
    fn the_parade_gives_each_channel_a_panel() {
        let (w, h) = (128, 64);
        let bins = bins(Scope::Parade, &flat(w, h, [1.0, 0.0, 0.0]), w, h);
        let (px, pw, ph) = pixels(&draw(Scope::Parade, &bins));
        let panel = (pw - 2) / 3;
        // The panels do not divide the bins evenly, so read the
        // brightest pixel of a row rather than one column of it.
        let row = |ch: usize, y: usize| {
            let x0 = ch * (panel + 1);
            (x0..x0 + panel)
                .map(|x| match ch {
                    0 => px[y * pw + x].r,
                    1 => px[y * pw + x].g,
                    _ => px[y * pw + x].b,
                })
                .max()
                .unwrap()
        };
        // Red is clipped, so it lies along the top of the first panel;
        // green and blue are at nothing, along the bottom of theirs.
        assert!(row(0, 0) > 0xa0, "red at the top of its panel");
        assert!(row(0, ph - 1) < 0x40, "and nowhere near the bottom");
        assert!(row(1, ph - 1) > 0xa0, "green at the bottom of its panel");
        assert!(row(1, 0) < 0x40);
        assert!(row(2, ph - 1) > 0xa0, "blue at the bottom of its panel");
    }

    #[test]
    fn the_wheel_puts_grey_in_the_middle_and_red_above_it() {
        let (w, h) = (32, 32);
        let (pw, _) = Scope::Vector.size();
        let pw = pw as usize;
        let center = pw / 2;

        let grey = bins(Scope::Vector, &flat(w, h, [0.5; 3]), w, h);
        let (px, _, _) = pixels(&draw(Scope::Vector, &grey));
        let p = px[center * pw + center];
        assert!(
            p.r > 0x80 && p.g > 0x80 && p.b > 0x80,
            "grey is a bright middle: {p:?}"
        );

        let red = bins(Scope::Vector, &flat(w, h, [1.0, 0.0, 0.0]), w, h);
        let (px, _, _) = pixels(&draw(Scope::Vector, &red));
        let (x, y) = px
            .iter()
            .enumerate()
            .max_by_key(|(_, p)| p.r as u32 + p.g as u32 + p.b as u32)
            .map(|(i, _)| (i % pw, i / pw))
            .unwrap();
        let p = px[y * pw + x];
        assert!(p.r > p.g + 0x40 && p.r > p.b + 0x40, "red reads red: {p:?}");
        assert!(y < center - 80, "red is near the top: {y}");
        assert!(x < center && x > center - 40, "a little to the left: {x}");
    }

    #[test]
    fn a_mask_of_nothing_is_an_empty_scope() {
        let (w, h) = (64, 40);
        let image = colors(w, h);
        let none = vec![0.0; w * h];
        for scope in Scope::ALL {
            let b = weighted_bins(scope, &image, &none, w, h);
            assert_eq!(b.len(), scope.bin_count());
            assert!(b.iter().all(|&n| n == 0), "{scope:?}");
            assert_eq!(Clipping::of(&b), Clipping::default());
            // It draws, with no trace: the histogram is all ground.
            let (px, _, _) = pixels(&draw(scope, &b));
            if scope == Scope::Rgb {
                assert!(
                    px.iter()
                        .all(|p| p.r == GROUND && p.g == GROUND && p.b == GROUND)
                );
            }
        }
    }

    /// The pixel of a scope's picture a mark is on, and its channel's
    /// value there; the luma's mark reads the brightest.
    fn under(px: &[slint::Rgb8Pixel], w: usize, h: usize, m: Mark) -> u8 {
        let x = ((m.x * w as f32) as usize).min(w - 1);
        let y = ((m.y * h as f32) as usize).min(h - 1);
        let p = px[y * w + x];
        match m.channel {
            0 => p.r,
            1 => p.g,
            2 => p.b,
            _ => p.r.max(p.g).max(p.b),
        }
    }

    #[test]
    fn a_colors_marks_land_on_its_histogram_bars_and_its_wheel_cell() {
        let colors = [
            [0.8, 0.45, 0.3],
            [0.2, 0.6, 0.9],
            [0.5, 0.5, 0.5],
            [0.95, 0.1, 0.6],
        ];
        for scope in [Scope::Rgb, Scope::Vector] {
            for (i, &c) in colors.iter().enumerate() {
                let (w, h) = (16, 8);
                let bins = bins(scope, &flat(w, h, c), w, h);
                let (px, pw, ph) = pixels(&draw(scope, &bins));
                // The histogram's bar reaches the top for a flat
                // picture; read its foot, under the luma's line, which
                // has no bar of its own.
                let foot = |m: Mark| Mark {
                    y: if scope == Scope::Rgb { 0.99 } else { m.y },
                    ..m
                };
                for m in marks(scope, c, 0.5)
                    .into_iter()
                    .filter(|m| scope == Scope::Vector || m.channel < 3)
                {
                    assert!(
                        under(&px, pw, ph, foot(m)) > 0xc0,
                        "{scope:?} {c:?} {m:?} is on the trace"
                    );
                }
                // Another color's marks are not, for the channels
                // whose levels differ; the graticule where the wheel's
                // axes cross the skin line is lit to about 0x78.
                let other = colors[(i + 1) % colors.len()];
                for m in marks(scope, other, 0.5) {
                    let differs = match m.channel {
                        3 => scope == Scope::Vector && wheel_cell(c) != wheel_cell(other),
                        ch => level(c[ch]) != level(other[ch]),
                    };
                    if differs {
                        assert!(
                            under(&px, pw, ph, foot(m)) < 0x90,
                            "{scope:?} {other:?} {m:?} is off it"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_pixels_marks_land_on_its_column_of_the_waveform_and_the_parade() {
        // A color a column, every channel on its own path, so each
        // column's trace is somewhere else.
        let (w, h) = (256, 4);
        let at = |x: usize| {
            let t = x as f32 / (w - 1) as f32;
            [t, 1.0 - t, ((x * 7) % w) as f32 / (w - 1) as f32]
        };
        let picture: Vec<[f32; 3]> = (0..w * h).map(|i| at(i % w)).collect();
        for scope in [Scope::Waveform, Scope::Parade] {
            let bins = bins(scope, &picture, w, h);
            let (px, pw, ph) = pixels(&draw(scope, &bins));
            for x in (0..w).step_by(5) {
                let across = (x as f32 + 0.5) / w as f32;
                for m in marks(scope, at(x), across) {
                    assert!(
                        under(&px, pw, ph, m) > GROUND + 0x10,
                        "{scope:?} column {x} {m:?} is on the trace"
                    );
                }
            }
            // The first column's color at the middle column is off
            // the trace: there the levels are elsewhere.
            for m in marks(scope, at(0), 0.5) {
                assert!(
                    under(&px, pw, ph, m) <= GROUND + 0x04,
                    "{scope:?} {m:?} is off it"
                );
            }
        }
    }

    #[test]
    fn a_color_off_the_wheel_has_no_vectorscope_mark() {
        // Pure blue's Cb is a half, on the grid's far edge: it bins to
        // no cell, so a ring on the edge cell would be on nothing lit.
        assert_eq!(wheel_cell([0.0, 0.0, 1.0]), None);
        assert!(marks(Scope::Vector, [0.0, 0.0, 1.0], 0.5).is_empty());
        // Its levels still mark the other scopes.
        assert_eq!(marks(Scope::Rgb, [0.0, 0.0, 1.0], 0.5).len(), 4);
        assert_eq!(marks(Scope::Vector, [0.2, 0.3, 0.9], 0.5).len(), 1);
    }

    #[test]
    fn the_parade_puts_each_channel_in_its_own_panel() {
        let m = marks(Scope::Parade, [0.5; 3], 1.0);
        assert!(
            m[0].x < 1.0 / 3.0 && m[1].x > 1.0 / 3.0 && m[1].x < 2.0 / 3.0 && m[2].x > 2.0 / 3.0
        );
        // Grey is in the middle of the wheel, and on the histogram
        // all four lines are one.
        let v = marks(Scope::Vector, [0.5; 3], 0.0)[0];
        assert!(
            (v.x - 0.5).abs() < 0.01 && (v.y - 0.5).abs() < 0.01,
            "{v:?}"
        );
        let rgb = marks(Scope::Rgb, [0.5; 3], 0.0);
        assert!(rgb.iter().all(|m| m.x == rgb[0].x));
    }
}
