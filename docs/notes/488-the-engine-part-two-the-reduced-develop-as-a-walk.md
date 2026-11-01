# §488. The engine, part two: the reduced develop as a walk

*2026-11-01*

The first of §478's three landings for part 2's last step: the
reduced develop and the models' picture are walks of the stage list.
28 exports of §476's bar are byte-equal before and after, and the
reduced develop itself, which the bar never runs, was hashed on 14
frames in 6 cases each, image, sizes, bin, clip level, guide, kind,
white, and planes, 84 for 84 against master over two runs a side.

**The walk under Reduced.** `reduced::develop` keeps its signature
and calls `develop_as(.., Purpose::Reduced)`: the frame opens as a
walk's input, a picture's image taken out of its record rather than
copied, and `stages(edit, purpose, facts, lenses)` walks on the CPU
with no models, no device, and nothing held. The old body, its
`settings()`, and its `bin_under()` are gone; `Facts::bin_under` and
the list say the same things, bin in place of the demosaic, the
learned tier and the profiled denoise skipped, the defringe scaled by
the bin, fills refused with the same words, planes inline, sharpen
skipped, dehaze kept, CA its own stage before the bin as the combined
path ran it. The source is the frame's (`Source::of_frame`), so a
DNG's own baseline reaches it when that lands.

**Nothing kept.** The list's `Keep` steps are stripped before the
plan: nothing reads the kept points under no models, and keeping them
held four clones of the picture and forced a copy at the first
in-place op, 400 MB more at the peak on a bin-1 crop of a 45 MP
frame. Stripped, the peak is master's.

**The models' picture.** `masks_for` and `person_on` call the same
walk under `Purpose::ModelsSee` on the edit as it is; the clone
without its retouch is gone, the list leaving the retouch out. That
list has no guide tap, so the masks' `Developed` carries an empty
guide, which neither caller reads. A debug assertion holds
`develop_as` to the two purposes; under Export or Viewport with no
models a fill would run and vanish.

**The upward imports, turned.** The reviewer of §478 noted the walk
and the list importing from the worker, the models, and the edit's
pictures. The three helpers, the turn as an orientation, a shape with
a prompt, and a framed size, now live with the list, re-exported at
their old paths, so the list and the walk import nothing from above.

**Tests.** Two tests that restated the list's spelling are behavior
tests on a linear DNG: a fill refuses the reduced develop and not the
models' picture (renamed from "the settings leave the sharpen, the
denoisers, and the dehaze out", whose name contradicted its body),
and the models' picture equals the reduced develop of the same edit
without its retouch, to the bit. Master's reduced develop is
nondeterministic on one bin-1 cropped Nikon frame, as its export was
(§476); the hash check runs twice a side.

**Next.** The exports as walks under Export, then the main develop
onto the kept points with the four `same_*` predicates retired.
