# §643. The capture sharpen decides what is flat before Texture (2026-11-11)

**What was wrong.** The capture sharpen (§21) blends its deconvolution
in by local contrast, under a threshold set from the flattest patch of
the picture (the one-percent rule), and sharpens everything when no
patch is flat enough. It ran after the Detail section's local contrast
and read the picture it was given. Texture lifts the flattest patch's
grain with everything else, so on an ISO 3200 frame the threshold fell
from 0.39 to 0 at Texture +50: the whole picture sharpened, the sky's
grain textured and then deconvolved, 3.2 times its noise at Texture 0
where Texture alone makes 1.4. Clarity +100 did the same there.
Texture -100 moved it the other way (0.39 to 0.07), and the sharpen
then covered four times the area, working against the smoothing.
Structure moved it by a hundredth at most.

**What we decided.** What is flat is read off a guide: the picture
before the local contrast, dehazed with the picture by the picture's
own haze model. The automatic threshold and the blend mask both read
the guide's L*; the deconvolution, its luminance, and the clip mask
stay the picture's, where they were. A step of its own,
`Tap::SharpenGuide`, takes the guide just before the local contrast,
only where Texture, Clarity, or Structure runs and a sharpen follows
(the picture's or a mask's); it rides with the taps, so the point kept
before the sharpen holds it and a sharpen slider resumes from there
with the same guide. The dehaze moves the guide as it moves the
picture (`dehaze_also` on the CPU, the same model on the device), so
Dehaze keeps the say it had over what is flat, and Texture at +1 does
not jump from a dehazed decision to an undehazed one. On the GPU the
sharpen's planes pass reads the guide's L* over the picture's
(`guide_lstar`), and the threshold is remembered with the guide.
RawTherapee sharpens the demosaiced raw, before any local contrast:
reading the guide puts ours back where its decision was made.

**Why the blend too, not only the threshold.** We measured both, CPU
path, every frame, Texture 0, +50, +100, and -100. Share is the blend
mask's mean; noise is the flat patch's grey less a 2 px blur, std
×1000; fine is the std of the log luminance less a 2 px blur on a
detailed 600 px region.

| ISO 3200 frame | threshold | share | sky noise | cloud noise | conifers fine |
|---|---|---|---|---|---|
| Texture 0 (all three) | 0.39 | 9 % | 19.0 | 15.9 | 0.518 |
| +50 before | 0 | 100 % | 60.6 (3.2×) | 53.1 | 0.743 |
| +50 threshold only | 0.39 | 26 % | 33.2 (1.7×) | 27.7 | 0.650 |
| +50 threshold and blend | 0.39 | 9 % | 26.7 (1.4×) | 22.6 | 0.597 |
| +100 before | 0 | 100 % | 73.4 (3.9×) | 66.9 | 0.821 |
| +100 threshold only | 0.39 | 46 % | 53.1 (2.8×) | 45.2 | 0.763 |
| +100 threshold and blend | 0.39 | 9 % | 34.3 (1.8×) | 29.3 | 0.672 |
| -100 before | 0.07 | 38 % | 3.9 | 2.5 | 0.486 |
| -100 threshold only | 0.39 | 1 % | 3.6 | 2.4 | 0.263 |
| -100 threshold and blend | 0.39 | 9 % | 3.8 | 2.5 | 0.342 |
| Clarity +100 before | 0 | 100 % | 43.4 | 36.9 | 0.663 |
| Clarity +100 after | 0.39 | 9 % | 19.2 | 15.9 | 0.520 |

Texture alone (sharpen off) puts that sky at 25.2 (+50) and 32.5
(+100): with the blend read off the guide the sharpen adds to the
textured sky what it adds at Texture 0, and no more. With the
threshold alone, the textured grain still crosses the unmoved
threshold, and the share grows with Texture (26, 46 %) where it should
stay put; at -100 it shrinks to 1 % and the smoothed detail goes
unsharpened. On the conifers, dark and grainy, "fine" counts grain as
well as needles, so its fall is the grain's; the 1:1 crops show the
needles kept.

A clean low-ISO landscape, a portrait, and an architecture frame:

| | threshold | share | sky noise | fine (larch / brick / bag) |
|---|---|---|---|---|
| landscape T0 | 0.07 | 48 % | 2.05 | 0.758 |
| landscape +100 before / threshold / blend | 0.13 / 0.07 / 0.07 | 42 / 57 / 48 % | 3.97 / 5.17 / 3.95 | 0.829 / 0.830 / 0.830 |
| portrait T0 | 0.13 | 63 % | | 0.165 |
| portrait +100 before / threshold / blend | 0.23 / 0.13 / 0.13 | 58 / 83 / 63 % | | 0.218 / 0.227 / 0.220 |
| architecture T0 | 0.10 | 29 % | 2.25 | 0.407 |
| architecture +100 before / threshold / blend | 0.17 / 0.10 / 0.10 | 26 / 55 / 29 % | 4.51 / 5.64 / 4.46 | 0.505 / 0.520 / 0.509 |

On the clean frames the old fault ran the other way: Texture raised
the threshold, so the sharpen covered less. Reading the guide holds
the share where Texture 0 has it, the flat noise at or under before,
and real detail (larch, brick, the bag, the faces) sharpened as much
as before or a little more. Threshold-only sharpens more flat area
than either. So both.

**A manual threshold** keeps its meaning: the slider's number is the
threshold, and the blend reads the guide's contrast against it, as the
automatic one is read.

**Output sharpening (§43)** reads the resized picture with its own
threshold, and Texture moves it: on the ISO 3200 frame at 2048,
0.17 at Texture 0, 0.22 to 0.32 above it, 0.07 at -100. It never falls
to zero; Texture's grain, resized, raises its threshold and the share
falls (24 % to 15 to 20 %), so it keeps flat parts flatter, not
sharper. That is its rule working on the picture it is given, the
small one, which is what it is meant to read. Left as it is.

**What existing edits see.** An edit with Texture, Clarity, and
Structure at zero renders as before, to the bit: the step list has no
guide, and the sharpen reads its own input as it did. Checked on the
four frames at Detail zero and at Dehaze +50 alone, CPU exports
against the build before, max difference 0; on lavapipe, the GPU
export at Texture 0 against the build before, also 0. An edit with any
of the three on renders with less sharpened grain at high ISO, and on
clean frames with the share of Texture 0. Its develop key takes
`SHARPEN_GUIDE` ("sharpen-guide-1") only when its sharpen reads a
guide (`Edit::sharpen_reads_guide`), so its kept pictures are made
again and every other edit's keep their key; the string is in
`keys_made_under`, so the index makes its keys anew once and finds
the same keys for those. `DEVELOP_RECIPE` is not raised.

**GPU against CPU.** The device's guided sharpen is the reference's to
the sharpen's tolerance (1e-4 relative), thresholds equal, in the
viewport's form and the export's with the delta; the random sweep
draws a guide in two cases of five, seeds 1 to 10 at 60 cases on
lavapipe and 200 cases on an RTX 5070 Ti, none failed. A full export
of the ISO 3200 frame at Texture +50 on lavapipe is the CPU's to a
mean of 0.02 levels of 65535 (max 150; before, the two paths were 709
apart at most on the same frame).

**Costs.** On the CPU nothing more without Dehaze (the guide is the
patched picture already held); with Dehaze, a second apply of the haze
model and a second picture until the sharpen, which the point kept
before the sharpen holds and the kept pictures' bytes count. On the device the
upload the run began with is held until the sharpen, one more
full-float picture on the card, and one L* pass; at Texture +50 the
lavapipe export's device run took 8.95 s against 8.06.

**Limits.** Dehaze still moves what is flat, as it did: at +50 on the
ISO 3200 frame alone it collapses the threshold to zero, Texture or
not. Whether the guide should come from before Dehaze too is a
separate call; it would change Dehaze-only edits. A mask's own Texture
runs at the finish, after the sharpen, and never moved it. The guide
is the picture before the global Detail, so a mask's Sharpen (§548),
the global sharpen's delta, follows the fix.
