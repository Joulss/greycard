//! The filmstrip's arithmetic: each frame's cell its picture's shape
//! at the row's height, the cells side by side with a gap between and
//! the pad at either end; where a cell is, what a scroll shows, the
//! scroll that shows a frame, and the scroll that keeps the view still
//! when shapes come in after the layout.
//!
//! The window draws the cells where this puts them, asked through its
//! `strip-place` and the rest, so the numbers are tested here rather
//! than left in a binding.

use crate::grid::UNKNOWN_ASPECT;

/// The padding at the strip's ends and above its cells, logical
/// pixels: the theme's `space-3`.
pub const PAD: f32 = 12.0;
/// The gap between two cells: the theme's `space-2`, room to breathe
/// between pictures that have no padding of their own.
pub const GAP: f32 = 8.0;
/// How far a picture is inset from its cell's edges: the selection
/// ring's width, so the ring and the hover's ground hug the picture.
pub const RING: f32 = 2.0;
/// The pictures' height, the row's.
pub const PHOTO: f32 = 91.0;
/// The narrowest and the widest a picture is drawn, width over height:
/// a frame past either is fitted inside a cell of that shape, so a
/// sliver keeps a cell that can be clicked and a panorama does not
/// take the strip.
pub const NARROWEST: f32 = 0.5;
pub const WIDEST: f32 = 3.0;

/// A cell's width for a picture of `aspect`, height over width as the
/// grid's shapes are (zero or less for not known, which is laid out as
/// most cameras' 3:2 on its side): the picture's width at the row's
/// height, held to [`NARROWEST`] and [`WIDEST`], in whole pixels, and
/// the ring either side.
pub fn cell_width(aspect: f32) -> f32 {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        UNKNOWN_ASPECT
    };
    let wide = (1.0 / aspect).clamp(NARROWEST, WIDEST);
    (PHOTO * wide).round() + 2.0 * RING
}

/// The cells of a strip: each one's left edge along the row, from the
/// row's start, and its width.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Layout {
    lefts: Vec<f32>,
    widths: Vec<f32>,
}

impl Layout {
    /// `count` cells, row `i`'s picture of shape `aspect(i)`.
    pub fn new(count: usize, aspect: impl Fn(usize) -> f32) -> Self {
        let mut lefts = Vec::with_capacity(count);
        let mut widths = Vec::with_capacity(count);
        let mut x = PAD;
        for i in 0..count {
            let w = cell_width(aspect(i));
            lefts.push(x);
            widths.push(w);
            x += w + GAP;
        }
        Layout { lefts, widths }
    }

    /// How many cells.
    pub fn len(&self) -> usize {
        self.lefts.len()
    }

    /// Row `row`'s cell: its left edge and its width.
    pub fn place(&self, row: i32) -> Option<(f32, f32)> {
        let r = usize::try_from(row).ok()?;
        Some((*self.lefts.get(r)?, self.widths[r]))
    }

    /// The row's length: the last cell's right edge and the pad.
    pub fn length(&self) -> f32 {
        match (self.lefts.last(), self.widths.last()) {
            (Some(l), Some(w)) => l + w + PAD,
            _ => 2.0 * PAD,
        }
    }

    /// How far a view `view` wide scrolls along the row.
    pub fn max_scroll(&self, view: f32) -> f32 {
        (self.length() - view).max(0.0)
    }

    /// The rows a view `view` wide scrolled `offset` along shows any of,
    /// first and last; none for an empty strip.
    pub fn shown(&self, offset: f32, view: f32) -> Option<(i32, i32)> {
        let n = self.len();
        if n == 0 {
            return None;
        }
        // The last cell starting at or before the view's left edge,
        // or the next when the edge is in the gap after it.
        let k = self.lefts.partition_point(|&l| l <= offset).max(1) - 1;
        let first = if self.lefts[k] + self.widths[k] <= offset {
            (k + 1).min(n - 1)
        } else {
            k
        };
        // The last cell starting short of the right edge.
        let last = self
            .lefts
            .partition_point(|&l| l < offset + view)
            .max(1)
            .saturating_sub(1)
            .max(first);
        Some((first as i32, last as i32))
    }

    /// The least scroll from `offset` that shows row `row` whole in a
    /// view `view` wide, the pad beside it, held to the row's range.
    pub fn reveal(&self, offset: f32, view: f32, row: i32) -> f32 {
        let Some((left, width)) = self.place(row) else {
            return offset.clamp(0.0, self.max_scroll(view));
        };
        let right = left + width;
        let wanted = if left - PAD < offset {
            left - PAD
        } else if right + PAD > offset + view {
            right + PAD - view
        } else {
            offset
        };
        wanted.clamp(0.0, self.max_scroll(view))
    }
}

/// The scroll that keeps a view `view` wide still, scrolled `scroll`
/// along `before`, now the shapes have made it `after`. The selected
/// frame on screen holds its place, else the first frame on screen,
/// so a cell off the left edge that grows or shrinks moves nothing in
/// view; a selected frame that was whole is shown whole again if its
/// own cell grew past an edge. A strip at its start stays there.
pub fn held(before: &Layout, after: &Layout, scroll: f32, view: f32, selected: i32) -> f32 {
    let Some((first, last)) = before.shown(scroll, view) else {
        return 0.0;
    };
    let on_screen = (first..=last).contains(&selected);
    let whole = on_screen && before.reveal(scroll, view, selected) == scroll;
    let anchor = if on_screen { selected } else { first };
    let moved = match (before.place(anchor), after.place(anchor)) {
        (Some((was, _)), Some((now, _))) if scroll > 0.0 => scroll + now - was,
        _ => scroll,
    };
    if whole {
        after.reveal(moved, view, selected)
    } else {
        moved.clamp(0.0, after.max_scroll(view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANDSCAPE: f32 = 2.0 / 3.0;
    const PORTRAIT: f32 = 1.5;

    #[test]
    fn a_cell_takes_its_pictures_shape_at_the_rows_height() {
        // 3:2 on its side: 91 high is 136.5 wide, 137 in whole pixels,
        // and the ring either side.
        assert_eq!(cell_width(LANDSCAPE), 137.0 + 4.0);
        // On its end: 61.
        assert_eq!(cell_width(PORTRAIT), 61.0 + 4.0);
        assert_eq!(cell_width(1.0), 91.0 + 4.0);
        // Not known yet: laid out as the landscape.
        assert_eq!(cell_width(0.0), cell_width(LANDSCAPE));
        assert_eq!(cell_width(f32::NAN), cell_width(LANDSCAPE));
        // A panorama and a sliver are held.
        assert_eq!(cell_width(0.1), 273.0 + 4.0);
        assert_eq!(cell_width(10.0), 46.0 + 4.0);
    }

    #[test]
    fn the_cells_run_from_the_pad_with_a_gap_between() {
        let shapes = [LANDSCAPE, PORTRAIT, 1.0, 0.0];
        let l = Layout::new(shapes.len(), |i| shapes[i]);
        assert_eq!(l.place(0), Some((12.0, 141.0)));
        assert_eq!(l.place(1), Some((12.0 + 141.0 + 8.0, 65.0)));
        assert_eq!(l.place(2), Some((161.0 + 65.0 + 8.0, 95.0)));
        assert_eq!(l.place(3), Some((234.0 + 95.0 + 8.0, 141.0)));
        assert_eq!(l.place(4), None);
        assert_eq!(l.place(-1), None);
        assert_eq!(l.length(), 337.0 + 141.0 + 12.0);
        // Empty: the pads alone.
        let empty = Layout::new(0, |_| 0.0);
        assert_eq!(empty.length(), 24.0);
        assert_eq!(empty.shown(0.0, 1500.0), None);
        assert_eq!(empty.reveal(40.0, 1500.0, 0), 0.0);
    }

    #[test]
    fn a_scroll_shows_the_cells_it_touches() {
        // Portraits, 65 wide at a pitch of 73 from 12.
        let l = Layout::new(100, |_| PORTRAIT);
        assert_eq!(l.shown(0.0, 1500.0), Some((0, 20)));
        // The left edge in the gap after row 1: row 2 is the first.
        assert_eq!(l.shown(12.0 + 73.0 + 66.0, 300.0).unwrap().0, 2);
        // On row 1's last pixel: still row 1.
        assert_eq!(l.shown(12.0 + 73.0 + 64.5, 300.0).unwrap().0, 1);
        // A right edge on a cell's left edge does not show that cell.
        let left_of_3 = l.place(3).unwrap().0;
        assert_eq!(l.shown(0.0, left_of_3).unwrap().1, 2);
        assert_eq!(l.shown(0.0, left_of_3 + 1.0).unwrap().1, 3);
        // Past the end: the last.
        assert_eq!(l.shown(1e6, 1500.0), Some((99, 99)));
    }

    #[test]
    fn a_frame_at_the_right_edge_is_revealed_with_the_pad_beside_it() {
        let shapes = |i: usize| {
            if i.is_multiple_of(3) {
                PORTRAIT
            } else {
                LANDSCAPE
            }
        };
        let l = Layout::new(40, shapes);
        let view = 1000.0;
        // Row 8 runs past the right edge of the view at the start.
        let (left, width) = l.place(8).unwrap();
        assert!(left < view && left + width > view);
        let to = l.reveal(0.0, view, 8);
        assert_eq!(to, left + width + PAD - view);
        // Shown, it stays where it is.
        assert_eq!(l.reveal(to, view, 8), to);
        // Off the left: brought in with the pad before it.
        assert_eq!(l.reveal(to, view, 1), l.place(1).unwrap().0 - PAD);
        // The last frame: the strip's end.
        assert_eq!(l.reveal(0.0, view, 39), l.max_scroll(view));
        assert_eq!(l.reveal(0.0, view, 0), 0.0);
    }

    #[test]
    fn a_late_shape_left_of_the_view_keeps_the_view_steady() {
        // Nothing known: every cell the 3:2 landscape's.
        let before = Layout::new(60, |_| 0.0);
        let view = 1200.0;
        let scroll = before.place(30).unwrap().0 - 20.0;
        let (first, last) = before.shown(scroll, view).unwrap();
        // Rows 3 and 10, far left of the view, come in as portraits.
        let after = Layout::new(60, |i| if i == 3 || i == 10 { PORTRAIT } else { 0.0 });
        // Selected off screen: the first frame on screen holds.
        let to = held(&before, &after, scroll, view, 0);
        let gone = 2.0 * (cell_width(0.0) - cell_width(PORTRAIT));
        assert_eq!(to, scroll - gone);
        for row in first..=last {
            let x = |l: &Layout, s: f32| l.place(row).unwrap().0 - s;
            assert_eq!(x(&after, to), x(&before, scroll), "row {row} moved");
        }
        // Selected on screen: it holds, and so does the rest.
        let to = held(&before, &after, scroll, view, first + 2);
        assert_eq!(to, scroll - gone);
        // A strip at its start stays there.
        assert_eq!(held(&before, &after, 0.0, view, 0), 0.0);
    }

    #[test]
    fn a_whole_selected_frame_that_grows_past_the_edge_is_shown_whole() {
        // Portraits all; the selected frame is the last whole one on
        // screen and comes in as a panorama.
        let before = Layout::new(60, |_| PORTRAIT);
        let view = 700.0;
        let scroll = 0.0;
        let (_, last) = before.shown(scroll, view).unwrap();
        let sel = last - 1;
        assert_eq!(before.reveal(scroll, view, sel), scroll, "whole");
        let after = Layout::new(60, |i| if i as i32 == sel { 0.1 } else { PORTRAIT });
        let to = held(&before, &after, scroll, view, sel);
        assert_eq!(to, after.reveal(scroll, view, sel));
        assert!(to > 0.0);
        // Cut at the edge before, it is left where it was.
        let cut = last;
        assert_ne!(before.reveal(scroll, view, cut), scroll);
        let after = Layout::new(60, |i| if i as i32 == cut { 0.1 } else { PORTRAIT });
        assert_eq!(held(&before, &after, scroll, view, cut), scroll);
    }

    #[test]
    fn a_strip_of_twenty_thousand_is_laid_out_quickly() {
        let started = std::time::Instant::now();
        let l = Layout::new(20_000, |i| {
            if i.is_multiple_of(4) {
                PORTRAIT
            } else {
                LANDSCAPE
            }
        });
        assert_eq!(l.len(), 20_000);
        assert!(l.shown(l.max_scroll(1500.0), 1500.0).unwrap().1 == 19_999);
        assert!(started.elapsed().as_millis() < 50);
    }
}
