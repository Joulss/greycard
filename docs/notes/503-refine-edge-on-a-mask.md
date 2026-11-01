# §503. Refine edge on a mask

*2026-11-01*

Asked on discussion 59: a brush's or an AI mask's edge that follows
the subject's, so two masks meet without a seam. Lightroom and
Photoshop call it Refine edge.

**A guided filter on the composed mask.** The mask is composed on the
masks' 2048-wide grid the way the finish composes it, feather,
invert, and range shapes included, against the picture averaged into
that grid, and fitted to the picture's edges with a guided filter
(He, Sun, and Tang, the color form, the picture's Oklab as the
guide; the fast form of He and Sun, fitted on blocks and read back
bilinearly, held to the exact one within 0.08 worst and 0.01 mean).
The result is one u8 raster read in place of the shapes in the CPU
finish, the viewport shader (packed as one raster shape, the shader
unchanged), and every export, so the window and an export read the
same mask by construction. The slider runs a radius of 2 to 32
texels; eps is fixed in Oklab and scaled by the stops of baseline
plus exposure, so a dark raw is not smoothed four times harder. At
0, the default, nothing is refined, no picture is made, and the
sidecar is unchanged; the develop key hashes the slider.

**The picture, made only when asked for.** The guide is the picture
before the sharpen and after Local Contrast and Dehaze, the one the
finish's range shapes read, so Refine edge at 1 % selects the pixels
0 % does. It costs about 35 ms and 17 MB per base at 45 MP, so it is
made only for an edit that refines a mask: by the develop up front
(the GPU reads its kept pre-sharpen texture back in 256-row bands,
34 MB of staging rather than 720), by a walk's tap after the
pre-sharpen keep, by the save's keep when nothing is kept, or for
the window on asking the worker when its develop came without one;
the worker answers only for its last delivered develop's generation.
It is kept on the base by the patched picture and the Detail
settings, counted in the shelf's budget, and dropped when the edit
stops refining. A develop with no refined mask pays nothing. The
window remakes the raster only when the mask, the learned rasters,
the picture, the aspect, or the exposure changes; brush rasters are
not in the key, since the strokes and the aspect decide them, and a
raster held there would be copied on every tick.

**Known, and next.** The edge is found on the 2048-wide grid, about
4 px at 45 MP, and a Sky mask's wide raster is read at that grid;
the right shape is to send the fitted coefficients to the finish and
the shader and apply them per pixel, on the roadmap. The filter runs
on the window's thread, 20 to 35 ms per change on a refined mask. A
develop export reuses the kept picture, which may come from the
GPU's Detail, where a set's walk makes its own on the CPU; for a
refining edit with Detail on the two can differ by that tolerance.
Timings at 45 MP: a develop with refine off 77 to 82 ms, on 107 to
126 ms of which about 30 ms is the picture when it is not kept.
