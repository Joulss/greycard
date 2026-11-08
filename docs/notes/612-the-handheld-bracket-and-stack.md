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
stays, each frame is compared and added and let go, so the memory
does not change.

**The focus stack keeps its similarity** and takes the kernel. Its
leftovers on the roadmap (frames through the walk, progress, a noise
profile, the memory bound) stay where they are.

## Verified before it lands

Synthetic: a tripod bracket's frames (the merge tests' mosaics)
warped by a known similarity (2 px, half a degree, a part in five
hundred of scale) and merged come back to the reference within a
tenth of a pixel; a mover painted into one frame is rejected and the
rest of the frame is not; a still bracket runs no deghost and merges
to the bit as before. Real: a handheld bracket of three frames at 2 EV
and a focus stack stepped by hand, shot for this (none exists yet,
`docs/test-frames.md`), merged and looked at, numbers written here.

## Not changed

A homography for the HDR merge, a near scene's parallax, and the
panorama's own geometry. The learned denoiser per frame stays off for
a bracket until measured (§523).
