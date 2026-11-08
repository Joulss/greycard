# §612. The handheld bracket and stack

*2026-11-08*

§523's phase 3, pulled into 0.11.0: a bracket shot by hand merges as
a tripod's does, and a focus stack stepped by hand keeps its detail.
What the merges have, and what each still wants:

- The HDR merge (§525) fits each frame onto the reference as a
  translation, believes the fit only when the reference has structure
  both ways and the move beats standing still, and resamples a moved
  frame bilinearly. A hand turns and tilts the camera as well as
  shifting it, so a translation leaves the corners a pixel or two off
  on a 45 MP frame, and the average then doubles every edge there.
  Nothing rejects a thing that moved in the scene.
- The focus stack (§125, §573) already fits a similarity per frame
  (`register::Model::Similarity`: shift, turn, and the magnification
  focus breathing changes), which is what a hand adds. It resamples
  bilinearly too, and bilinear softens more than the merge's
  weighting gains back (§523 measured it).
- `register.rs` (§119) fits a translation, a similarity, an affine, or
  a homography (`fit_homography_from`) by inverse-compositional
  Lucas-Kanade on a log-luminance pyramid, blind to exposure, and
  warps bilinearly.

## The decision

**One kernel for every warp.** `register.rs`'s warps take a kernel:
bilinear as today, or Lanczos-3 (a windowed sinc over six taps a
side), with the edge handling they have. The HDR merge, the focus
stack, and the panorama's refinement pass resample with Lanczos-3; the
fit itself keeps bilinear inside its iterations, where speed counts
and the fraction of a pixel it settles does not depend on the kernel.
A Lanczos warp rings at a hard clipped edge; the merges clamp its
output to the frame's range, and the measurement below checks the
ring against the noise. Measured on a synthetic plane with a known
half-pixel shift and on a real frame warped and warped back: the
sharpness kept (the MTF at a quarter and a half of Nyquist) against
bilinear, and the ring at a step edge.

**The HDR merge fits a similarity.** `check_moved` fits
`Model::Similarity` instead of a translation, on the same held planes,
believed under the same two tests (structure both ways, the move beats
the identity), and a frame believed moved is resampled with the
kernel. A hand's tilt that a similarity cannot take out (a keystone)
is left: on a distant scene it is under a pixel, and a near scene's
parallax no plane fit removes. The recipe keeps the transform as it
does (`Registration::shift` is a `Transform`, which holds a
similarity), so a merge made again places the frames to the bit.

**Deghosting, as §523 wrote it, on when a frame moved.** The
reference is the truth. Once every frame is on the reference's scale,
a sample that differs from the reference's by more than four times
the noise model's deviation of the difference is dropped from the
sums; the rejection map is closed and blurred over a few pixels so it
does not speckle. Where the reference is clipped, the darkest frame
that disagrees with no other stands in; where none agrees, the
darkest frame alone. It runs when any frame's fit was believed moved,
since a tripod's bracket has no ghosts a 4σ test should be allowed to
find in its noise, and the command line's `greycard merge hdr` takes
`--deghost on|off|auto` (auto is this rule); the window's action has
no switch, as decided in §523. The frame report says how much of the
frame was rejected. The sums are streamed as today: the reference
stays, each frame is compared and added and let go. What the ledger
for the reference's clipped pixels and the samples held aside before
a move is known add on top is bounded, and said below.

**The focus stack keeps its similarity** and takes the kernel. Its
leftovers on the roadmap (frames through the walk, progress, a noise
profile, the memory bound) stay where they are.

## Not changed

A homography for the HDR merge, a near scene's parallax, and the
panorama's own geometry. The learned denoiser per frame stays off for
a bracket until measured (§523).

## What was built

**One kernel.** `register::Kernel` (bilinear, Catmull-Rom, Lanczos-3)
is the panorama's kernel moved into `register.rs`; `merge::pano`
re-exports it, so the pano tag's `kernel=` and its canvas warp are
unchanged. Every warp (`warp_plane`, `warp_samples`, `warp_image`,
`warp_camera_image`) takes one. Bilinear keeps its old code, to the
bit; the wider kernels sample directly in 2-D with the separable
taps normalized per pixel, the taps past the edge held to it, and
`Edge` as before (a point outside under `Fill` is the fill). We kept
the direct 2-D path for every transform: a translation's taps are the
same at every pixel, but at 1 s for 45 MP the separable special case
did not earn its code. The fit's own iterations sample bilinearly as
they did. `register::sample_range` and `clamp_samples` are what a
merge holds a Lanczos warp to: the frame's own darkest and brightest
finite sample (so no negative sample where the frame has none, nothing
over its brightest), a not-a-number fill left as it is. The clamp is
at the call sites, not in the kernel.

**A resampled frame's mask.** `clip_mask` grows a clipped sample by
2 px, the demosaic's reach; Lanczos-3 reaches 3, so its last lobe
landed 1 px past the grown mask at about +2.75% of a clip-to-dark
step, and with deghosting off (or a re-merge of an older tag) it was
averaged in. After a wide-kernel warp the mask is grown again by the
kernel's taps past bilinear's (`reach() - 1`, 2 for Lanczos-3), so 4
px in all; a bilinear warp's mask is as it was.

**Who takes it.** The HDR merge's `resample` (a frame believed moved,
or the recipe's move on a re-merge) and the focus stack's warp of the
picture (`stack::merge`, which both `stack_camera` and the engine's
focus stack go through) use Lanczos-3, clamped. Bilinear stays where
it is not resampling a picture we keep: the stack's coverage mask (a
ringing mask would read over one and under zero), the HDR movement
check's residual warp, and the panorama's reduced canvas for its
exposure and seams. The panorama's full canvas already warped with
Lanczos-3 by default (§523's kernel measurement); its refinement pass
is a homography fit, which warps nothing, so the panorama has no
change past the kernel's new home.

**The HDR merge fits a similarity.** `check_moved` fits
`Model::Similarity` on the same held planes (`merge::fit_planes`,
which only builds the planes and has no model in it); the two belief
tests are untouched. `Registration::shift` already held a
`Transform`, and the tag writes all six numbers in the fewest digits
that read back to the same `f64`, so a similarity round trips to the
bit (the tag test's transform is now a similarity's).

**Deghosting** (`merge::ghost`). For each frame against the reference
(both on the reference's scale): a channel more than 4 sigma off,
sigma the root of the frame's variance over `e^2` plus the
reference's, each from `Noise::variance` at the channel's own level,
is marked. The marks are closed by 2 px (grown, then the inverse
grown, with the merge's own `grow`), blurred by a 5-wide box twice,
doubled, held to 1, and never under the closed mark; the frame is
added with its weight times one less that. A 20 px mover is taken out
whole and the weight fades over about 4 px past it; a lone noise mark
takes out its own sample and under a tenth of its neighbors'.

Where the reference's clip mask is set, the reference cannot judge.
We chose a ledger over darkest-first order: each frame's sample
(its pixel and its value on the reference's scale, 16 bytes; its
weight in the sums is worked out again from the value)
is kept at the reference's clipped pixels it holds, and once every
frame is in, a pixel held by two or more is judged: if any frame
agrees with every other (the same 4-sigma test, a sample's variance
the inverse of its weight), the darkest that does judges and, since it
agrees with all, all stay; if none does, all but the darkest are
taken back out of the sums. Darkest-first order would have changed
the order of the sums (and so a still bracket's bits) and held the
darkest frame whole through the walk. The ledger's rejection is hard,
not blurred.

**What it costs, and the bound.** The merge's memory is no longer
only the reference, its mask, one frame, and the sums. The ledger is
16 bytes for every frame's sample at the reference's clipped pixels
it holds: a 45 MP reference clipped over 40%, with two darker frames
holding it, is about 0.6 GB. It is built under auto whether or not a
frame ever moves, since a later move needs it. The pending lists are
20 bytes a pixel the rejection would take from a frame added before
the first move. Both are held to a budget of 512 MB together
(`merge::ghost::BUDGET`): past it, the ledger ends at the last row
that fits, for every frame (the reference's clipped pixels past its
end merge as without deghosting, the darkest-frame rule skipped
there), and a frame whose pending would not fit is kept whole should
a later frame move. Counted before anything is kept, so neither
exceeds the budget even for a moment. Each is logged once when the
budget is reached, and the merge logs the most each held. The
rejection map's own planes (about five full-size, a frame at a time)
are on top of that, transient.

**When it runs.** `merge::Options::deghost: Deghost { On, Off, Auto }`,
`Auto` the default and what the window passes (`Options::default()`,
no UI change): on once any frame's fit is believed moved, or a recipe
gives a frame a move. Frames go through the sums in the walk's order,
so under `Auto` a frame added before the move is known goes in whole,
exactly as `Accumulator::add` adds it, and what the rejection would
take is kept aside, sparse; at the first move it is taken out of the
sums. A bracket where nothing moved therefore merges to the bit as
with deghosting off (tested), at the cost of the rejection map being
worked out for every frame. `greycard merge hdr --deghost
on|off|auto`, default auto.

**Report and recipe.** `FrameReport::rejected` is the frame's share
of its pixels taken from the reference (the rejection's weight summed,
plus the ledger's pixels), none for the reference or when deghosting
did not run; the log line `merge: NAME: moved X px, resampled, Y%
deghosted` and the CLI's frame line carry it beside the move. The
recipe keeps `deghost=on|off|auto` (`Recipe::deghost`, none for a
panorama); a bracket's tag written before it reads none, which the
re-merge takes as `off`, what that merge ran. A bracket's recipe also
keeps `kernel=` (`Recipe::kernel`, as the panorama's `Laid` keeps its
own); one written before reads as bilinear, so a re-merge of an older
merge resamples its moved frames as it did. `merge::Options::kernel`
(none for Lanczos-3) carries it.

## Measured

- **Kernel, a grating half a pixel off each way and warped back**
  (128x32, the contrast kept against the truth): at a quarter of
  Nyquist (0.125 cycles a pixel) bilinear keeps 0.924 (its `cos(pi f)`),
  Lanczos-3 1.007; at half of Nyquist (0.25) bilinear 0.707,
  Lanczos-3 1.022. Lanczos-3's response rises a little over one below
  half of Nyquist.
- **Ring:** a hard one-pixel step (0.1 to 0.9) warped half a pixel
  rings 11.1% of the step under Lanczos-3; the same step drawn over
  about a pixel (a logistic of scale 0.5 px, as a lens draws one)
  puts nothing past the frame's range. The clamp holds the hard
  step's ring to the range, so what is left of it is inside the
  frame's levels; at a clipped edge the mask grown by the kernel's
  reach covers it (tested: a clipped step half a pixel off, the ring
  past the old mask now masked, nothing over a part in a thousand of
  the step past the new one).
- **The similarity fit:** a bracket frame turned half a degree,
  scaled 1.002, and shifted (2, -1) px is found to 0.005 px at the
  worst corner (256x160 synthetic mosaics, the merge's own path).
  The focus stack's fit of sharp frames turned 0.5 degrees and scaled
  by 0.2% each: 0.03 and 0.11 px at the corners; merged back to an
  RMS of 0.00018 of the scene.
- **45 MP warp** (8256x5504x3, a similarity, release, 4 rayon
  threads, other builds running on the machine): bilinear 0.13 s,
  Lanczos-3 1.0 s.
- **The focus stack's halo test** (`a_depth_edge_leaves_little_halo`):
  the fixture's frames carry no move, but the fit reads the near
  frame's growing discs as a 1.8% scale and warps it. Under Lanczos-3
  that warp no longer softens the no-pyramid baseline: its halo falls
  from 13.3% to 9.1% of the step; the pyramid's goes from 3.2% to
  4.3% (unwarped frames: 14.1% and 4.0%). The test's absolute bound
  (under 5%) holds; its ratio bound moved from 0.4 to 0.5 of the
  baseline. The pyramid's halo rose under the sharper kernel because
  the mis-fitted warp no longer blurs the near frame's halo away; it
  stays within the 5% bound.

## Verified

- `cargo test -p greycard-core -p greycard-engine -p greycard-cli`
  (see the report for the run), `cargo clippy --all-targets` with
  `-D warnings` on the three crates, `cargo fmt --all`.
- New tests: the ring past a resampled clipped edge masked; the
  ledger ended by the budget and a pending past it keeping its frame
  whole; the bracket's kernel round trip and an old tag reading
  bilinear; the kernel's MTF and ring; flat, grid-exact, and fill
  behavior of Lanczos-3; the range and clamp; a mover rejected and the
  rest of the frame as without deghosting; a still bracket under not
  running merged to the bit, and a later run taking out what earlier
  frames would have lost; the reference clipped under a mover taking
  the darkest frame; the tag's deghost round trip and an old tag
  reading none; the hand's similarity found to a tenth of a pixel at
  the corners with the still frame left alone; a still bracket under
  auto merging to the bit as off; a moved bracket under off rejecting
  nothing and under auto saying what it rejected; the re-merge still
  the same to the sample (now with a moved frame deghosted both
  times); a stack's known similarity recovered.

## Not verified

- No real handheld bracket and no hand-stepped focus stack exist yet
  (`docs/test-frames.md`); nothing here has been merged from real
  frames. The figures are synthetic.
- The rejection's time at 45 MP and the ledger's real size on a real
  bracket with a large clipped sky were not measured; the 0.6 GB is
  arithmetic, and the budget's behavior is tested on small frames.
- Past the budget the merge falls back without deghosting at the
  reference's clipped pixels beyond the ledger's end: a row boundary
  where the rule stops may show on a mover crossing it.
- The ledger judges per pixel and hard: a mover crossing the
  reference's clip edge may show a seam where the soft rejection
  meets the hard one.
- Deghosting runs on luminance-level noise per channel (`Noise` is
  the channels' average); a demosaic's correlated red and blue noise
  may mark more or less than 4 sigma predicts.
