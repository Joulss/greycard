# §629. The develop summary leaves the window (2026-11-11)

After each develop the status line said the frame's size, the develop's
time, the local contrast's radii and where it ran, the dehaze's amount
and transmission, the sharpen's radius and coverage, and the learned
denoiser's and the fills' seconds. A tester called it redundant and
intrusive (discussion 117), and we agree: the size is in the photo
information, the values are in the panels, and timings are
diagnostics, which go to the log, never to the window.

**A landing says nothing of its own.** The status line shows only what
someone else asked it to carry (a preset's, a sync's, or a merge's
words, stashed for that generation), and what the edit went without:
the engine's denoise standing in for a missing or failed learned one,
no denoise on a mosaic it has no version for, and a fill left as it
was (model missing, no GPU, or failed). With none of these the line is
empty and its plate is not drawn. Progress, exports, failures, hints,
and the Original's word are as they were. No wording is new; six
warnings lost the comma that joined them to the summary.

**The log takes it.** The worker's "developed WxH: ..." line carries
what the window dropped: the radii, the transmission, the sharpen's
radius and coverage, why the learned denoiser stood down, and each
fill by name, those that wait on a GPU among them.

**Drawing in software is said once.** The tail " on the CPU, no usable
GPU" went with the time it explained; the picture is the same, only
slower. The start's own line says it, but the first file's
"developing..." covered that line at once, so it is kept and put back
when the run's first develop lands in Develop, after any action's
words and before any warning, and not again. A landing in Cull, of the
Original, with a dropper in hand, or of a stale generation leaves it
for the next. The log warns it at the start.

**The thumbnail pool keeps off the picture.** An empty line after each
develop would have let "making N pictures..." onto the picture in
Develop, where the summary had kept it off by accident. It shows over
the grid, in culling, and in the lightbox only, and takes off no words
but its own.

**Two reports for the tests alone.** Of the outcome's local contrast
and dehaze reports only the engine's tests read them now, some fifteen
of them, comparing the CPU and GPU paths; the fields are compiled for
the tests and not for the build. The variant's shape differing between
the two is a smell we accept for those checks.

**Not covered by a test:** the note held past an Original, a dropper,
a stale generation, or Cull, and the three-part join; read, not run.
