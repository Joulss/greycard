# §473. Bursts into stacks

*2026-10-31*

The culling track's ask: frames seconds apart with near-identical
previews grouped as one stack, its sharpest frame on top (§334 has
the stacks).

**The rule.** A folder's frames sorted by capture time. A frame
joins the run when it is within two seconds of the previous frame
(capture times are whole seconds, so that covers real gaps under two
and some up to three) and its look is near the previous frame's. A
look is the embedded preview's grey averaged over a 16 by 16 grid,
256 bytes; two are near when their mean absolute difference, after
each one's mean is taken out so exposure flicker does not split a
burst, is under eight levels. Measured on an airshow's pans: burst
neighbors 0.7 to 7.7, different pictures 11 at the least and mostly
over 30, the same subject seventy seconds later 36. Open sky minutes
apart came as low as 5.8, so the time gap does the work there. A
frame with no preview splits a run; a frame with no time is in no
burst and splits none. A run needs two frames.

**The top.** Focus as the variance of a four-neighbor Laplacian over
the same grey, downscaled to a long edge of 1024; ties keep the
earlier frame. It measures detail, not focus: it favored treetops
over open sky in the same pan, and a bracket shot in a second groups
as a burst with its brightest frame on top, since contrast raises
the score. The roadmap's focus score on the subject is the fix; the
score is stored for its sort.

**In the index.** A `looks` table beside the frames, schema 13: the
signature and the score keyed by the file's row and its mtime, read
only while the mtime still matches, deleted with the row; a table
rather than columns, so the index pass and the older-schema tests
stay as they were. Looks are made the first time Stack bursts runs,
not by the index pass. The preview is read as stored, unrotated: a
camera held through a burst stores the pixels alike even when its
orientation tag flips.

**The action.** Stack bursts, in the frame menu beside the stack
actions, over the folders of the listed frames, on a thread of its
own with a writer connection, the stacks made on the window's
thread through the stacks' own store; frames already in a stack are
never read and split a run; a collection's list is refused with the
existing words. About a minute a card of raws, longer for Fuji's
large previews. A run holds a guard that clears on any exit, and a
run that has read nothing for thirty seconds is passed over by the
next ask. A status while it runs and a cancel are on the roadmap.
