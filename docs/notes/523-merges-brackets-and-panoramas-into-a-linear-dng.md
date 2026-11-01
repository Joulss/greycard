# §523. Merges: brackets and panoramas into a linear DNG

*2026-11-01*

The Merge track's design, drafted 2026-10-31 beside a prototype and
revised after review. A tripod bracket, a handheld bracket, and a
panorama each become a linear DNG beside their sources through one
pipeline; the focus stack of §125 takes the same front door once the
first phase has built it. The prototype (its own small workspace
against the core, not in the tree) made its bracket and its panorama
from single raws, since the real series in docs/test-frames.md were
not yet shot; its findings and timings are at the end.

**One pipeline.** Both merges are the same five steps: each frame
developed to camera RGB, registered onto a reference, put on one
exposure scale, merged by weights, and written as a linear DNG beside
the sources. They differ in the transform (a bracket's is near the
identity, a panorama's is not) and the weights (a bracket's by
signal-to-noise, a panorama's by distance to each frame's edge, per
band). So one module with two front ends, not two features.

**The frames.** Each frame is walked to camera RGB as `greycard stack`
does today: Prepare, Ca, Demosaic, no gains baked in, nothing clipped
after them, so a saturated photosite reads 1 in its own channel. The
merge works there because the DNG stores it and its `ColorMatrix`
and `AsShotNeutral` describe it; a merge in the working space could
not be written as a raw. Every frame takes the reference's white
balance.

**Registration.** On the demosaiced linear picture, as luminance. A
bracket uses the core's Lucas-Kanade fit (`register.rs`), which is
already blind to exposure: a tripod bracket fits a translation as a
check and is resampled only if it moved more than a tenth of a pixel;
a handheld one fits a similarity. A panorama's overlap is partial and
its transform large, so it starts from features: Harris corners on the
log luminance at a quarter scale, normalized patch descriptors (an
exposure step is an offset in the log, which the mean removes), mutual
matches under Lowe's ratio, and a RANSAC homography. Two frames' worth
of that is under 0.2 s. The refinement is the Lucas-Kanade fit on the
overlap, which wants a homography model added to `register.rs`: the
affine it has cannot take out a keystone.

**Projection.** Planar, through the homography, up to a field of view
of about 90 degrees; past that a cylinder, the frames' homographies
converted to rotations about one center, which needs the focal length
in pixels from EXIF and the sensor size (the lens database's crop
factor; a phone or an adapted lens that gives no focal length stays
planar, and says so). A sphere is a later step and
not in this design. A homography is exact only for a camera turned
about its entrance pupil: a handheld panorama's near objects move
against the far ones (parallax), and no transform takes that out.
The seam finder below is the answer to parallax, not a polish, and
phase 2 without it is for a tripod on the nodal point. Frames are chained to their neighbors as
`stack.rs` chains them, and a closing pass spreads the drift.

**Lens first.** A panorama's frames get the lens correction (distortion
and vignette, §165's profile) before matching, in camera space: a
vignette biases the overlap's gain and pulls the refinement off (both
measured, see results). The vignette is applied to the samples; the
distortion is not a warp of its own but a term composed into the
frame's one warp (distortion, then homography, then projection, one
inverse map sampled once), as §513 composes a DNG's own warps, so the
"resampled once" below holds for a panorama too. The written panorama
then carries no lens identity, and no focal length or aperture either:
§453's fallback picks a profile from the body and the focal length
when a file names no lens, so stripping the lens name alone would
have the develop correct a second time. A tag of our own on the DNG
(a merged file, with what was corrected) is what the lens stage reads
to stand down; the stage list's purpose for a merged input checks it.
A bracket keeps its lens identity and is corrected once, at develop,
since it sits in the reference's geometry.

**Exposure.** From EXIF first: shutter, aperture, and ISO relative to
the reference (the middle exposure). Then refined from the frames
themselves: the median ratio of luminances over pixels both frames
hold between 1% and 70% of clip. Shutters miss their marks by a few
percent and a merge on the marks shows it as banding where the
weights change hands; the refinement recovers the true ratio to
0.01% in the prototype. A panorama's overlap gives the same ratio, per
channel, which also settles a white balance that drifted.

**The bracket's weights.** Each sample divided by its exposure lands on
the reference's scale; the weight is the inverse of its variance on
that scale, exposure squared over the noise model's shot and read
terms, taken from the core's noise estimate of each frame. A pixel
with any channel at or over 0.97 of clip, grown by the demosaic's
reach (two pixels), is dropped from that frame. Where every frame
clipped, the darkest frame's value is used after its own highlight
reconstruction (`inpaint_opposed` on its mosaic): the merge does the
reconstruction once, at the one place it is still needed.

**Headroom, and the develop.** The result is stored divided by its
brightest white-balanced sample and `BaselineExposure` says the stops,
as `dng.rs` already does. Stored that way, nothing in a linear DNG's
develop clips it: reconstruction runs on mosaics only, and the clip
after the gains sits at the file's top. So the roadmap's "develop
skips highlight reconstruction and the white-level clip" needs no
flag on the shot; it holds by construction, and a test pins it. The
merged file opens at the reference's exposure. Not clipping is the
easy half: the tone side decides where white is, and the adaptive
highlights and whites (§315), the scene curve, and a matched look all
read the frame's range. The prototype only checked that the export
opened. The test to write develops the merged bracket and the
reference frame side by side at the reference's exposure and pins
their midtones and unclipped highlights to each other, so a bracket
four stops over white comes out as the reference did with its
highlights kept, not as a flat frame pulled down to fit.

**The file's identity.** The DNG carries the body's make and model and
the reference's `ColorMatrix`, so the camera profile and a matched
look (§180) apply to it as they do to the reference, which is what we
want; the camera-match sampler never takes a merged file as a pair,
since its brightness is no camera's. It carries a `NoiseProfile`
(DNG 1.3): the merge knows each frame's shot and read terms and the
weights, so the result's noise on the reference's scale is a known
function of level, and the file says it. The denoisers still stand
down on a merged input (the profiled model is one frame's), but the
grain estimate, the fill's grain cap (§434, §447), and the sharpen's
noise floor read the profile and so behave as on a frame, instead of
reading a clean merge as a noiseless one.

**Samples.** Float. A 16-bit file's step is 2^-16 of the headroom, so
each stop of headroom costs a stop of shadow precision: at the
prototype's 2.5 stops the 16-bit file already adds a tenth to the
deepest band's noise, and a five-frame bracket at 2 EV has four
stops, where the step is 2^-12 of the reference's clip, coarser than
the deepest band's signal.
DNG 1.4's float samples go into `dng.rs` now, not when someone asks.
Uncompressed they are 540 MB at 45 MP, against 118 MB for 16-bit
lossless, so we write 16-bit float with the floating-point predictor
and Deflate, which rawler reads (its `decompressors::deflate`: half,
24-bit, and single floats, byte planes shuffled, predictor 3, zlib)
and does not yet write. Its `DngCompression` has `Uncompressed` and
`Lossless` only, and `Lossless` turns floats into integers. The change
is a `Deflate` arm of the writer: each strip's floats narrowed to
half, the bytes shuffled into planes and delta-predicted as the
reader undoes them, zlib-compressed with the libflate it already
depends on, and the IFD tagged Compression 8, Predictor 3 (the
floating-point one), SampleFormat 3, BitsPerSample 16. The mirror of
the reader, offered upstream as dnglab/dnglab#896 (2026-11-01, with
a RowsPerStrip fix to the uncompressed writer found on the way).
Phase 1 does not wait on its acceptance: greycard pins rawler to a
commit on our fork already, so the two commits go onto the pinned
branch and the pin moves, and the merge writes half-float Deflate
from the first release. No choice of samples is shown; a tester
never needs one.

**Deghosting.** Handheld only, phase 3. The reference frame is the
truth: a sample that differs from the reference's (both on one scale)
by more than four times the noise model's deviation of the difference
is dropped, the rejection map closed and blurred over a few pixels so
it does not speckle; where the reference is clipped, the darkest
frame that disagrees with no one stands in, and where none agrees
(the reference clipped and the scene moved), the darkest frame alone,
since one frame is a picture and an average of movers is not. Moving
water and foliage then come from one frame, which is the honest
answer.

**Resampling.** A warped frame is resampled once, onto the reference,
with a better kernel than the bilinear `register.rs` uses: bilinear
softens detail more than the merge's averaging gains back (measured).
That is the Engine pool's `stack.rs` and `register.rs` warps item,
which this design pulls forward.

**Seams.** Multi-band (Burt and Adelson), sharing `stack.rs`'s pyramid
code, with the masks from distance to each frame's edge. Accumulated
frame by frame into one band pyramid, so memory does not grow with the
count. Gradient-domain blending is better at moving objects across a
seam and costs a Poisson solve; we start with multi-band and put a
seam-finding pass (a graph cut over the overlap's difference, the
seam led around what disagrees) in phase 2, since it is what makes a
handheld panorama's parallax and its movers come out whole from one
frame rather than doubled across the blend. The result is cropped to the
largest covered rectangle; the uncropped canvas is a later option.

**Where it lives.** `greycard-core`: a `merge` module (exposure,
weights, deghost, features, homography, projection, blend), the float
writer in `dng.rs`, the homography model in `register.rs`; no UI, no
edit schema. `greycard-engine`: the frames' walk, as a purpose of the
stage list. `greycard-cli`: `greycard merge hdr|pano`, the reference
the action must match, as `greycard stack` is for focus. `greycard-ui`:
the action. `greycard-library`: nothing new to merge; a bracket finder
is a question below.

**The stage list.** A sixth purpose, `MergeInput`: the list cut after
the demosaic, all CPU, the highlights clipped not reconstructed (except
the darkest frame of a bracket), CA corrected, and for a panorama the
lens stage moved before `IntoWorking`, which wants the lens stage to
run in camera space as well as in the working space. On the merged
file the list needs one fact from the decode: a linear, merged input
has no mosaic for the learned denoiser, and its noise is no longer one
frame's, so the working-space denoisers read the file's `NoiseProfile`
(below) rather than a camera's model.

**Denoising, per frame.** The learned denoiser takes a packed mosaic
and gives RGB, so it is a demosaic in the list's terms, and
`MergeInput` runs it in the demosaic's place when the action asks:
Prepare, CA, the denoiser. What the merge reads from the mosaic (the
clip masks, the darkest frame's reconstruction) it reads before the
denoiser runs, and the bracket's weights and the deghost threshold
keep the frames' raw noise model, which is conservative on denoised
data. A panorama gains most: nothing averages outside its overlaps,
so its noise is one frame's, and features match better on a clean
frame; on by default for a panorama, at one model run a frame (about
3 s for 45 MP on a current discrete card). A bracket gains less, since
the merge already averages the bright frames into the shadows, and
the denoiser may invent slightly different texture in each exposure
that the average then softens; off for a bracket until measured on a
real one. The choice follows the reference edit's Noise setting and
is shown in the action. A file merged from denoised frames is marked
so the develop's denoisers stand down, and its `NoiseProfile` holds
the residual, measured on the frames' flat regions. The other
denoisers run in the working space, after `IntoWorking`, and have no
place before a merge; at develop the merged file gets them like any
frame, read against its `NoiseProfile`, and never the learned one.

**The action.** Over the selection (§156), in the frame menu beside
Stack bursts, on a thread of its own as Stack bursts runs, with the
progress words through the engine's progress trait and a status on
landing; a second ask waits on the first. The new file lands beside
the first source, named after it with a suffix and no space
(`IMG_0001-HDR.dng`, `IMG_0001-pano.dng`, a number appended when the
name is taken as `-edit` does), and goes into a stack with its
sources, itself on top, through the stacks' store. Its edit starts
as the reference frame's less the exposure, so a look and a crop
carry over and the headroom is not pulled down. The menu item reads
"Merge to HDR..."; the panorama's, when it comes, "Merge to
Panorama...". The frames are the selection; finding brackets by their
exposure steps is a later item.

**Memory.** A 45 MP frame in camera RGB is 540 MB. Holding five frames,
as the prototype and `greycard stack` do, peaks near 5 GB. The merge is
a per-pixel sum, so we stream: the reference and one frame resident,
plus the accumulators (sums and weights, 720 MB) and two luminance
planes; about 2.2 GB at any count. A panorama accumulates its band
pyramid the same way: a third over its canvas in RGB and weight plus
one frame's pyramid, about 1.5 GB for a 36 MP canvas and 7 GB for a
300 MP one, which wants a cap or strips; see the questions. The
prototype, holding every layer whole, peaked at 6.5 GB for 36 MP.

**Phases.** Phase 1, the tripod bracket: `MergeInput`, exposure and
refinement, the weights, reconstruction of what every frame clipped,
the `NoiseProfile`, the float writer (its Deflate arm upstream, the
16-bit cap meanwhile), the develop test against the reference frame,
`greycard merge hdr`, and the action. Phase 2, the
panorama: lens in camera space composed into the one warp, features
and RANSAC, the homography fit, the cylinder, the seam finder, the
blend accumulated, the crop, the merged-file tag the lens stage reads. Phase 3, the
handheld bracket: the similarity fit (there now), the better
resampling kernel, and deghosting.

**Decisions taken 2026-11-01.** The action reads "Merge to HDR..."
and, when it comes, "Merge to Panorama..."; the file is named after
its first source with a suffix and no space; it goes on top of a stack
with its sources; its edit starts as the reference frame's less the
exposure; the samples are half-float Deflate from the first release,
through the fork's rawler pin, and no choice is shown; brackets are
the selection, a finder by exposure steps is a later item; deghosting
is on for a handheld bracket; a panorama is planar or cylindrical, the
sphere left out; the per-frame learned denoise is on for a panorama
and off for a bracket until measured; focus joins the Merge action
after phase 1. The panorama's canvas cap and the uncropped output stay
open for phase 2.

## The prototype's results

### What worked

- **Exposure refinement.** EXIF 0.25 and 4.0; measured 0.2632 and
  4.0785 against truths 0.2632 and 4.0789. On the EXIF numbers the
  merge is off 1 to 5% by band; refined, it is at the noise.
- **The weighted merge beats the reference frame in every band** on
  the pixels both have (relative RMS error against the source):

  | stops under the reference's clip | reference alone | merge |
  |---|---|---|
  | -14 to -10 | 0.396 | 0.115 |
  | -10 to -6 | 0.052 | 0.020 |
  | -6 to -2 | 0.016 | 0.0071 |
  | -2 to 0 | 0.0058 | 0.0052 |

  Above the reference's clip the merge holds 2.5 stops more at 0.7%
  error; 0.006% of pixels were clipped in every frame.
- **The editor opens the result.** The 16-bit and the float DNG both
  export headless (exit 0, 6.2 to 7.6 s a file) at the reference's
  exposure, and at `--exposure=-2` the sunlit water holds. The two
  renders differ by 0.0008 on average (JPEG units). The float file
  reads back exactly; the 16-bit file adds 5% relative error in the
  deepest band, 0.4% at -10 to -6.
- **No develop change was needed.** A linear DNG is never
  reconstructed, and with the headroom set to the brightest
  white-balanced sample, the clip after the gains sits at the file's
  top. The design drops the "merged" flag on that ground.
- **Handheld registration.** The core's similarity fit puts the shaken
  frames back within 0.011 and 0.031 px at the corners, 0.7 and 0.9 s a
  frame at 45 MP.
- **Panorama registration.** 1823 and 1899 corners, 461 matches, 439
  inliers; the homography lands within 0.43 px of the truth over the
  overlap. Without the vignette, the affine refinement takes it to 0.29
  px and the overlap's gain is exact (1.1767 against 1.1765).
- **Blend.** At 1:1 no seam shows in either the multi-band or the hard
  cut (the prototype's seam captures); the textured overlap does not test it
  hard, and the gain error below was in both.

### What did not

- **Vignetting spoils a panorama's overlap.** With the vignette on, the
  overlap's gain read 1.10 for a truth of 1.18 and the refinement went
  from 0.43 to 0.49 px. Hence lens correction before matching.
- **Bilinear resampling costs a handheld merge more than averaging
  gains.** With the frames shaken and fitted back, the merge's error at
  -6 to -2 stops rose from 0.0071 to 0.019, worse than the reference
  alone. Part of that is the test (the source's own noise is shared by
  every frame, so smoothing it reads as error), but the detail lost is
  real. Hence a better kernel before phase 3.
- **A clipped sun turns pink in a linear DNG.** The panorama's clipped
  sky came out pink in the editor's export: the core's demosaic leaves a
  saturated channel at 1, nothing reconstructs a linear file, and the
  gains do the rest. The bracket hid this only because the prototype
  sets its everywhere-clipped pixels neutral. Hence reconstruction
  inside the merge.
- **Float is big uncompressed:** 537 MB at 45 MP against 118 MB for
  16-bit lossless. Rawler writes float only uncompressed.
- **The affine refinement cannot take out a keystone:** 0.41 to 0.29
  px, not to zero. Hence a homography model in `register.rs`.
- **Memory:** 5.4 GB peak for the three-frame bracket (it holds every
  frame, the truth, and both read-backs) and 6.5 GB for the two-tile
  panorama (three-channel masks, every pyramid whole). The design
  streams both.

### Timings, seconds

| stage | tripod, 3 x 45 MP | handheld, 3 x 45 MP |
|---|---|---|
| decode (one file) | 0.29 | 0.26 |
| make the bracket (prototype only) | 1.75 | 1.62 |
| demosaic, bilinear, 3 frames | 1.69 | 1.74 |
| register 2 frames, similarity | - | 1.64 |
| clip masks | 0.17 | 0.20 |
| refine exposures | 0.05 | 0.06 |
| merge | 0.18 | 0.16 |
| write 16-bit lossless | 1.18 | 0.97 |
| write float, uncompressed | 0.43 | 0.51 |

| stage | panorama, 2 x 22 MP to 36 MP |
|---|---|
| decode and demosaic the source | 0.9 |
| corners, descriptors, matches, RANSAC | 0.11 to 0.18 |
| affine refinement on the overlap | 0.33 to 0.41 |
| gain from the overlap | 0.07 to 0.09 |
| canvas and 7-level multi-band blend | 1.5 to 2.1 |
| write 16-bit lossless | 1.2 to 1.7 |

A real tripod bracket of three 45 MP raws would take about 5 s with
the bilinear demosaic, before the editor's default demosaic and the
reconstruction of the darkest frame; the merge itself is under half a
second. The editor's headless export of the result: 6.2 to 7.6 s, run
with the release build of master from earlier today rather than a
fresh one in the worktree.
