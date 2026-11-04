# §560. The range masks read the picture from before Detail

*2026-11-05*

The pool line from §158: a Luminance or Color shape read the picture
after the Detail section, so a Dehaze or Clarity move shifted what its
window took in. Now, with that section doing something (Texture,
Clarity, Structure, or Dehaze off zero under its switch), a range
shape reads the picture the section ran on: the patched picture, the
one §287's planes are made of. Capture sharpen is after it, so it
leaves the sample too. With the section doing nothing, the sample is
the developed picture as before, capture sharpen in: the sharpen alone
moves 0.04 to 0.06% of a window's pixels by more than 0.05, too little
for a second texture.

**Carried beside the planes.** `DetailPlanes::sample`, an
`Arc<WorkingImage>` shared, not copied, and `BandImages::sample` with
it, so the geometry, the edited pictures' reduce and fit, and the
export's resize take it as they take the bands. Asked for as
`DetailWants::sample` (`Edit::samples_before_detail`: a range shape
in any mask, switched on or off), and handed on whenever it is asked
for: on the CPU it is the patched picture the base keeps anyway. The
tap's wants and the kept planes strip it: it is no band. The
viewport's develop and an edited picture made from it hand on the
patched picture the base keeps; an export and the reduced develop
take it at a new `Tap::Sample` after `Keep::Patched`, only for those
purposes, so the viewport's step list and its kept points are
unchanged. The export resamples it only when an enabled mask reads
it.

**Refined masks read it too, always.** A Refine edge's picture
(`MaskPicture`, the masks' grid) is tapped once, at `Keep::Patched`,
for every edit that refines, and the worker makes it of the patched
picture on every path; the GPU read-back of the picture before the
sharpen is gone, and so is its key on the Detail section. So a refined
window takes in what an unrefined one does, and no refined gradient or
Sky changes its edge when another mask gains or loses a range shape,
as it would if the picture followed `samples_before_detail`. The first
build left it after Detail: on the reviewer's edit (a crop to the top
half, a hard window 0.78 to 0.90, Dehaze 0.6, Clarity 0.8) the
unrefined mask covered 62.0% of the export and the refined one, at
Refine edge 0.05, 34.8%, near the old after-Detail 32.0%. Existing
refined gradients, brushes, and learned masks under Detail move once,
by design: their edges are now the picture's own, without the halos
Dehaze and Clarity draw.

**The GPU.** Binding 15, `range_src`, Rgba16Float at the source's size;
`Params::range_sample.x` says it is there. The shader reads it as it
reads the source (bilinear, or cubic when the plane resamples) and takes
the color window's 5x5 mean from it (`local_ab_of`); the mixer keeps its
own mean of the source. It goes up only while a drawn mask reads it, an
enabled adjustment with a live, unrefined range shape or the one whose
mask is shown (`Renderer::sync_sample`, at `render` and `analyze`), and
back to a texel when none does; known by its allocation, so a Detail
slider's develop does not send it again. The planes' bands are known by
their own allocations as well, so attaching the sample does not upload
them again. Cost: 8 bytes a pixel, 192 MB at 24 MP and 360 MB at 45 MP
while it is up, and a peak of 720 MB at 45 MP during the upload (the
half-float copy and wgpu's staging). The reviewer measured, at 44.8 MP
on the 5070 Ti, +3125 MiB for the frame with no shape and +3466 MiB with
an enabled Luminance shape (+342 MiB); a switched-off one held the
texture in the first build (+3676 MiB) and now holds none. Each
switch-on of an adjustment, or of Show mask, converts and uploads the
sample again (360 MB of half floats and wgpu's staging at 45 MP): a
brief hitch, acceptable for a toggle. And the renderer's `sample_held`
keeps the patched picture (540 MB of f32 at 45 MP) until the next
`set_detail`, shared with the engine's base while the frame is open but
outside the engine's memory accounting once the base goes.

`detail_asks` still asks for a develop when a range shape first goes
in under a Detail section that does something, since the develop is
what hands the picture on; until it lands the shader reads the
source, so the window can jump once. Switching an adjustment on or
off, or showing its mask, needs no develop: the texture follows the
draw.

**Checked.** In render,
`the_range_masks_read_the_picture_from_before_the_detail_section`: a
hazy ramp, Dehaze 0.6 moving more than 5% of it across a hard window's
edges, a -8 stop look under the window; the export's dark pixels are the
window read off the ramp before the dehaze, to the pixel, at the
picture's size and through a resize, and not the one after. Refined at
0.05 against the picture before the dehaze, it is 0.07% of pixels apart
from the unrefined one (coverage 47.53% against 47.46%); against the one
after, 36.3% apart.
`the_shaders_range_masks_read_the_picture_from_before_detail`
(viewport): the five §158 masks over the field dehazed, the field beside
it as the sample, against `Local::weight_sampled` on the field: max
0.0020 on lavapipe and 0.0022 on the card, mean 0.0003, the eight-bit
read back's half level, every mask covering part of the field.
`the_range_sample_goes_up_only_while_a_drawn_mask_reads_it` (viewport)
and `the_sample_is_tapped_for_a_finish_on_the_cpu` (stages, the refined
picture tapped once, before Detail, for a range shape and a gradient).
Three engine tests held the refined picture after Detail and now hold it
before: the range shapes' picture is the patched one; a Dehaze move
keeps it, for a range shape and a gradient alike; and with the GPU the
picture is the patched one's, the CPU develop's within the base's GPU
rounding, with nothing read back
(`with_the_gpu_the_refined_masks_picture_is_the_patched_ones`, which
replaces the read-back test). No UI test relied on the old sampling.

The reviewer's §18 run (release, weston headless GL, Dehaze 0.6 and
Clarity 0.8), the view against the full export: a Luminance window
0.70 to 1.00 0.075% RMSE, no mask pixel apart at 1:1 and 0.007% at
half size; a Color window at 250 degrees 0.067%, 0.005% of mask
pixels at 1:1 and 0.11% at half size; a hard window 0.78 to 0.90
0.41%, 0.018% of mask pixels, which is the hard edge's own (0.64% on
master with Detail off); no mask 0.066%. With Detail off the view, the
full export, and the half one are bit-identical to the base for all
three. A Dehaze drag costs the same with a shape as without (138.6 to
139.5 ms): no upload on a Detail move. The export at 44.8 MP to 2048
finishes in 0.81 s with an enabled range mask against 0.56 s with a
disabled one.

**What moves in an existing edit.** By design an edit with a range
mask and Detail now develops differently. Measured on two sample
raws, developed at the defaults on the CPU, §158's sky window (70 to
100, fading 6 below), the mask's weight read off the picture before
Detail against the one after it and the capture sharpen:

| frame | Detail | coverage now | before | moved > 0.05 |
|---|---|---|---|---|
| a 20 MP frame | Dehaze 0.6 | 35.27% | 28.66% | 9.86% |
| the same | Clarity 0.8 | 35.27% | 35.20% | 2.68% |
| a 44.8 MP frame | Dehaze 0.6 | 0.62% | 0.13% | 1.61% |
| the same | Clarity 0.8 | 0.62% | 2.01% | 3.16% |

**Left.**
- A smaller texture, if the 8 GB Mac needs it: Rg11b10Ufloat, 4 bytes
  a pixel, negatives clamped, the CPU quantizing the same; unmeasured
  there.
- The measurement tool (`samples.rs`'s direct `finish_with`) still
  samples the picture it is handed.
