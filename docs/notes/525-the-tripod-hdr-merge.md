# §525. The tripod HDR merge

*2026-11-01*

Phase 1 of the merge design: a tripod bracket merged into one linear
DNG beside its first frame, from the command line and from the frame
menu.

**The frames.** A seventh purpose of the stage list, `MergeInput`:
Prepare, CA, Demosaic (or the learned denoiser in its place, when
asked), a DNG's own warps, all on the CPU, ending at camera RGB. The
prepare runs with nothing clipped after the gains (`prepare_unclipped`
in core: the ceiling is the largest gain), so a red photosite at half
its range under a gain of two still reads as itself once the gains are
divided back out, and a saturated one reads 1 in its own channel. The
noise is measured on every frame. The reference walks at its own
as-shot white and every other frame takes the reference's gains. Only
the darkest frame is walked with `inpaint_opposed`; the rest are
walked with `Clip`, their clipped samples dropped.

**The merge,** core's `merge`, no UI and no edit schema. The exposure
is EXIF's (shutter times ISO over the f-number squared) relative to
the reference, then measured: the median ratio of camera luminance
over the pixels both frames hold between 1% and 70% of clip, every
seventh pixel. A pixel with any channel at or over 0.97, grown by two
pixels, is dropped from its frame. The weight is the inverse variance
on the reference's scale, e^2 over the noise model's shot and read
terms at the sample's own level, one weight a pixel for all three
channels. Where no frame held a pixel, the darkest frame's
reconstructed value stands, set whole. It streams: the reference is
walked first and held, each other frame is walked, checked, measured,
added to the sums, and let go, the darkest last so its fallback is
written once everything else is in.

**The noise, from the pairs.** The design took each frame's own
noise estimate. On the measurement set that cost the bright frame
most of its weight, and the reference's estimate overstated the
merge's variance 6x to 133x: core's estimator reads edges and texture
as noise. A bracket has one sensor, so the sensor's model is measured
once, from the frames against each other: a frame over its exposure
less the reference cancels the scene and leaves the two noises, whose
variance at level m is a·m(r/e+1) + b(r²/e²+1) for a frame at e
times the reference's exposure and r times its ISO. The green
channel's held pixels in half-stop bins, each bin's variance a median
of squares (robust to an edge the two frames see a hair apart), the
two terms fitted in relative terms. A frame's noise is that model at
its ISO, shot with the ISO ratio and read with its square, so a
bracket shot by ISO (one shutter, ISO 100, 400, 1600) is weighted as
right as one shot by shutter. A resampled frame's pair is left out:
the resampling smooths its noise. The reference's own estimate stands
only while no pair has spoken. On the measurement set the pairs read
1.3e-5 where the reference's estimate read 1.3e-4; the errors below
moved by less than the third digit.

**The movement check.** A translation fit (`register.rs`) of each frame
onto the reference, on planes both held to the band both frames hold
well (from 1% of the darker frame's clip to 70% of the brighter's).
Without that hold, the edge of a clipped area, which moves with the
exposure, read as a 27-pixel shift on the test bracket; with it, a
still frame reads 0.002 to 0.008 px and a frame knocked 2 px reads
1.8. A fit past a tenth of a pixel is believed only when the reference
has structure both ways and the move beats standing still: the
smaller eigenvalue of the structure tensor of the log plane (at an
eighth of the size) over the larger at least 0.05, and the RMS log
difference after the move under 0.9 of the one before. A flat horizon
over sky and sea, the classic bracket, has its structure one way and a
fit along it reads noise: a still two-patch scene read 2.6 and 1.4 px
of motion before this check and is left alone now (condition 1e-4). A
frame believed moved is resampled (bilinear, as `register.rs` has it),
re-masked and re-measured, and said. The fit's own residual is not
asked: a bracket's dark frame is noisy enough to sit over a stack's
`ALIGNED_RESIDUAL` (a soft 2D checker knocked 2 px read 2.002 px at
0.079), and the identity is the alternative that counts here.

**The file.** `write_merged_dng` in `dng.rs`: the reference's body,
`ColorMatrix` pair, forward matrices, `AsShotNeutral` and EXIF (the
lens kept: a bracket sits in the reference's geometry), the samples
divided by the brightest white-balanced sample with `BaselineExposure`
saying the stops, a `NoiseProfile` and our tag. The tag is
`DNGPrivateData`: "greycard", a NUL, then a line a fact: the kind, the
reference, whether the frames were denoised, the headroom's stops, and
each frame's name. Samples are half floats with the floating-point
predictor and Deflate (DNG 1.4), through the fork's float-deflate pin
that the rawler port brought in (`MERGED_SAMPLES`, one line): 173 MB
for three 45 MP frames against 537 MB as 32-bit floats uncompressed,
and the error bands below unchanged to the printed digit. A half keeps
11 bits down to 2^-14 of the stored white and fades below it, finer
than the merge's noise anywhere. A merged file reads back through our
decoder exact to the half (the test rounds each sample once and
compares), and `greycard export` opens the 45 MP merge in 4.7 s. The
name is `<first>-HDR.dng`, numbered as `-edit` is when taken.

**The noise profile.** One `(S, O)` line, as DNG has it, for a curve
that is not a line: the merge's variance steps up wherever a frame
drops out at its clip, fourfold where the frame two stops brighter
goes (sixfold measured, after the demosaic and the grown clip mask)
and fourfold again over the reference's clip, where the darkest frame
stands alone. No line is within 2x of all of it. A least-squares line
sat on the many levels under the first step and said 3.5 to 6 times
too little over it. The line now keeps its largest ratio to the curve
smallest, either way, over the band the reference holds (2^-14 of the
stored white up to the reference's clip, a quarter stop apart, a
search on a log grid): on the synthetic bracket it says 1.6x and 1.7x
the measured variance at -9 and -4 stops and 3.4x too little a stop
over the bright frame's clip (6.3x before). Over the reference's clip
it still says 12x too little (22x before); the file's readers (the
grain's estimate, the fill's cap, and the sharpen's floor) meet that
range only in highlights pulled down. A profile that follows the
steps wants a noise model with more than a line in it, in core, for
every reader; not here.

**Opening it.** The develop reads the new tags: `NoiseProfile` as a
linear file's noise model when our tag is there, and only then (a
phone's ProRAW and a denoised DNG carry one about their own pipeline),
and our tag onto `DngTags::merged`. The profile written is the pairs'
model carried through the weights (the inverse of the summed inverse
variances), fitted as above. One departure
from the design: the design took "the merged file opens at the
reference's exposure" as already true, and it was not. Our develop
does not apply a DNG's `BaselineExposure` (§474: it waits on the
neutral), so a merge stored under its headroom opens that many stops
dark; the prototype's check that the export opened did not look at
the brightness. A merge of ours
says its headroom in its own tag, needing no neutral
(`baseline::headroom`), and the finish's `Source` carries it beside the
body's baseline: the exposure's zero is the baseline plus the
headroom, so the merge opens at the reference's brightness, while the
display white follows the body's baseline alone (`White::at(body)`),
so the merge renders as its reference did and keeps the range over it
for bringing down. With the white on the whole sum, held highlights
rendered 2.4% darker at display 200 to 220 and 3.9% at 230 to 250
than the reference's. A file without our tag has no headroom, and
renders exactly as before. Once §474 honors `BaselineExposure`, a
merge's (the reference's, carried) must not be added on top of the
headroom. The learned denoiser stands down on any linear file
("needs a mosaic"), and the profiled denoise on every merge of ours:
its noise is a weighted mean of several frames', not one frame's.

The test the design promised develops the merged bracket and its
reference frame side by side through the viewport's walk, each
brightened by its baseline: the midtones and the highlights the
reference held agree within 2%, and two to four stops over the
reference's clip the merge is more than three times as bright where
the reference is flat. Through the finish as an export makes it, at
the reference's exposure, the midtones, the highlights the reference
held, and their brightest (display 200 to 250) agree within 2% (0.4%,
0.3%, and 0.3%). Two stops under, past the reference's clip, the
reference's export is flat and the merge's climbs. The Highlights
slider alone at -100 barely reaches that range on the test's ramp
(its span hangs from the frame's top; a rise of 0.0005 against
0.0003); the exposure does.

**The command line.** `greycard merge hdr <frames> [--reference N]
[--out PATH] [--on-exists WHAT]`, exits 0 merged, 2 merged with
something said (a frame resampled, an exposure only EXIF's), 1 nothing
written. An `--out` that is one of the frames, by canonical path, is
refused under any `--on-exists`. The file is written under a hidden
`.<name>.part` beside it, synced, and renamed at the end: a failure,
a quit, or a kill leaves no half file under the name and never costs
a file that was there under `--on-exists overwrite`. The window's quit
does not wait for the merge thread; the rename is what makes that
safe. A part file a killed run left (up to the file's size) is removed
when the next merge to that name starts, from the window or the
command line.

**The action.** Merge to HDR in the frame menu (no dialog follows, so
no ellipsis) after Stack bursts,
offered with more than one frame chosen. The frames chosen, each file
once, in the list's order; a thread of its own; each frame said in the
status line; a second ask queued behind the first, the status line
saying so. The reference's edit is the window's for a frame whose
sidecar the window has read, and its sidecar's otherwise, read on the
merge's thread. In a session that writes no sidecars the merged
file's edit is held in memory, as an edit made there is, and given to
its sidecar once the file is listed and read. The file is given to the indexer, its
sidecar written with the reference's edit less the exposure and less
the learned denoiser, and it goes on top of a closed stack with its
sources when they are of one folder and the list is a folder's.

## Measured

The prototype's set: one 45 MP CR3 made into x1, x3.8 and x15.5
frames (EXIF x1, x4, x16) with shot and read noise added, the middle
the reference; release build, 4 threads. Relative RMS error against
the source before the noise, by band in stops under the reference's
clip:

| band | prototype: reference / merge | here, bilinear, CA off: reference / merge | here, bilinear, CA on: reference / merge |
|---|---|---|---|
| -14 to -10 | 0.396 / 0.115 | 0.396 / 0.115 | 0.339 / 0.179 |
| -10 to -6 | 0.052 / 0.020 | 0.052 / 0.020 | 0.049 / 0.023 |
| -6 to -2 | 0.016 / 0.0071 | 0.016 / 0.0071 | 0.015 / 0.0090 |
| -2 to 0 | 0.0058 / 0.0052 | 0.0058 / 0.0052 | 0.0057 / 0.0052 |
| 0 to +1 | merge 0.0069 | merge 0.0069 | merge 0.0081 |
| +1 to +4 | merge 0.0066 | merge 0.0052 | merge 0.0073 |

With the prototype's demosaic (bilinear) and its CA (none), the merge
matches the prototype's table to the printed digit in every band up
to the reference's clip; above it the darkest frame's reconstruction
beats the prototype's neutral fallback (0.0052 against 0.0066). As
built the CA runs on each frame, as the design has it, and costs the
merge a quarter in the middle bands (0.0071 to 0.0090): each frame
fits its own correction, from its own noise and exposure, and the
fits disagree. A CA fit made once on the reference and applied to
every frame is the fix; core's `correct_ca` does not expose its fit,
so it is left for a follow-up. The weights on the frames' measured
noise against the noise the frames were made with differ by nothing
past the third digit. With the editor's default demosaic (AMaZE and
VNG4) the merge reads 0.026 / 0.011 / 0.0057 in the three middle
bands, against 0.052 / 0.018 / 0.0061 for the reference alone, on the
pairs' noise model.

Exposures: EXIF 0.25 and 4, measured 0.26316 and 4.0787 (truth
0.26316 and 4.0789). Five frames (x0.26 to x60): 0.06926 and 15.782
(truth 0.06925 and 15.789).

Time, three 45 MP frames: 16.0 s with the default demosaic (3.5 s a
frame walked, 0.55 s a fit, 0.2 s a merge with its pairs' noise, 3.8 s
the Deflate write, 0.4 s uncompressed). Bilinear and five frames were
measured before the pairs and the Deflate: 6.4 s and 21.4 s.

Memory: peak resident 3.2 GB for three frames and 3.16 GB for five,
measured in the test process (which holds the source raw and makes
each frame from it). Flat with the count, as the design wanted, but a
gigabyte over its 2.2 GB: the design counted the reference, one frame
and the sums, and not the walk's own working set while a frame is
demosaiced (its mosaic, the prepared samples, the demosaic's buffers).

The file: 173 MB for 45 MP as half floats with Deflate (537 MB as
32-bit floats uncompressed). The user guide says about 170 MB.

## Left

- The CA fitted once on the reference and applied to every frame.
- A noise model with the steps in it, for a merge's readers.
- The Deflate write on more threads: 3.8 s of the 16.
- The learned denoiser per frame: wired in the purpose and the merge
  (`Options::denoiser`), not offered in the action or the command line.
- The camera-match sampler refusing a merged file as a pair.
- A real tripod bracket: the measurement is synthetic.
