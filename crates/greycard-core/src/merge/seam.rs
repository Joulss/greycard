//! Where a panorama's frames hand over: a seam led around what the
//! frames disagree on, so a thing that moved between them, or stands
//! nearer than the rest and so shifted against it (the parallax of a
//! camera not turned about its lens), comes whole from one frame rather
//! than doubled across a blend.
//!
//! The frames are laid down one at a time on a reduced canvas. Each new
//! frame's overlap with what is down already is cut in two by a minimum
//! cut of a graph (Boykov and Jolly, "Interactive Graph Cuts for Optimal
//! Boundary and Region Segmentation", ICCV 2001; Kwatra and others,
//! "Graphcut Textures", SIGGRAPH 2003, for the seam's costs): every
//! pixel of the overlap a node, every two neighbors joined by what it
//! costs to put the seam between them, which is how much the two frames
//! disagree there, and a little for the seam's length. Where the overlap
//! meets what only the canvas holds, it is tied to the canvas; where it
//! meets what only the new frame holds, to the frame. A pull toward the
//! frame whose edge is farther (the seam a blend by distance would put
//! there) settles where the seam goes when nothing disagrees. The cut
//! is Dinic's maximum flow. Nothing ported.

use super::pano::{Rect, Warped};

/// What a seam's length costs a pixel, beside the disagreement.
pub const LENGTH: f32 = 0.05;
/// What each sensor pixel the seam sits nearer the edge of the frame
/// it takes costs, against a seam where the frames' edges are equally
/// far.
pub const PULL: f32 = 6e-4;
/// The brightness under which a disagreement is measured against this
/// floor, so the noise in a shadow does not read as one.
const FLOOR: f32 = 0.02;

/// The canvas as laid so far: whose each pixel is, its color, and how
/// far it is from that frame's edge.
pub struct Composite {
    pub width: usize,
    pub height: usize,
    /// The frame a pixel is from; none yet.
    pub owner: Vec<Option<u16>>,
    rgb: Vec<f32>,
    inside: Vec<f32>,
}

/// What a frame's cut came to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cut {
    /// Pixels the frame and the canvas both had.
    pub overlap: usize,
    /// Of those, the ones the frame took.
    pub taken: usize,
    /// The disagreement summed along the seam.
    pub cost: f64,
}

/// How much `a` and `b` (camera RGB) disagree: their difference over
/// their brightness.
#[inline]
fn disagreement(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = (0..3).map(|c| (a[c] - b[c]).abs()).sum();
    let s: f32 = (0..3).map(|c| 0.5 * (a[c] + b[c])).sum();
    (d / s.max(FLOOR)).min(2.0)
}

impl Composite {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            owner: vec![None; width * height],
            rgb: vec![0.0; width * height * 3],
            inside: vec![0.0; width * height],
        }
    }

    /// Frame `k`, warped onto this canvas over its rectangle (not a
    /// number where it is not), laid down: where only it is, it is
    /// taken; where the canvas has a frame already, the cut decides.
    pub fn add(&mut self, k: u16, tile: &Warped) -> Cut {
        let r = tile.rect;
        let w = self.width;
        let has = |i: usize, j: usize| -> bool {
            let p = &tile.rgb[(j * r.width + i) * 3..(j * r.width + i) * 3 + 3];
            p.iter().all(|v| v.is_finite())
        };
        // The overlap's nodes, by canvas pixel.
        let mut node = vec![usize::MAX; r.area()];
        let mut nodes: Vec<(usize, usize)> = Vec::new();
        for j in 0..r.height {
            for i in 0..r.width {
                let at = (r.y + j) * w + r.x + i;
                if !has(i, j) {
                    continue;
                }
                if self.owner[at].is_some() {
                    node[j * r.width + i] = nodes.len();
                    nodes.push((i, j));
                } else {
                    self.take(k, tile, i, j);
                }
            }
        }
        let mut cut = Cut {
            overlap: nodes.len(),
            ..Cut::default()
        };
        if nodes.is_empty() {
            return cut;
        }
        let tile_px =
            |i: usize, j: usize| &tile.rgb[(j * r.width + i) * 3..(j * r.width + i) * 3 + 3];
        let canvas_at = |i: usize, j: usize| (r.y + j) * w + r.x + i;
        let d: Vec<f32> = nodes
            .iter()
            .map(|&(i, j)| {
                let at = canvas_at(i, j);
                disagreement(&self.rgb[at * 3..at * 3 + 3], tile_px(i, j))
            })
            .collect();
        let s = nodes.len();
        let t = s + 1;
        let mut g = Flow::new(nodes.len() + 2);
        for (n, &(i, j)) in nodes.iter().enumerate() {
            let at = canvas_at(i, j);
            let (mine, theirs) = (tile.inside[j * r.width + i], self.inside[at]);
            // The pull: taking the frame where its edge is nearer than
            // the canvas's frame's costs, and the other way about.
            let mut to_source = PULL * (theirs - mine).max(0.0);
            let mut to_sink = PULL * (mine - theirs).max(0.0);
            let neighbors = [
                (i.wrapping_sub(1), j),
                (i + 1, j),
                (i, j.wrapping_sub(1)),
                (i, j + 1),
            ];
            for (ni, nj) in neighbors {
                let edge = LENGTH + 2.0 * d[n];
                if ni >= r.width || nj >= r.height {
                    // Past the tile: only the canvas can be there.
                    let (cx, cy) = (r.x as isize + ni as isize, r.y as isize + nj as isize);
                    if cx >= 0
                        && cy >= 0
                        && (cx as usize) < w
                        && (cy as usize) < self.height
                        && self.owner[cy as usize * w + cx as usize].is_some()
                    {
                        to_source += edge;
                    }
                    continue;
                }
                let m = node[nj * r.width + ni];
                if m != usize::MAX {
                    if m > n {
                        let e = LENGTH + d[n] + d[m];
                        g.edge(n, m, e, e);
                    }
                } else if has(ni, nj) {
                    // The frame's alone (taken above).
                    to_sink += edge;
                } else if self.owner[canvas_at(ni, nj)].is_some() {
                    to_source += edge;
                }
            }
            if to_source > 0.0 {
                g.edge(s, n, to_source, 0.0);
            }
            if to_sink > 0.0 {
                g.edge(n, t, to_sink, 0.0);
            }
        }
        g.max_flow(s, t);
        let canvas_side = g.reached(s);
        for (n, &(i, j)) in nodes.iter().enumerate() {
            if !canvas_side[n] {
                self.take(k, tile, i, j);
                cut.taken += 1;
            }
        }
        // The seam's cost, for the report.
        for (n, &(i, j)) in nodes.iter().enumerate() {
            for (ni, nj) in [(i + 1, j), (i, j + 1)] {
                if ni >= r.width || nj >= r.height {
                    continue;
                }
                let m = node[nj * r.width + ni];
                if m == usize::MAX {
                    continue;
                }
                if canvas_side[n] != canvas_side[m] {
                    cut.cost += f64::from(d[n] + d[m]);
                }
            }
        }
        cut
    }

    fn take(&mut self, k: u16, tile: &Warped, i: usize, j: usize) {
        let r = tile.rect;
        let at = (r.y + j) * self.width + r.x + i;
        let from = (j * r.width + i) * 3;
        self.owner[at] = Some(k);
        self.rgb[at * 3..at * 3 + 3].copy_from_slice(&tile.rgb[from..from + 3]);
        self.inside[at] = tile.inside[j * r.width + i];
    }

    /// Whether each pixel of `rect` is frame `k`'s, as a weight: one
    /// where it is, zero where it is not.
    pub fn mask(&self, k: u16, rect: Rect) -> Vec<f32> {
        let mut out = vec![0.0f32; rect.area()];
        for j in 0..rect.height {
            for i in 0..rect.width {
                let (x, y) = (rect.x + i, rect.y + j);
                if x < self.width && y < self.height && self.owner[y * self.width + x] == Some(k) {
                    out[j * rect.width + i] = 1.0;
                }
            }
        }
        out
    }
}

/// A graph for Dinic's maximum flow, each edge with its reverse.
struct Flow {
    first: Vec<usize>,
    next: Vec<usize>,
    to: Vec<usize>,
    cap: Vec<f32>,
}

const NONE: usize = usize::MAX;
const EPS: f32 = 1e-7;

impl Flow {
    fn new(n: usize) -> Self {
        Self {
            first: vec![NONE; n],
            next: Vec::new(),
            to: Vec::new(),
            cap: Vec::new(),
        }
    }

    /// An edge from `a` to `b` of capacity `ab`, and back of `ba`.
    fn edge(&mut self, a: usize, b: usize, ab: f32, ba: f32) {
        for (u, v, c) in [(a, b, ab), (b, a, ba)] {
            self.to.push(v);
            self.cap.push(c);
            self.next.push(self.first[u]);
            self.first[u] = self.to.len() - 1;
        }
    }

    fn levels(&self, s: usize, t: usize) -> Option<Vec<u32>> {
        let mut level = vec![u32::MAX; self.first.len()];
        level[s] = 0;
        let mut queue = std::collections::VecDeque::from([s]);
        while let Some(u) = queue.pop_front() {
            let mut e = self.first[u];
            while e != NONE {
                let v = self.to[e];
                if self.cap[e] > EPS && level[v] == u32::MAX {
                    level[v] = level[u] + 1;
                    queue.push_back(v);
                }
                e = self.next[e];
            }
        }
        (level[t] != u32::MAX).then_some(level)
    }

    fn max_flow(&mut self, s: usize, t: usize) -> f64 {
        let mut total = 0.0f64;
        while let Some(level) = self.levels(s, t) {
            let mut arc = self.first.clone();
            loop {
                // One augmenting path along the levels, found with a
                // stack, each node's arc pointer kept between paths.
                let mut path: Vec<usize> = Vec::new();
                let mut u = s;
                let found = loop {
                    if u == t {
                        break true;
                    }
                    let mut advanced = false;
                    while arc[u] != NONE {
                        let e = arc[u];
                        let v = self.to[e];
                        if self.cap[e] > EPS && level[v] == level[u] + 1 {
                            path.push(e);
                            u = v;
                            advanced = true;
                            break;
                        }
                        arc[u] = self.next[e];
                    }
                    if !advanced {
                        // A dead end: back up, and never come here again
                        // this phase.
                        let Some(e) = path.pop() else {
                            break false;
                        };
                        let back = self.to[e ^ 1];
                        arc[back] = self.next[arc[back]];
                        u = back;
                    }
                };
                if !found {
                    break;
                }
                let push = path.iter().map(|&e| self.cap[e]).fold(f32::MAX, f32::min);
                for &e in &path {
                    self.cap[e] -= push;
                    self.cap[e ^ 1] += push;
                }
                total += f64::from(push);
            }
        }
        total
    }

    /// What the source still reaches: its side of the cut.
    fn reached(&self, s: usize) -> Vec<bool> {
        let mut seen = vec![false; self.first.len()];
        seen[s] = true;
        let mut stack = vec![s];
        while let Some(u) = stack.pop() {
            let mut e = self.first[u];
            while e != NONE {
                let v = self.to[e];
                if self.cap[e] > EPS && !seen[v] {
                    seen[v] = true;
                    stack.push(v);
                }
                e = self.next[e];
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile of a flat grey scene over `rect`, the frame's edge `left`
    /// and `right` columns of the canvas; a bright square at `mover`
    /// when the frame saw it.
    fn tile(rect: Rect, mover: Option<(usize, usize, usize)>) -> Warped {
        let mut rgb = Vec::with_capacity(rect.area() * 3);
        let mut inside = Vec::with_capacity(rect.area());
        for j in 0..rect.height {
            for i in 0..rect.width {
                let (x, y) = (rect.x + i, rect.y + j);
                // A gentle texture both frames share.
                let base = 0.2 + 0.02 * ((x * 7 + y * 3) % 5) as f32;
                let v = match mover {
                    Some((mx, my, s)) if (mx..mx + s).contains(&x) && (my..my + s).contains(&y) => {
                        0.8
                    }
                    _ => base,
                };
                rgb.extend_from_slice(&[v, v, v]);
                // The frames' edges are their left and right: they
                // span the canvas's height.
                let d = i.min(rect.width - 1 - i);
                inside.push(8.0 * (d as f32 + 0.5));
            }
        }
        Warped { rect, rgb, inside }
    }

    /// Two frames overlapping over forty columns; the second saw a
    /// mover straddling the middle of the overlap, where a seam by
    /// distance would cut it in two. The cut takes it whole from one
    /// frame or leaves it whole to the other.
    #[test]
    fn the_seam_goes_round_a_mover() {
        let (w, h) = (120, 60);
        let mut c = Composite::new(w, h);
        let a = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: h,
        };
        let b = Rect {
            x: 40,
            y: 0,
            width: 80,
            height: h,
        };
        // Square 52..68 by 20..36: straddles column 60.
        let mover = (52, 20, 16);
        c.add(0, &tile(a, None));
        let cut = c.add(1, &tile(b, Some(mover)));
        assert_eq!(cut.overlap, 40 * h);
        let owners: Vec<Option<u16>> = (mover.1..mover.1 + mover.2)
            .flat_map(|y| (mover.0..mover.0 + mover.2).map(move |x| (x, y)))
            .map(|(x, y)| c.owner[y * w + x])
            .collect();
        assert!(
            owners.iter().all(|o| *o == owners[0]),
            "the mover is cut in two"
        );
        // Away from it, the seam still runs down the overlap's middle.
        let seam_at = |y: usize| (40..80).find(|&x| c.owner[y * w + x] == Some(1)).unwrap();
        for y in [2, 8, 50, 57] {
            assert!((57..=62).contains(&seam_at(y)), "row {y}: {}", seam_at(y));
        }
        // Only frame 0 left of the overlap, frame 1 right of it.
        assert!((0..h).all(|y| c.owner[y * w + 10] == Some(0) && c.owner[y * w + 110] == Some(1)));
    }

    #[test]
    fn the_flow_is_the_cut() {
        // s -> a (3), s -> b (2), a -> b (1), a -> t (2), b -> t (3).
        let mut g = Flow::new(4);
        let (s, a, b, t) = (0, 1, 2, 3);
        g.edge(s, a, 3.0, 0.0);
        g.edge(s, b, 2.0, 0.0);
        g.edge(a, b, 1.0, 0.0);
        g.edge(a, t, 2.0, 0.0);
        g.edge(b, t, 3.0, 0.0);
        assert!((g.max_flow(s, t) - 5.0).abs() < 1e-6);
        let side = g.reached(s);
        assert!(side[s] && !side[t]);
    }
}
