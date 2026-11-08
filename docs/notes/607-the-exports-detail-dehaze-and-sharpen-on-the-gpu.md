# §607. The export's Detail, dehaze, and sharpen on the GPU (2026-11-07)

§600 put an export lent a device on it for the CA, the lens, and the
demosaic, and left the Detail section's local contrast, the dehaze,
and the sharpen on the CPU, since their device runs ended at the
viewport's half-float texture. Its "left for later" asked for a
full-float download and a sharpen that writes full floats.

## What was missing

Less than it looked. The local contrast and the dehaze already wrote
an `Image` in full floats whenever the sharpen came after them, and
the sharpen's `Output::Full` (picture and mask in `Rgba32Float` and
`R32Float`) was there for `sharpen_image`, the tests' path. What was
missing was a sharpen from one device picture to another
(`Context::sharpen_to_image`, with no picture written when it makes
the delta alone), a walk that reads its run back, and a plan that lets
an export run there.

## The rule

`walk::plan` is still the one place that decides, and the rule is now
shorter: a walk lent a device runs every stage with a device version
there, for the viewport, the prefetch, and the export alike. The
shader alone stays the viewport's and the prefetch's. A run of the
Detail section, the dehaze, and the sharpen that ends at the CPU's
finish rather than the shader writes full floats and is read back at
its end (`download` in `device_run`), so the finish, color
management, and encoding stay where they were. The reduced pictures,
the models' pictures, and a merge's frames stay on the CPU.

That run keeps nothing on the device past itself. Its upload is not
kept as the viewport's is, so a frame's picture is not held on the
card through the run for a re-run that never comes. A keep inside the
run puts no point there either. The export's list has none today,
since the worker strips its keeps, but the walk no longer counts on
that. A run that changes nothing, which is a mask's Sharpen with the
global sharpen off and no Detail or dehaze, makes the delta alone. It
writes no full-float copy and reads nothing back: the picture still in
memory goes on to the finish.

The editor's export of the open frame now resumes from the viewport's
picture before the sharpen when it is on the device, since its sharpen
goes on there. Before, it resumed from the patched picture and ran the
Detail section again on the CPU.

The download reads a band of rows at a time and keeps three channels
of each. Before, it read the whole texture first, which at 45 MP is
720 MB beside the 540 MB picture. The upload's texture can now be read
back too, for a run that changes nothing.

`greycard export --cpu` is unchanged. A device error short of a lost
device runs the frame on the CPU and keeps the context, as §600 has
it, and this now covers an error in the run or its read back too.

## Measured

On lavapipe, which is a shape and not a number: lavapipe is slower
than the CPU, and a software adapter is declined in production
(§600). `greycard export` from the release build, the Detail section
(Texture 0.3, Clarity 0.4, Structure 0.2), Dehaze 0.25, and the
sharpen at 30 iterations, on a 32-thread machine:

| Frame | Before | After | The three ops |
|---|---|---|---|
| 24 MP (6000x4000) | 4.2 s | 13.0 s | 0.55 s on the CPU, 11.2 s on lavapipe |
| 45 MP (8192x5464) | 8.2 s | 29.9 s | 1.5 s on the CPU, 22.7 s on lavapipe |

The real card's numbers are the parent's to take.

How far the device's export is from the CPU's export, on 16-bit
TIFFs, in §600's measure:

| Frame | Edit | At most | Past a thousandth |
|---|---|---|---|
| 45 MP | sharpen off | 164 of 65535 | 30 of 134283264 |
| 45 MP | default (sharpen on) | 217 of 65535 | 49 of 134283264 |
| 45 MP | Detail, dehaze, sharpen | 380 of 65535 | 984 of 134283264 |
| 24 MP | sharpen off | 555 of 64590 | 25 of 72000000 |
| 24 MP | default (sharpen on) | 1016 of 64795 | 4663 of 72000000 |
| 24 MP | Detail, dehaze, sharpen | 739 of 65535 | 1779 of 72000000 |

Every row is inside the bound the mosaic identity test holds: at most
a tenth of the largest value, and at most one sample in ten thousand
past a thousandth. Most of the growth from the sharpen-off rows is the
demosaic's difference that the deconvolution's iterations amplify,
not the three ops' own rounding. On the same base, the device's run
against the CPU's is at most 4.6e-5 relative at 45 MP and 7.1e-5 at
24 MP, with no sample past 1e-4. That is inside the sharpen's own test
tolerance (1e-4), and the local contrast's and the dehaze's are
tighter for their own ops.

## Memory

The run's peak is in the sharpen, at 45 MP: the dehaze's picture (716
MB), the sharpen's picture and mask (716 and 179 MB), its working set
(about 900 MB), and the local contrast's planes, which a discrete card
keeps (about 950 MB). That is about 3.5 GB, or 3.6 GB with a mask's
Sharpen and its delta. The CA's and the demosaic's sets are made and
dropped before it. In the editor, the viewport's own device points add
about 1.8 GB (its upload, its picture before the sharpen, and its
half-float texture). An 8 GB card holds it. A card that does not gets
an out-of-memory error, and the frame goes to the CPU under the
export's hold. These are figures from the textures' sizes and
formats, not read off a device.

The local contrast and the sharpen keep their working sets between
runs at one picture size. An export of a frame that is not open, or a
set's, re-made them at its own size, which evicted the viewport's and
then left about 1.8 GB on the card until the next develop. Now an
export's run lets go of a set it made at a size the context did not
hold before the run (`Context::kept_sizes`). A set of the size it held
stays, so the open frame's export, which shares the viewport's sets,
leaves them as they were. Freeing the sets after every export would
throw away the viewport's for its next run, and giving the export
per-run sets would cost a second set beside the viewport's during the
run. The peak during the run is unchanged. After it, an export of
another size leaves nothing behind. A command-line set of same-size
frames makes the sets afresh for each frame, which costs allocation
only, since their contents are made each run anyway.

## Where the editor and the command line can still differ

Out of memory in the run under the export's hold sends the steps from
the Detail run on to the CPU, with the context kept. The CA, the
demosaic, and the lens keep their places on the device, and the retry
resumes from the device-made base or patched picture. But the editor
holds about 1.8 GB of the viewport's device points that the command
line does not. On a card near its limit the editor's export can run
out of memory where the command line's does not. The editor's file
then has the three ops from the CPU where the command line's has them
from the device. The two differ by those ops' rounding, at most 1e-4
relative, and not by the demosaic's. The exception is an out-of-memory
in the demosaic itself, which its own arm runs on the CPU for the
frame, as §600 has it. Out of memory is untested.

A picture before the sharpen made on the CPU can serve an export's
sharpen on the device. `allowed_at` asks only that a step the device
ran be placed there again. A viewport whose Detail run declined with
`Unsupported` runs that run on the CPU, sharpen and all, and keeps its
picture before the sharpen as the CPU's. Every decline is decided by
the edit and the picture, so the command line meets the same decline
and runs all three ops on the CPU. The editor's export, resumed from
that point, would still sharpen on the device. The two files would
then differ by the sharpen's rounding. The review found no edit that
reaches it today, and we record it beside §600's gap.

## Verified

- The mosaic identity test, a sibling with the Detail section, the
  dehaze, and the sharpen on, and two with a mask's Sharpen, the
  global sharpen on (the delta beside the picture) and off (the delta
  alone, nothing read back): the window's export and the command
  line's are one file, pixel for pixel, on lavapipe.
- The walk's own test: an export lent a device runs the three ops
  there, reads back, and keeps no upload, no point, and no set the
  context did not hold. A viewport's sets of the same size stay
  through it, with the local contrast's planes kept as on a discrete
  card, and an export of another size lets its own go. It is the CPU's
  run on the same base within 9.6e-6 relative.
- A new GPU test holds `sharpen_to_image` to the reference with and
  without the delta, and with the picture left as it was. Another
  checks a picture taller than a band reads back whole. The local
  contrast's tests now read back through the banded download.
- The two ignored sample tests, on one 24 MP and one 45 MP frame,
  both now asserting their bounds.

## Not verified

- On a real card: the timing, the peak memory, and the out-of-memory
  fallback at 45 MP.
- The editor itself: the window's export ran only through the
  worker, in the identity test.
- Out of memory under the export's hold, on any device.
- The local contrast's kept planes in a whole export on a discrete
  card. Lavapipe keeps none by default, so only the walk test, which
  turns them on, exercises them.
