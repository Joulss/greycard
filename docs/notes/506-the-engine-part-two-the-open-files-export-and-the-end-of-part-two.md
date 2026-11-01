# §506. The engine, part two: the open file's export, and the end of part two

*2026-11-01*

The last of §478's landings. With it every develop and every export
in the editor is a walk of a stage list over kept points: the
reduced develop (§488), the set export (§496), the window's develop
(§505), and now the open file's export.

**Resumed from a copy.** The open file is written as a set frame is,
through the export's stage list, but resumed from a copy of what the
window's walks kept. The copy shares every picture by Arc and costs
nothing to make; what resuming drops and keeps is the copy's, so the
window's points, on the device too, are as they were for its next
develop, where before an export threw the device points away and
the next move re-uploaded, reran the local contrast and the dehaze,
and sharpened. The export's list runs on the CPU and so resumes only
from points the CPU made; a kept point that left fills unmade does
not serve it, and the fills run again (a failed one fails fast from
its cache) so the report says what each went without, as a set
frame's does.

**What goes.** Last: on the CPU the window keeps its developed
picture, which the export and a save of the edit start from, and
which its own next walk drops first, since resuming from it would
skip the sharpen and lose the viewport's blend and stats; it is kept
only when no device is lent, so it never sits beside the pre-sharpen
picture or the upload. Edit's same_develop, the last of the four
predicates of the 2026-10-29 review: a save and a waiting one compare
the two edits' lists to the finish (stages::same_develop, keeps, the
masks tap, and the mask picture's tap left out, since the learned
rasters are made beside the picture), and the history keeps its own
field test for whether to ask for a develop at all. finish_export,
scene_export, open_picture, write_export, refined_picture, and
kept_picture go with the old path. FrameDevelop (the camera match
and the measurement runners) walks the export's list and finishes
through the walk's finish; every caller finishes under an edit that
differs from the develop's only in the finish. The refined masks'
picture an edit stops asking for is let go.

**Held equal.** The bar's 40 set exports are byte-equal to master's,
and a new harness of 16 open-frame exports (eight frames, a 16-bit
TIFF and a 2048 JPEG each, the clock pinned) is byte-equal with and
without a device lent. The open-frame export now taps the refined
masks' picture on the CPU as a set export does, where the old path
read the window's back from the GPU; with Refine edge and the Detail
section both on, the two differ by the GPU's Detail on the 2048
grid, below sight. One bar run beside a release build differed on
one frame by a bit in some 3800 samples; three sequential reruns
were identical. The likely cause is a rayon parallel sum whose
reduction order follows work-stealing (the sharpen's plane mean, the
defringe), cast to f32; on the roadmap, not proven.

**Part 2 is done.** Parts 3 (the viewport crate) and 4 (`greycard
export` and the settings reader) no longer wait, nor do the camera
match and the runners' move into the library. The three notes from
the reviews stand: a device error that takes the context mid-walk
can leave a pre-sharpen picture in memory serving CPU walks; the
window's own walk drops the context on any error but Unsupported, as
before, where the prefetch keeps it short of a lost device; an
export of a frame with an unmade fill re-walks from the base.
