//! Deghosting: what moved in the scene between a bracket's frames
//! taken from the reference alone.
//!
//! The reference is the truth. Once a frame is on the reference's
//! scale, a sample that differs from the reference's by more than
//! [`SIGMAS`] times the deviation of the difference (the root of the
//! sum of the two samples' variances, from the merge's [`Noise`]) is
//! marked. The marks are closed (grown, then shrunk, [`CLOSE`] pixels)
//! so a mover's speckled inside is whole, then blurred ([`BLUR`]) into
//! a soft weight, doubled, held to one, and never under the closed mark
//! itself: a region of marks is taken out whole and the weight fades
//! over a few pixels past its edge, while a lone mark (the noise's own
//! four-sigma tail) takes out its own sample and under a tenth of its
//! neighbors'. The frame is added with its weight times one less that.
//!
//! Where the reference clipped it cannot judge. There every frame that
//! holds the pixel is kept in a ledger, and once all are in, the
//! darkest frame that agrees with every other (by the same test)
//! judges: since it agrees with all, all are kept. Where no frame
//! agrees with all the others, the darkest alone is kept: its sample
//! is set in the sums in place of them all. The ledger is the frames'
//! samples at the reference's clipped pixels only, which a bracket's
//! darker frames hold and its brighter ones mostly do not: a second
//! small pass over those pixels, rather than every frame held to
//! compare them in darkest-first order.
//!
//! What that costs: 16 bytes a ledger sample (the weight is worked out
//! again from the value) and 20 a pending one, so a 45 MP reference
//! clipped over 40% with two darker frames holding it would want about
//! 0.6 GB of ledger, built whether or not anything moves. Both are held
//! to [`BUDGET`] together: past it the ledger ends (the reference's
//! clipped pixels past its end merge as without deghosting) and a
//! frame whose pending would not fit is kept whole should a later frame
//! move, each said once in the log; [`Ghosts::peak`] says what was
//! held.
//!
//! When it runs is the caller's: always, or only once any frame was
//! found to have moved ([`Ghosts::new`]'s `running`). Until it runs,
//! frames go into the sums whole, exactly as without it, and what
//! each would have had taken out is kept aside, sparse, so a move
//! found at a later frame takes it out of the earlier ones too
//! ([`Ghosts::run`]); a bracket where nothing moved merges to the bit
//! as it would have with no deghosting at all.
//!
//! Nothing here is ported from another project.

use rayon::prelude::*;

use super::{Accumulator, Frame, Noise, grow};

/// A sample this many deviations of the difference from the
/// reference's is marked as moved.
pub const SIGMAS: f32 = 4.0;
/// The marks are closed by this many pixels.
pub const CLOSE: usize = 2;
/// The closed marks are blurred by a box of this radius, twice.
pub const BLUR: usize = 2;
/// The most the ledger and the pending lists hold together, in bytes.
/// Past it the ledger stops growing (the reference's clipped pixels
/// past its end are merged as with no deghosting), and a frame whose
/// pending would not fit is kept whole should a later frame move.
pub const BUDGET: usize = 512 << 20;
/// A ledger entry's bytes.
const HELD: usize = std::mem::size_of::<Held>();
/// A pending entry's bytes.
const PENDING: usize = std::mem::size_of::<(u32, f32, [f32; 3])>();
/// A pixel's weight left at or under this fraction of what it held
/// before its take-out is rounding, not a frame: the pixel is emptied.
/// The rounding of a few subtractions is some 1e-7 of the most the sum
/// held, which is what it held before them, all weights being
/// positive; a frame left a 4096th of a bright mover's weight is kept.
const TAKEN: f32 = 1e-5;

/// What a frame is judged against: the reference's samples, its clip
/// mask, and its noise.
pub struct Judge<'a> {
    pub rgb: &'a [f32],
    pub mask: &'a [bool],
    pub noise: Noise,
}

/// A frame's sample in the ledger: where the reference clipped and the
/// frame did not, its value on the reference's scale. Its weight in
/// the sums is worked out again from the value ([`Seen::weight`]).
#[derive(Clone, Copy)]
struct Held {
    /// The pixel's index in the frame.
    at: u32,
    value: [f32; 3],
}

/// One frame's part of the deghosting.
struct Seen {
    /// The caller's name for the frame.
    id: usize,
    exposure: f64,
    noise: Noise,
    /// Whether its pending did not fit the budget: it is kept whole
    /// should a later frame move.
    whole: bool,
    /// The rejection's weight summed over the pixels the frame holds:
    /// what it takes once it runs.
    rejected: f64,
    /// Before it runs, what it would take out of the sums: each pixel's
    /// index, its weight, and its three weighted samples.
    pending: Vec<(u32, f32, [f32; 3])>,
    /// Its samples where the reference clipped.
    ledger: Vec<Held>,
}

/// The deghosting's state over one merge: each frame added through
/// [`Ghosts::add`], the reference through the accumulator's own add.
pub struct Ghosts {
    width: usize,
    height: usize,
    running: bool,
    frames: Vec<Seen>,
    /// The bytes the ledger and the pending lists may hold together.
    budget: usize,
    /// The pixels from this index on are past the ledger's end.
    cutoff: usize,
    /// The most the ledger and the pending lists held, in bytes.
    peak: (usize, usize),
    /// Whether the budget was said to be reached.
    said: bool,
}

impl Seen {
    /// A ledger sample's weight in the sums, as the accumulator gave
    /// it: worked out from its value rather than kept, to the rounding
    /// of the value.
    fn weight(&self, held: &Held) -> f32 {
        let e = self.exposure;
        let p = held.value.map(|v| (f64::from(v) * e) as f32);
        (e * e / f64::from(self.noise.variance(super::luma(&p)))) as f32
    }
}

impl Ghosts {
    /// For a merge of `width` by `height`; `running` from the start,
    /// or only once [`Ghosts::run`] says so.
    pub fn new(width: usize, height: usize, running: bool) -> Self {
        Self {
            width,
            height,
            running,
            frames: Vec::new(),
            budget: BUDGET,
            cutoff: width * height,
            peak: (0, 0),
            said: false,
        }
    }

    /// [`Ghosts::new`] with another budget than [`BUDGET`].
    pub fn with_budget(mut self, bytes: usize) -> Self {
        self.budget = bytes;
        self
    }

    /// The most the ledger and the pending lists held over the merge,
    /// in bytes: the ledger's, then the pending's.
    pub fn peak(&self) -> (usize, usize) {
        self.peak
    }

    /// What the ledger and the pending lists hold now, in bytes.
    fn held(&self) -> (usize, usize) {
        self.frames.iter().fold((0, 0), |(l, p), s| {
            (l + s.ledger.len() * HELD, p + s.pending.len() * PENDING)
        })
    }

    /// The budget said to be reached, once.
    fn say(&mut self, what: &str) {
        if !self.said {
            self.said = true;
            log::info!(
                "merge: deghosting's budget of {} MB reached: {what}",
                self.budget >> 20
            );
        }
    }

    /// Whether it is taking what moved out.
    pub fn running(&self) -> bool {
        self.running
    }

    /// From now on, what moved is taken out, and what the frames added
    /// so far would have had taken out is taken out of `acc` now.
    pub fn run(&mut self, acc: &mut Accumulator) {
        if self.running {
            return;
        }
        self.running = true;
        // The pending lists are each in pixel order: walk them
        // together, so a pixel's take-out is judged against what it
        // held before any of it.
        let mut at = vec![0usize; self.frames.len()];
        let mut parts: Vec<(f32, [f32; 3])> = Vec::new();
        loop {
            let next = self
                .frames
                .iter()
                .zip(&at)
                .filter_map(|(s, &k)| s.pending.get(k).map(|p| p.0))
                .min();
            let Some(pixel) = next else {
                break;
            };
            parts.clear();
            for (seen, k) in self.frames.iter().zip(&mut at) {
                if let Some(&(i, wt, s)) = seen.pending.get(*k)
                    && i == pixel
                {
                    parts.push((wt, s));
                    *k += 1;
                }
            }
            take_out(acc, pixel as usize, &parts);
        }
        for seen in &mut self.frames {
            seen.pending = Vec::new();
            if seen.whole {
                seen.rejected = 0.0;
            }
        }
    }

    /// `frame`, named `id`, judged against `judge` and added to `acc`:
    /// what moved taken out if running, kept aside if not.
    pub fn add(&mut self, acc: &mut Accumulator, frame: &Frame<'_>, judge: &Judge<'_>, id: usize) {
        let (w, h) = (self.width, self.height);
        assert_eq!((acc.width, acc.height), (w, h));
        let reject = rejection(frame, judge, w, h);
        // The ledger first, to what the budget leaves: past it, the
        // ledger ends where this frame's would, for every frame.
        let (held, _) = self.held();
        let room = self.budget.saturating_sub(held) / HELD;
        let (cutoff, short) = ledger_end(frame, judge, w, self.cutoff, room);
        if short {
            self.cutoff = cutoff;
            for s in &mut self.frames {
                s.ledger.retain(|h| (h.at as usize) < cutoff);
            }
            self.say("the reference's clipped pixels past the ledger merge as without deghosting");
        }
        let ledger = ledger(frame, judge, w, self.cutoff);
        let mut seen = Seen {
            id,
            exposure: frame.exposure,
            noise: frame.noise,
            whole: false,
            rejected: 0.0,
            pending: Vec::new(),
            ledger,
        };
        if self.running {
            let keep: Vec<f32> = reject.par_iter().map(|r| 1.0 - r).collect();
            acc.add_kept(frame, Some(&keep));
        } else {
            let (ledger, pending_now) = self.held();
            let count = pending_count(frame, &reject, w);
            if ledger + pending_now + seen.ledger.len() * HELD + count * PENDING > self.budget {
                seen.whole = true;
                self.say("a frame is kept whole should a later frame move");
            } else {
                seen.pending = pending(frame, &reject, w);
            }
            acc.add(frame);
        }
        seen.rejected = reject.par_iter().map(|&r| f64::from(r)).sum();
        self.frames.push(seen);
        let (l, p) = self.held();
        self.peak = (self.peak.0.max(l), self.peak.1.max(p));
    }

    /// The ledger judged, once every frame is in: where the reference
    /// clipped and no frame agrees with all the others, all but the
    /// darkest taken back out of `acc`. Each frame's share of its
    /// pixels rejected, by the id it was added under; none for a
    /// deghosting that never ran.
    pub fn finish(self, acc: &mut Accumulator) -> Vec<(usize, Option<f64>)> {
        let n = (self.width * self.height).max(1) as f64;
        if !self.running {
            return self.frames.iter().map(|s| (s.id, None)).collect();
        }
        let mut extra = vec![0usize; self.frames.len()];
        // The frames' ledgers are each in pixel order: walk them
        // together, a pixel at a time.
        let mut at = vec![0usize; self.frames.len()];
        let mut here: Vec<(usize, Held, f32)> = Vec::new();
        loop {
            let next = self
                .frames
                .iter()
                .zip(&at)
                .filter_map(|(s, &k)| s.ledger.get(k).map(|h| h.at))
                .min();
            let Some(pixel) = next else {
                break;
            };
            here.clear();
            for (f, s) in self.frames.iter().enumerate() {
                if let Some(h) = s.ledger.get(at[f])
                    && h.at == pixel
                {
                    here.push((f, *h, s.weight(h)));
                    at[f] += 1;
                }
            }
            if here.len() < 2 {
                continue;
            }
            let judged = here
                .iter()
                .any(|(_, a, wa)| here.iter().all(|(_, b, wb)| agree((a, *wa), (b, *wb))));
            if judged {
                continue;
            }
            let darkest = here
                .iter()
                .min_by(|a, b| {
                    self.frames[a.0]
                        .exposure
                        .total_cmp(&self.frames[b.0].exposure)
                })
                .map(|(f, _, _)| *f)
                .expect("two or more");
            let i = pixel as usize;
            if acc.weight[i] < 0.0 {
                continue;
            }
            // Where the reference clipped, the frames that hold the
            // pixel are all here, so the darkest's sample is set
            // rather than the rest subtracted: a subtraction in f32
            // of weights far apart (a black sample's is the inverse of
            // the read noise alone) leaves rounding, or even zero or
            // below zero, where the darkest's weight should be. The
            // sums keep their meaning, the darkest's weight and its
            // weighted sample, so a frame added after weighs fairly.
            let (_, h, wd) = here.iter().find(|(f, _, _)| *f == darkest).expect("here");
            for (sum, v) in acc.sum[i * 3..i * 3 + 3].iter_mut().zip(h.value) {
                *sum = wd * v;
            }
            acc.weight[i] = *wd;
            for (f, _, _) in &here {
                if *f != darkest {
                    extra[*f] += 1;
                }
            }
        }
        self.frames
            .iter()
            .zip(extra)
            .map(|(s, e)| (s.id, Some((s.rejected + e as f64) / n)))
            .collect()
    }
}

/// Every frame's `parts` (a weight and its weighted samples) taken out
/// of `acc`'s pixel `i`, as [`pending`] says the accumulator added
/// them. The subtraction
/// rounds, and where the sums held little else it leaves a residual
/// weight, or one below zero, that the merge would divide by or take
/// for a pixel set whole: a weight left at or under [`TAKEN`] of what
/// the pixel held before empties it, so a weight of zero means no frame holds it and
/// a later frame (the reference, which holds every pixel it judged)
/// or the fill sets it.
fn take_out(acc: &mut Accumulator, i: usize, parts: &[(f32, [f32; 3])]) {
    let before = acc.weight[i];
    if before < 0.0 {
        return;
    }
    let mut left = before;
    let sums = &mut acc.sum[i * 3..i * 3 + 3];
    for (wt, s) in parts {
        left -= wt;
        for (sum, s) in sums.iter_mut().zip(s) {
            *sum -= s;
        }
    }
    // Not over: under, or not a number from an infinite weight.
    if left.partial_cmp(&(TAKEN * before)) != Some(std::cmp::Ordering::Greater) {
        left = 0.0;
        sums.fill(0.0);
    }
    acc.weight[i] = left;
}

/// Whether two of the ledger's samples agree: every channel within
/// [`SIGMAS`] deviations of their difference, a sample's variance on
/// the reference's scale being the inverse of its weight.
fn agree((a, wa): (&Held, f32), (b, wb): (&Held, f32)) -> bool {
    let sigma = (1.0 / wa + 1.0 / wb).sqrt();
    (0..3).all(|c| (a.value[c] - b.value[c]).abs() <= SIGMAS * sigma)
}

/// The frame's weight a pixel in the sums, as the accumulator gives
/// it; none where it adds nothing.
#[inline]
fn weight_of(frame: &Frame<'_>, i: usize) -> Option<f32> {
    if frame.mask[i] {
        return None;
    }
    let p = &frame.rgb[i * 3..i * 3 + 3];
    if !p.iter().all(|v| v.is_finite()) {
        return None;
    }
    let e = frame.exposure;
    Some((e * e / f64::from(frame.noise.variance(super::luma(p)))) as f32)
}

/// How much of each pixel of `frame` moved, 0 to 1: the marks where
/// both it and the reference hold the pixel, closed, blurred, doubled,
/// held to one, and no less than the closed mark; nothing where the
/// reference clipped, which the ledger judges.
pub(crate) fn rejection(frame: &Frame<'_>, judge: &Judge<'_>, w: usize, h: usize) -> Vec<f32> {
    let e = frame.exposure;
    let e32 = e as f32;
    let marks: Vec<bool> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            if judge.mask[i] || frame.mask[i] {
                return false;
            }
            let p = &frame.rgb[i * 3..i * 3 + 3];
            let r = &judge.rgb[i * 3..i * 3 + 3];
            (0..3).any(|c| {
                let (v, rv) = (p[c], r[c]);
                if !(v.is_finite() && rv.is_finite()) {
                    return false;
                }
                let var = frame.noise.variance(v) / (e32 * e32) + judge.noise.variance(rv);
                (v / e32 - rv).abs() > SIGMAS * var.sqrt()
            })
        })
        .collect();
    let grown = grow(&marks, w, h, CLOSE);
    drop(marks);
    let outside: Vec<bool> = grown.par_iter().map(|m| !m).collect();
    drop(grown);
    let shrunk = grow(&outside, w, h, CLOSE);
    drop(outside);
    let closed: Vec<f32> = shrunk
        .par_iter()
        .map(|o| if *o { 0.0 } else { 1.0 })
        .collect();
    drop(shrunk);
    let blurred = box_blur(&box_blur(&closed, w, h, BLUR), w, h, BLUR);
    blurred
        .into_par_iter()
        .zip(closed)
        .enumerate()
        .map(|(i, (b, m))| {
            if judge.mask[i] || frame.mask[i] {
                0.0
            } else {
                (2.0 * b).min(1.0).max(m)
            }
        })
        .collect()
}

/// `plane` blurred by a box `2 r + 1` wide each way, the box shortened
/// at the edges.
fn box_blur(plane: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut rows = vec![0.0f32; w * h];
    rows.par_chunks_mut(w)
        .zip(plane.par_chunks(w))
        .for_each(|(out, src)| {
            for (x, o) in out.iter_mut().enumerate() {
                let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
                *o = src[a..=b].iter().sum::<f32>() / (b - a + 1) as f32;
            }
        });
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, o)| {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for yy in a..=b {
            for (v, s) in o.iter_mut().zip(&rows[yy * w..(yy + 1) * w]) {
                *v += *s;
            }
        }
        let n = (b - a + 1) as f32;
        o.iter_mut().for_each(|v| *v /= n);
    });
    out
}

/// What `reject` would take of `frame` out of the sums, where it would
/// take anything.
fn pending(frame: &Frame<'_>, reject: &[f32], w: usize) -> Vec<(u32, f32, [f32; 3])> {
    let e = frame.exposure;
    reject
        .par_chunks(w)
        .enumerate()
        .flat_map_iter(|(y, row)| {
            row.iter().enumerate().filter_map(move |(x, &r)| {
                if r <= 0.0 {
                    return None;
                }
                let i = y * w + x;
                let wt = weight_of(frame, i)?;
                let p = &frame.rgb[i * 3..i * 3 + 3];
                // As the accumulator adds them, so taking them out
                // takes out what went in.
                let full = [0, 1, 2].map(|c| wt * (f64::from(p[c]) / e) as f32);
                let kept = wt * (1.0 - r);
                let gone = wt - kept;
                Some((i as u32, gone, full.map(|s| s * (gone / wt))))
            })
        })
        .collect()
}

/// `frame`'s samples where the reference clipped and it did not, in
/// pixel order.
fn ledger(frame: &Frame<'_>, judge: &Judge<'_>, w: usize, cutoff: usize) -> Vec<Held> {
    let e = frame.exposure;
    judge.mask[..cutoff]
        .par_chunks(w)
        .enumerate()
        .flat_map_iter(|(y, row)| {
            row.iter().enumerate().filter_map(move |(x, &clipped)| {
                let i = y * w + x;
                if !clipped || weight_of(frame, i).is_none() {
                    return None;
                }
                let p = &frame.rgb[i * 3..i * 3 + 3];
                Some(Held {
                    at: i as u32,
                    value: [0, 1, 2].map(|c| (f64::from(p[c]) / e) as f32),
                })
            })
        })
        .collect()
}

/// Where `frame`'s ledger ends: before `cutoff`, at the end of the
/// last row whose entries fit `room`, and whether that is short of
/// `cutoff`. Counted before anything is kept, so the ledger never holds
/// more than the budget even for a moment.
fn ledger_end(
    frame: &Frame<'_>,
    judge: &Judge<'_>,
    w: usize,
    cutoff: usize,
    room: usize,
) -> (usize, bool) {
    let rows: Vec<usize> = judge.mask[..cutoff]
        .par_chunks(w)
        .enumerate()
        .map(|(y, row)| {
            row.iter()
                .enumerate()
                .filter(|&(x, &clipped)| clipped && weight_of(frame, y * w + x).is_some())
                .count()
        })
        .collect();
    let mut total = 0;
    for (y, n) in rows.iter().enumerate() {
        if total + n > room {
            return (y * w, true);
        }
        total += n;
    }
    (cutoff, false)
}

/// How many pixels `reject` would take anything from.
fn pending_count(frame: &Frame<'_>, reject: &[f32], w: usize) -> usize {
    reject
        .par_chunks(w)
        .enumerate()
        .map(|(y, row)| {
            row.iter()
                .enumerate()
                .filter(|&(x, &r)| r > 0.0 && weight_of(frame, y * w + x).is_some())
                .count()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::clip_mask;

    const W: usize = 96;
    const H: usize = 64;
    const NOISE: Noise = Noise {
        shot: 2e-5,
        read: 6e-8,
    };

    /// A textured scene in the reference's units, under clip.
    fn scene(x: usize, y: usize, c: usize) -> f32 {
        let t = 0.5 + 0.4 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
        0.05 + 0.4 * t * [0.6, 1.0, 0.8][c]
    }

    /// `light` at `e` with the noise drawn from `seed`, clipped at one.
    fn frame(e: f32, seed: u64, light: impl Fn(usize, usize, usize) -> f32) -> Vec<f32> {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut out = Vec::with_capacity(W * H * 3);
        for y in 0..H {
            for x in 0..W {
                for c in 0..3 {
                    let v = light(x, y, c) * e;
                    let (u1, u2) = (rnd().max(1e-12), rnd());
                    let g = ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32;
                    out.push((v + g * NOISE.variance(v).sqrt()).min(1.0));
                }
            }
        }
        out
    }

    /// The square a mover is painted over.
    fn mover(x: usize, y: usize) -> bool {
        (40..60).contains(&x) && (20..40).contains(&y)
    }

    fn merge(frames: &[(f32, &[f32])], reference: &[f32], ghosts: Option<bool>) -> Merged {
        let g = ghosts.map(|running| Ghosts::new(W, H, running));
        merge_by(frames, reference, g).0
    }

    /// A merge's samples and what each frame had rejected.
    type Merged = (Vec<f32>, Vec<(usize, Option<f64>)>);

    /// [`merge`] by `g`, and what `g` held at most.
    fn merge_by(
        frames: &[(f32, &[f32])],
        reference: &[f32],
        mut g: Option<Ghosts>,
    ) -> (Merged, (usize, usize)) {
        let ref_mask = clip_mask(reference, W, H);
        let masks: Vec<Vec<bool>> = frames.iter().map(|(_, f)| clip_mask(f, W, H)).collect();
        let mut acc = Accumulator::new(W, H);
        for (k, ((e, rgb), mask)) in frames.iter().zip(&masks).enumerate() {
            let frame = Frame {
                rgb,
                mask,
                exposure: f64::from(*e),
                noise: NOISE,
            };
            match g.as_mut() {
                Some(g) => g.add(
                    &mut acc,
                    &frame,
                    &Judge {
                        rgb: reference,
                        mask: &ref_mask,
                        noise: NOISE,
                    },
                    k,
                ),
                None => acc.add(&frame),
            }
        }
        acc.add(&Frame {
            rgb: reference,
            mask: &ref_mask,
            exposure: 1.0,
            noise: NOISE,
        });
        let peak = g.as_ref().map_or((0, 0), Ghosts::peak);
        let said = g.map_or(Vec::new(), |g| g.finish(&mut acc));
        ((acc.finish().data, said), peak)
    }

    /// A mover painted into one frame is taken from the reference, and
    /// the frame away from it is averaged as with no deghosting.
    #[test]
    fn a_mover_is_taken_from_the_reference_and_the_rest_averaged() {
        let reference = frame(1.0, 1, scene);
        let dark = frame(
            0.25,
            2,
            |x, y, c| if mover(x, y) { 0.9 } else { scene(x, y, c) },
        );
        let (plain, _) = merge(&[(0.25, &dark)], &reference, None);
        let (judged, said) = merge(&[(0.25, &dark)], &reference, Some(true));
        let mut same = 0;
        let mut away = 0;
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) * 3;
                if mover(x, y) {
                    for c in 0..3 {
                        let (got, want) = (judged[i + c], reference[i + c]);
                        assert!((got - want).abs() < 1e-5, "{x},{y}: {got} against {want}");
                    }
                } else if !(32..68).contains(&x) || !(12..48).contains(&y) {
                    away += 1;
                    same += usize::from(judged[i..i + 3] == plain[i..i + 3]);
                }
            }
        }
        assert!(same as f64 > 0.95 * away as f64, "{same} of {away}");
        let rejected = said[0].1.unwrap();
        let square = 400.0 / (W * H) as f64;
        assert!(rejected > square && rejected < 2.0 * square, "{rejected}");
    }

    /// Not running, the frames go in whole, to the bit; what they would
    /// lose is kept aside, and taken out when it runs.
    #[test]
    fn not_running_merges_to_the_bit_and_running_later_takes_it_out() {
        let reference = frame(1.0, 1, scene);
        let dark = frame(
            0.25,
            2,
            |x, y, c| if mover(x, y) { 0.9 } else { scene(x, y, c) },
        );
        let bright = frame(2.0, 3, scene);
        let frames = [(0.25, dark.as_slice()), (2.0, bright.as_slice())];
        let (plain, _) = merge(&frames, &reference, None);
        let (idle, said) = merge(&frames, &reference, Some(false));
        assert!(
            plain
                .iter()
                .zip(&idle)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert!(said.iter().all(|(_, r)| r.is_none()));

        // Run after both are in: the mover goes as it would have.
        let ref_mask = clip_mask(&reference, W, H);
        let judge = Judge {
            rgb: &reference,
            mask: &ref_mask,
            noise: NOISE,
        };
        let mut acc = Accumulator::new(W, H);
        let mut g = Ghosts::new(W, H, false);
        for (k, (e, rgb)) in frames.iter().enumerate() {
            let mask = clip_mask(rgb, W, H);
            let frame = Frame {
                rgb,
                mask: &mask,
                exposure: f64::from(*e),
                noise: NOISE,
            };
            g.add(&mut acc, &frame, &judge, k);
        }
        g.run(&mut acc);
        acc.add(&Frame {
            rgb: &reference,
            mask: &ref_mask,
            exposure: 1.0,
            noise: NOISE,
        });
        let said = g.finish(&mut acc);
        let late = acc.finish().data;
        let (early, _) = merge(&frames, &reference, Some(true));
        for (a, b) in late.iter().zip(&early) {
            assert!((a - b).abs() <= 1e-5 * b.abs().max(1e-3), "{a} against {b}");
        }
        assert!(said[0].1.unwrap() > 0.0);
    }

    /// Where the reference clipped and two darker frames disagree (one
    /// holds a mover), the darkest alone stands; where they agree both
    /// are kept.
    #[test]
    fn where_the_reference_clipped_the_darkest_stands_for_it() {
        // Bright enough to clip the reference everywhere.
        let light = |x: usize, y: usize, c: usize| 2.0 * scene(x, y, c) + 1.0;
        let reference = frame(1.0, 1, light);
        assert!(clip_mask(&reference, W, H).iter().all(|m| *m));
        let darkest = frame(0.25, 2, light);
        let middle = frame(0.4, 3, |x, y, c| {
            if mover(x, y) {
                0.05
            } else {
                light(x, y, c) * 1.0
            }
        });
        let frames = [(0.25, darkest.as_slice()), (0.4, middle.as_slice())];
        let (judged, said) = merge(&frames, &reference, Some(true));
        let (plain, _) = merge(&frames, &reference, None);
        let i = (30 * W + 50) * 3;
        for c in 0..3 {
            let want = darkest[i + c] / 0.25;
            assert!(
                (judged[i + c] - want).abs() < 1e-4 * want,
                "{} against {want}",
                judged[i + c]
            );
        }
        let away = (5 * W + 5) * 3;
        assert_eq!(judged[away..away + 3], plain[away..away + 3]);
        assert!(said[1].1.unwrap() > 0.0);
        assert!(said[0].1.unwrap() < 1e-3);

        // A budget the darkest frame's first ten rows fill: the ledger
        // ends there for both frames, the mover past it merges as with
        // no deghosting, and nothing past the budget is held.
        let budget = 10 * W * HELD;
        let ((short, _), peak) = merge_by(
            &frames,
            &reference,
            Some(Ghosts::new(W, H, true).with_budget(budget)),
        );
        assert!(peak.0 <= budget && peak.0 > 0, "{peak:?}");
        assert_eq!(short[i..i + 3], plain[i..i + 3]);
        let ((full, _), peak) = merge_by(&frames, &reference, Some(Ghosts::new(W, H, true)));
        assert_eq!(full, judged);
        assert_eq!(peak.0, 2 * W * H * HELD);
    }

    /// A noise whose read term fitted to nothing: a black sample's
    /// weight is then the inverse of the read term's floor, thousands
    /// of times another frame's, so the other's is rounded in the
    /// sums.
    const SILENT: Noise = Noise {
        shot: 2e-5,
        read: f32::MIN_POSITIVE,
    };

    /// `frames` (each its exposure, samples, and noise) through a
    /// deghosting started idle and run once all are in, then the
    /// reference: the merge, and how many samples held to the
    /// darkest's clip.
    fn merge_late(frames: &[(f32, &[f32], Noise)], reference: &[f32]) -> (Vec<f32>, usize) {
        let ref_mask = clip_mask(reference, W, H);
        let judge = Judge {
            rgb: reference,
            mask: &ref_mask,
            noise: NOISE,
        };
        let mut acc = Accumulator::new(W, H);
        let mut g = Ghosts::new(W, H, false);
        for (k, (e, rgb, noise)) in frames.iter().enumerate() {
            let mask = clip_mask(rgb, W, H);
            let frame = Frame {
                rgb,
                mask: &mask,
                exposure: f64::from(*e),
                noise: *noise,
            };
            g.add(&mut acc, &frame, &judge, k);
        }
        g.run(&mut acc);
        acc.add(&Frame {
            rgb: reference,
            mask: &ref_mask,
            exposure: 1.0,
            noise: NOISE,
        });
        g.finish(&mut acc);
        let mut image = acc.finish();
        let e_min = frames.iter().map(|f| f.0).fold(1.0, f32::min);
        let held = crate::merge::hold_to(&mut image, 1.0 / e_min);
        (image.data, held)
    }

    /// Two frames moved over one square, the reference not, the
    /// deghosting run only once both are in: their pending taken back
    /// out at every pixel of the square leaves the reference's samples
    /// there, though one frame's black outweighs the other's
    /// thousands of times in the sums.
    #[test]
    fn a_square_taken_out_of_every_frame_is_the_reference() {
        let reference = frame(1.0, 1, scene);
        let middle = frame(
            0.5,
            2,
            |x, y, c| if mover(x, y) { 0.0 } else { scene(x, y, c) },
        );
        let dark = frame(
            0.25,
            3,
            |x, y, c| if mover(x, y) { 0.9 } else { scene(x, y, c) },
        );
        let (merged, held) =
            merge_late(&[(0.5, &middle, SILENT), (0.25, &dark, NOISE)], &reference);
        assert!(merged.iter().all(|v| v.is_finite() && *v <= 4.0));
        assert_eq!(held, 0);
        for y in 20..40 {
            for x in 40..60 {
                let i = (y * W + x) * 3;
                for c in 0..3 {
                    let (got, want) = (merged[i + c], reference[i + c]);
                    assert!((got - want).abs() < 1e-5, "{x},{y}: {got} against {want}");
                }
            }
        }
    }

    /// Taken out of a pixel two frames of weights far apart hold, by
    /// what each added, in either order, the weight is exactly zero
    /// and its sums with it: no frame holds the pixel.
    #[test]
    fn taking_every_frame_out_leaves_exactly_nothing() {
        let black = [0.0f32; 3];
        let grey = [0.3f32, 0.32, 0.29];
        let reject = [1.0f32];
        for order in [[0, 1], [1, 0]] {
            for (a, b) in [(black, grey), (grey, grey), ([0.11, 0.7, 0.05], grey)] {
                let mask = [false];
                let frames = [
                    Frame {
                        rgb: &a,
                        mask: &mask,
                        exposure: 0.5,
                        noise: SILENT,
                    },
                    Frame {
                        rgb: &b,
                        mask: &mask,
                        exposure: 0.25,
                        noise: NOISE,
                    },
                ];
                let mut acc = Accumulator::new(1, 1);
                frames.iter().for_each(|f| acc.add(f));
                let parts: Vec<(f32, [f32; 3])> = order
                    .iter()
                    .flat_map(|&k| pending(&frames[k], &reject, 1))
                    .map(|(_, wt, s)| (wt, s))
                    .collect();
                take_out(&mut acc, 0, &parts);
                assert_eq!(acc.weight[0].to_bits(), 0, "{a:?} {b:?} {order:?}");
                assert!(acc.sum.iter().all(|v| v.to_bits() == 0));
            }
        }

        // A dim frame a 4096th of a bright mover's weight, left when
        // the mover is taken out, is kept.
        let mask = [false];
        let (mover, dim) = ([0.2f32; 3], [0.4f32; 3]);
        let bright = Frame {
            rgb: &mover,
            mask: &mask,
            exposure: 64.0,
            noise: NOISE,
        };
        let faint = Frame {
            rgb: &dim,
            mask: &mask,
            exposure: 1.0,
            noise: NOISE,
        };
        let mut acc = Accumulator::new(1, 1);
        acc.add(&bright);
        acc.add(&faint);
        let kept = weight_of(&faint, 0).unwrap();
        assert!(kept < acc.weight[0] / 4000.0, "{kept} of {}", acc.weight[0]);
        let parts: Vec<_> = pending(&bright, &reject, 1)
            .into_iter()
            .map(|(_, wt, s)| (wt, s))
            .collect();
        take_out(&mut acc, 0, &parts);
        let got = acc.sum[0] / acc.weight[0];
        assert!((got - 0.4).abs() < 1e-3, "{got}");
    }

    /// Where the reference clipped and a moved frame holds black at
    /// many times the darkest's weight, the darkest's sample stands:
    /// not the rounding of the black's weight taken back out.
    #[test]
    fn a_black_mover_where_the_reference_clipped_leaves_the_darkest() {
        let light = |x: usize, y: usize, c: usize| 2.0 * scene(x, y, c) + 1.0;
        let reference = frame(1.0, 1, light);
        let darkest = frame(0.25, 2, light);
        let middle = frame(
            0.5,
            3,
            |x, y, c| if mover(x, y) { 0.0 } else { light(x, y, c) },
        );
        let (merged, held) = merge_late(
            &[(0.5, &middle, SILENT), (0.25, &darkest, NOISE)],
            &reference,
        );
        assert!(merged.iter().all(|v| v.is_finite() && *v <= 4.0));
        assert_eq!(held, 0);
        for y in 20..40 {
            for x in 40..60 {
                let i = (y * W + x) * 3;
                for c in 0..3 {
                    let want = darkest[i + c] / 0.25;
                    assert!(
                        (merged[i + c] - want).abs() <= 1e-6 * want,
                        "{x},{y}: {} against {want}",
                        merged[i + c]
                    );
                }
            }
        }
    }

    /// A frame whose pending would not fit the budget is kept whole
    /// when a later frame's move starts the deghosting.
    #[test]
    fn a_pending_past_the_budget_keeps_its_frame_whole() {
        let reference = frame(1.0, 1, scene);
        let dark = frame(
            0.25,
            2,
            |x, y, c| {
                if mover(x, y) { 0.9 } else { scene(x, y, c) }
            },
        );
        let ref_mask = clip_mask(&reference, W, H);
        let mask = clip_mask(&dark, W, H);
        let judge = Judge {
            rgb: &reference,
            mask: &ref_mask,
            noise: NOISE,
        };
        let frame = Frame {
            rgb: &dark,
            mask: &mask,
            exposure: 0.25,
            noise: NOISE,
        };
        let mut plain = Accumulator::new(W, H);
        plain.add(&frame);
        let mut acc = Accumulator::new(W, H);
        let mut g = Ghosts::new(W, H, false).with_budget(PENDING);
        g.add(&mut acc, &frame, &judge, 0);
        assert_eq!(g.peak(), (0, 0));
        g.run(&mut acc);
        assert_eq!(g.finish(&mut acc), [(0, Some(0.0))]);
        assert_eq!(acc.finish().data, plain.finish().data);
    }
}
