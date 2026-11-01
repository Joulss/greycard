//! The scopes' binning: the RGB histogram, the waveform, the RGB
//! parade and the vectorscope.
//!
//! All four are binnings of the same picture, the whole frame drawn
//! small through the viewport's shader, so they show what the screen
//! shows: encoded output, not linear light. [`bins`] is the reference
//! for `scope.wgsl`, which bins the same way on the GPU; the window
//! draws the bins, and its tests check the drawing against bins the
//! reference made.
//!
//! One buffer holds them all. The histogram is always there, in the
//! first [`HIST`] bins, because the curve editor draws it behind the
//! curve whatever the panel shows; the chosen scope's own bins follow
//! it.

/// Levels a channel is binned into.
pub const LEVELS: usize = 256;
/// The histogram: 256 red, 256 green, 256 blue.
pub const HIST: usize = 3 * LEVELS;
/// Columns of the waveform. The parade reads the same bins.
pub const COLUMNS: usize = 256;
/// The waveform: a level histogram per column per channel.
pub const WAVE: usize = 3 * COLUMNS * LEVELS;
/// Cells across the vectorscope's square grid of Cb and Cr. One cell
/// is one pixel of the picture: `2 * RADIUS == WHEEL`.
pub const WHEEL: usize = 208;
/// The vectorscope.
pub const VECTOR: usize = WHEEL * WHEEL;
/// The most bins any scope asks for, which is what the buffer holds.
pub const MAX: usize = HIST + WAVE;

/// Which scope the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// The RGB histogram.
    #[default]
    Rgb,
    /// Levels against image column, the channels over each other.
    Waveform,
    /// The waveform's three channels side by side.
    Parade,
    /// Chroma on a Cb/Cr wheel, with the skin-tone line.
    Vector,
}

impl Scope {
    /// In the order the panel offers them.
    pub const ALL: [Scope; 4] = [Scope::Rgb, Scope::Waveform, Scope::Parade, Scope::Vector];

    pub fn name(self) -> &'static str {
        match self {
            Scope::Rgb => "RGB",
            Scope::Waveform => "Wave",
            Scope::Parade => "Parade",
            Scope::Vector => "Vector",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// What the shader binds beyond the histogram: 0 nothing, 1 the
    /// waveform, 2 the vectorscope.
    pub fn kind(self) -> u32 {
        match self {
            Scope::Rgb => 0,
            Scope::Waveform | Scope::Parade => 1,
            Scope::Vector => 2,
        }
    }

    /// Bins this scope reads, the histogram's included.
    pub fn bin_count(self) -> usize {
        match self.kind() {
            1 => HIST + WAVE,
            2 => HIST + VECTOR,
            _ => HIST,
        }
    }

    /// The picture's size in pixels. The histogram is low; a waveform
    /// wants height to hold a trace apart; the vectorscope is square
    /// so its circle is round however wide the panel is.
    pub fn size(self) -> (u32, u32) {
        match self {
            Scope::Rgb => (256, 80),
            Scope::Waveform | Scope::Parade => (256, 140),
            Scope::Vector => (WHEEL as u32 + 12, WHEEL as u32 + 12),
        }
    }
}

// The binning below is the reference: `scope.wgsl` is written from
// it and the tests measure the drawing against it. The running
// program bins on the GPU; only the marks of the color under the
// pointer call these, to put each mark in the cell its color bins to.

/// The level a channel value falls in, as the shader rounds it.
pub fn level(v: f32) -> usize {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as usize
}

/// The cell of the vectorscope's grid a color falls in, or `None`
/// when its chroma is off the grid. Rec.709 luma, and Cb and Cr each
/// from -0.5 to 0.5; the grid runs left to right in Cb and top to
/// bottom in falling Cr, as the picture is drawn.
pub fn wheel_cell(c: [f32; 3]) -> Option<usize> {
    let (cb, cr) = chroma(c);
    let u = ((cb + 0.5) * WHEEL as f32).floor();
    let v = ((0.5 - cr) * WHEEL as f32).floor();
    if u < 0.0 || v < 0.0 || u >= WHEEL as f32 || v >= WHEEL as f32 {
        return None;
    }
    Some(v as usize * WHEEL + u as usize)
}

/// Rec.709 Cb and Cr of an encoded color.
pub fn chroma(c: [f32; 3]) -> (f32, f32) {
    let y = luma(c);
    ((c[2] - y) / 1.8556, (c[0] - y) / 1.5748)
}

/// The reference binning, over an encoded picture. `scope.wgsl` does
/// the same on the GPU over the analysis texture.
pub fn bins(scope: Scope, pixels: &[[f32; 3]], width: usize, height: usize) -> Vec<u32> {
    accumulate(scope, pixels, width, height, |_| 1)
}

/// The steps a pixel's coverage is held in: the analysis texture's
/// alpha, eight bits, where the viewport shader writes the weighing
/// mask's coverage.
pub const COVERAGE_STEPS: u32 = 255;

/// A coverage in [0, 1] as the analysis texture holds it, and as
/// `scope.wgsl` reads it back: a whole number of [`COVERAGE_STEPS`].
pub fn coverage_step(w: f32) -> u32 {
    (w.clamp(0.0, 1.0) * COVERAGE_STEPS as f32 + 0.5) as u32
}

/// The reference binning weighted by a mask: each pixel adds its
/// coverage, in [`COVERAGE_STEPS`], to every bin it lands in rather
/// than one, and the bins are then brought back to pixels by
/// [`normalize`]. `coverage` is a value a pixel, in [0, 1]. This is
/// `scope.wgsl` with its `weighted` switch on, followed by what
/// `Renderer::analyze` does with the read back.
pub fn weighted_bins(
    scope: Scope,
    pixels: &[[f32; 3]],
    coverage: &[f32],
    width: usize,
    height: usize,
) -> Vec<u32> {
    let mut out = accumulate(scope, pixels, width, height, |i| coverage_step(coverage[i]));
    normalize(&mut out, width * height);
    out
}

/// Weighted bins, which hold coverage steps, as pixels again, scaled
/// so the selection has the mass of the whole picture: a bin holding
/// `n` of a coverage sum `sum` becomes `n * pixels / sum`, rounded.
/// The histogram's end bins, which the clipping marks read
/// ([`Clipping::of`]), are never rounded to nothing where anything
/// landed, so a single pixel at the edge of a mask still lights a
/// mark; every other bin rounds, so the faint edge of a soft mask
/// does not lift the cells it touches into view. The coverage sum is
/// the histogram's red channel, into which every pixel adds its
/// coverage once.
///
/// So a small mask's scope is as bright as the whole frame's (the
/// drawing scales to its densest cells, and the mass is the same),
/// a mask of one everywhere gives the unweighted bins exactly, and a
/// mask of nothing gives no bins at all.
pub fn normalize(bins: &mut [u32], pixels: usize) {
    let sum: u64 = bins[..LEVELS.min(bins.len())]
        .iter()
        .map(|&n| u64::from(n))
        .sum();
    if sum == 0 {
        bins.fill(0);
        return;
    }
    let pixels = pixels as u64;
    let clip = |i: usize| i < HIST && matches!(i % LEVELS, 0 | 255);
    for (i, n) in bins.iter_mut().enumerate() {
        if *n != 0 {
            let scaled = (2 * u64::from(*n) * pixels + sum) / (2 * sum);
            let floor = if clip(i) { 1 } else { 0 };
            *n = scaled.clamp(floor, u64::from(u32::MAX)) as u32;
        }
    }
}

/// Bin every pixel, adding `weight(i)` for the pixel at index `i`.
fn accumulate(
    scope: Scope,
    pixels: &[[f32; 3]],
    width: usize,
    height: usize,
    weight: impl Fn(usize) -> u32,
) -> Vec<u32> {
    let mut out = vec![0u32; scope.bin_count()];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let q = weight(i);
            if q == 0 {
                continue;
            }
            let c = pixels[i];
            for (ch, v) in c.iter().enumerate() {
                out[ch * LEVELS + level(*v)] += q;
            }
            match scope.kind() {
                1 => {
                    let column = (x * COLUMNS / width).min(COLUMNS - 1);
                    for (ch, v) in c.iter().enumerate() {
                        out[HIST + (ch * COLUMNS + column) * LEVELS + level(*v)] += q;
                    }
                }
                2 => {
                    if let Some(cell) = wheel_cell(c) {
                        out[HIST + cell] += q;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Which channels the histogram's end bins hold: what the output
/// clips to black and to white. A channel clips when any pixel of
/// the analysis image lands in its first or last bin, as the level
/// [`level`] rounds it, so one bright pixel in a few hundred
/// thousand lights the mark, as it does on the histogram itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clipping {
    pub shadows: [bool; 3],
    pub highlights: [bool; 3],
}

impl Clipping {
    /// From the histogram's bins.
    pub fn of(bins: &[u32]) -> Self {
        let at = |i: usize| bins.get(i).copied().unwrap_or(0) > 0;
        Self {
            shadows: [at(0), at(LEVELS), at(2 * LEVELS)],
            highlights: [at(LEVELS - 1), at(2 * LEVELS - 1), at(3 * LEVELS - 1)],
        }
    }

    /// The color of a mark for the channels in `chans`: red, green
    /// or blue for one of them; yellow, magenta or cyan for two;
    /// white for all three; `None` when nothing clips.
    pub fn mark(chans: [bool; 3]) -> Option<[u8; 3]> {
        if !chans.iter().any(|&c| c) {
            return None;
        }
        Some(chans.map(|c| if c { 0xff } else { 0x00 }))
    }
}

/// The Rec.709 luma of an encoded color, the Y the vectorscope's Cb
/// and Cr are taken against.
pub fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The color under the pointer in words: each channel's level and
/// the luma's, 0 to 255, as the scopes bin them.
pub fn readout(c: [f32; 3]) -> String {
    format!(
        "R {}  G {}  B {}  L {}",
        level(c[0]),
        level(c[1]),
        level(c[2]),
        level(luma(c))
    )
}

/// The color under the pointer as the analysis picture holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// Encoded, 0 to 1: one of the picture's own pixels, so it is in
    /// the bins that picture made.
    pub color: [f32; 3],
    /// Whether any of the box read is in the scopes' selection.
    /// Unweighed, every pixel is.
    pub selected: bool,
    /// How far across the frame that pixel is, 0 to 1: the middle of
    /// its column, which the waveform and the parade bin it in.
    pub across: f32,
}

/// The color the readout reads about a point of the analysis picture,
/// `width` by `height` with the scopes' selection's coverage in the
/// alpha (one everywhere unweighed): `u` and `v` are fractions of the
/// frame, and the box read reaches `radius` of the width about the
/// pixel under them (that one pixel when it is under one).
///
/// The box's mean calms the marks over a textured area, but a mean
/// can fall between the colors the box holds, on a cell nothing lit:
/// an edge between two hues puts the vectorscope's ring between their
/// two clouds. So the color is the box's own pixel nearest the mean,
/// and every mark sits on a lit cell. The mean is weighed by the
/// coverage, as the bins are, and only covered pixels are candidates,
/// so on a mask's soft edge it is the selected part's color. With
/// nothing in the box selected it is the plain mean's nearest, and
/// the pixel is on no trace. `None` for an empty picture.
pub fn under_pointer(
    pixels: &[[u8; 4]],
    width: usize,
    height: usize,
    u: f32,
    v: f32,
    radius: f32,
) -> Option<Reading> {
    if width == 0 || height == 0 || pixels.len() < width * height {
        return None;
    }
    let (w, h) = (width as i64, height as i64);
    let x = ((u * w as f32) as i64).clamp(0, w - 1);
    let y = ((v * h as f32) as i64).clamp(0, h - 1);
    let reach = (radius * w as f32).round().max(0.0) as i64;
    let (xs, ys) = (
        (x - reach).max(0)..=(x + reach).min(w - 1),
        (y - reach).max(0)..=(y + reach).min(h - 1),
    );
    let at = |x: i64, y: i64| pixels[(y * w + x) as usize];
    let (mut weighed, mut covered) = ([0u64; 3], 0u64);
    let (mut plain, mut n) = ([0u64; 3], 0u64);
    for y in ys.clone() {
        for x in xs.clone() {
            let p = at(x, y);
            let q = u64::from(p[3]);
            for (i, c) in [p[0], p[1], p[2]].into_iter().enumerate() {
                weighed[i] += q * u64::from(c);
                plain[i] += u64::from(c);
            }
            covered += q;
            n += 1;
        }
    }
    let selected = covered > 0;
    let mean = if selected {
        weighed.map(|s| s as f32 / covered as f32)
    } else {
        plain.map(|s| s as f32 / n as f32)
    };
    // The nearest in the box, the first in reading order of any tied.
    let mut best: Option<(f32, i64, [u8; 4])> = None;
    for y in ys {
        for x in xs.clone() {
            let p = at(x, y);
            if selected && p[3] == 0 {
                continue;
            }
            let d: f32 = [p[0], p[1], p[2]]
                .into_iter()
                .zip(mean)
                .map(|(c, m)| (f32::from(c) - m).powi(2))
                .sum();
            if best.is_none_or(|(b, _, _)| d < b) {
                best = Some((d, x, p));
            }
        }
    }
    let (_, x, p) = best?;
    Some(Reading {
        color: [p[0], p[1], p[2]].map(|c| f32::from(c) / 255.0),
        selected,
        across: (x as f32 + 0.5) / w as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{colors, flat, ramp};

    #[test]
    fn every_pixel_lands_in_a_bin() {
        let (w, h) = (64, 40);
        let image = ramp(w, h);
        for scope in Scope::ALL {
            let bins = bins(scope, &image, w, h);
            assert_eq!(bins.len(), scope.bin_count());
            let hist: u32 = bins[..HIST].iter().sum();
            assert_eq!(hist as usize, w * h * 3, "{scope:?} histogram");
            match scope.kind() {
                1 => assert_eq!(
                    bins[HIST..].iter().sum::<u32>() as usize,
                    w * h * 3,
                    "waveform"
                ),
                // A grey ramp is all on the wheel's center column.
                2 => assert_eq!(bins[HIST..].iter().sum::<u32>() as usize, w * h, "wheel"),
                _ => {}
            }
        }
    }

    #[test]
    fn the_clipping_reads_the_end_bins() {
        let mut ends = vec![0u32; HIST];
        assert_eq!(Clipping::of(&ends), Clipping::default());
        assert_eq!(Clipping::mark([false; 3]), None);
        // One red pixel at the top, and green and blue at the bottom.
        ends[LEVELS - 1] = 1;
        ends[LEVELS] = 3;
        ends[2 * LEVELS] = 7;
        let c = Clipping::of(&ends);
        assert_eq!(c.highlights, [true, false, false]);
        assert_eq!(c.shadows, [false, true, true]);
        assert_eq!(Clipping::mark(c.highlights), Some([0xff, 0, 0]));
        assert_eq!(Clipping::mark(c.shadows), Some([0, 0xff, 0xff]));
        assert_eq!(Clipping::mark([true; 3]), Some([0xff; 3]));
        // The bins of a ramp reach both ends in every channel.
        let (w, h) = (256, 4);
        let ramp = bins(Scope::Rgb, &ramp(w, h), w, h);
        assert_eq!(
            Clipping::of(&ramp),
            Clipping {
                shadows: [true; 3],
                highlights: [true; 3]
            }
        );
    }

    #[test]
    fn a_mask_of_one_everywhere_is_the_unweighted_scope() {
        let (w, h) = (97, 61);
        let image = colors(w, h);
        let ones = vec![1.0; w * h];
        for scope in Scope::ALL {
            assert_eq!(
                weighted_bins(scope, &image, &ones, w, h),
                bins(scope, &image, w, h),
                "{scope:?}"
            );
        }
    }

    #[test]
    fn a_small_selection_has_the_whole_pictures_mass() {
        // The left eighth of a ramp, selected: its bins are the left
        // eighth's alone, scaled up to the whole picture's count, so
        // the scope is drawn as bright as the whole frame's.
        let (w, h) = (256, 16);
        let image = ramp(w, h);
        let coverage: Vec<f32> = (0..w * h)
            .map(|i| if i % w < w / 8 { 1.0 } else { 0.0 })
            .collect();
        for scope in Scope::ALL {
            let b = weighted_bins(scope, &image, &coverage, w, h);
            let hist: u32 = b[..HIST].iter().sum();
            assert_eq!(hist as usize, 3 * w * h, "{scope:?}");
            // Nothing from the right of the selection.
            assert!(b[level(0.5)..LEVELS].iter().all(|&n| n == 0));
            assert_eq!(b[0], 8 * h as u32, "the darkest column, eight times over");
        }
        // A fifth of a coverage counts a fifth: a picture half at 0.2
        // and half at 0.8, the dark half weighed 51 of the texture's
        // 255 steps and the bright one all of them, holds five times
        // the bright.
        let mut image = flat(w, h, [0.2; 3]);
        image[w * h / 2..].fill([0.8; 3]);
        let coverage: Vec<f32> = (0..w * h)
            .map(|i| if i < w * h / 2 { 51.0 / 255.0 } else { 1.0 })
            .collect();
        let b = weighted_bins(Scope::Rgb, &image, &coverage, w, h);
        let (dark, bright) = (b[level(0.2)], b[level(0.8)]);
        assert_eq!(dark + bright, (w * h) as u32);
        assert!(bright.abs_diff(5 * dark) <= 5, "{dark} {bright}");
    }

    #[test]
    fn a_faint_edge_does_not_lift_a_scope_cell() {
        // A grey field wholly selected, and one saturated mid-level
        // pixel at the lowest coverage step: it rounds to nothing in
        // the waveform and on the wheel, as the rounding says, rather
        // than being lifted to a count that draws.
        let (w, h) = (128, 128);
        let mut image = flat(w, h, [0.5; 3]);
        let odd = [0.7, 0.3, 0.4];
        image[w * h / 2 + w / 2] = odd;
        let mut coverage = vec![1.0; w * h];
        coverage[w * h / 2 + w / 2] = 1.0 / 255.0;
        let wave = weighted_bins(Scope::Waveform, &image, &coverage, w, h);
        let column = (w / 2) * COLUMNS / w;
        let at = HIST + column * LEVELS + level(odd[0]);
        assert_eq!(wave[at], 0, "the red level of the faint pixel");
        let wheel = weighted_bins(Scope::Vector, &image, &coverage, w, h);
        let cell = wheel_cell(odd).unwrap();
        assert_ne!(Some(cell), wheel_cell([0.5; 3]));
        assert_eq!(wheel[HIST + cell], 0, "the faint pixel's chroma");
        // And the mass is the whole picture's less the faint pixel's
        // three rounded-away levels, not lifted by it.
        let mass: u32 = wave[HIST..].iter().sum();
        assert_eq!(mass as usize, 3 * w * h - 3);
    }

    #[test]
    fn a_pixel_at_the_edge_of_a_mask_still_lands() {
        // One clipped pixel at the lowest coverage the texture holds,
        // beside a fully selected field: it is a count still, and the
        // clipping mark lights for it.
        let (w, h) = (128, 128);
        let mut image = flat(w, h, [0.5; 3]);
        image[0] = [1.0, 0.5, 0.5];
        let mut coverage = vec![1.0; w * h];
        coverage[0] = 1.0 / 255.0;
        let b = weighted_bins(Scope::Rgb, &image, &coverage, w, h);
        assert_eq!(b[LEVELS - 1], 1);
        assert!(Clipping::of(&b).highlights[0]);
        // And below the texture's first step it is not in the mask.
        coverage[0] = 0.4 / 255.0;
        let b = weighted_bins(Scope::Rgb, &image, &coverage, w, h);
        assert_eq!(b[LEVELS - 1], 0);
    }

    #[test]
    fn the_names_round_trip() {
        for scope in Scope::ALL {
            assert_eq!(Scope::from_name(scope.name()), Some(scope));
        }
        assert_eq!(Scope::from_name("Nope"), None);
    }

    /// A picture 8 by 4, red on its left half and blue on its right,
    /// the alpha `cover` gives each.
    fn two_colors(cover: impl Fn(bool) -> u8) -> Vec<[u8; 4]> {
        (0..8 * 4)
            .map(|i| {
                let left = i % 8 < 4;
                let (r, b) = if left { (200, 40) } else { (40, 200) };
                [r, 40, b, cover(left)]
            })
            .collect()
    }

    const RED: [f32; 3] = [200.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0];
    const BLUE: [f32; 3] = [40.0 / 255.0, 40.0 / 255.0, 200.0 / 255.0];

    #[test]
    fn a_box_straddling_two_colors_reads_one_of_its_own_pixels() {
        let px = two_colors(|_| 0xff);
        // One pixel: the one under the point, at its column's middle.
        let r = under_pointer(&px, 8, 4, 0.3, 0.5, 0.0).unwrap();
        assert_eq!((r.color, r.selected, r.across), (RED, true, 2.5 / 8.0));
        // A box from x 2 to 6 over the edge, two red and three blue:
        // its mean is a purple neither half holds, and the reading is
        // the nearer of the two the box does, at that pixel's column.
        let r = under_pointer(&px, 8, 4, 4.5 / 8.0, 0.5, 2.0 / 8.0).unwrap();
        assert_eq!(r.color, BLUE, "{r:?}");
        assert!(r.selected);
        assert_eq!(r.across, 4.5 / 8.0, "the first blue in the box");
        // Its marks are on the cells the picture lit: the vectorscope's
        // ring on blue's, which a purple mean's would not be.
        let wheel = bins(
            Scope::Vector,
            &px.iter()
                .map(|p| [p[0], p[1], p[2]].map(|c| f32::from(c) / 255.0))
                .collect::<Vec<_>>(),
            8,
            4,
        );
        let mean: [f32; 3] = std::array::from_fn(|i| (2.0 * RED[i] + 3.0 * BLUE[i]) / 5.0);
        assert!(wheel[HIST + wheel_cell(r.color).unwrap()] > 0);
        assert_eq!(
            wheel[HIST + wheel_cell(mean).unwrap()],
            0,
            "the mean is off it"
        );
        // Clipped at the picture's edge, and never outside it.
        let r = under_pointer(&px, 8, 4, 1.0, 1.0, 0.5).unwrap();
        assert!(r.color == RED || r.color == BLUE);
        assert!((0.0..1.0).contains(&r.across));
        assert_eq!(under_pointer(&[], 0, 0, 0.5, 0.5, 0.0), None);
    }

    #[test]
    fn a_weighed_box_reads_its_covered_pixels_and_says_when_none_are() {
        // Only the red half selected: the box over the edge, though
        // mostly blue, reads red, the selection's color.
        let px = two_colors(|left| if left { 0xff } else { 0 });
        let r = under_pointer(&px, 8, 4, 4.5 / 8.0, 0.5, 2.0 / 8.0).unwrap();
        assert_eq!((r.color, r.selected), (RED, true), "{r:?}");
        assert!(r.across < 0.5, "a red pixel's column: {r:?}");
        // Nothing in the box selected: the plain color, on no trace.
        let r = under_pointer(&px, 8, 4, 7.5 / 8.0, 0.5, 1.0 / 8.0).unwrap();
        assert_eq!((r.color, r.selected), (BLUE, false), "{r:?}");
    }
}
