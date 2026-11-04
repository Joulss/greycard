# §559. A focus score for every frame

*2026-11-05*

The Culling track's first item, the frame-wide half: one number a frame,
from its embedded preview, for a sort, a `focus` term, and the order
inside a burst. The Subject mask's refinement, the score taken on the
subject where it finds one, is left for later.

**The score** (`greycard_core::sharpness::focus_score`) reads the
peaking map (§542) as a level, not a share: the share of a preview
within the peaking window of its own top is the same for a sharp frame
and its blur, since each is measured against itself. The level is the
log2 of the map's 99.5th percentile: a sharper frame has more pixels at
a high energy, so its top is higher. The map is §542's measure (the log
of the linear luminance plus 2^-5, the Laplacian's energy, a 5 by 5 box)
on the preview box downscaled by a whole factor to a long edge of 2048
or less, which brings previews of 2048 to 8192 near one scale (a
smaller one, 1024 to 1920 among the samples, is measured at its own
size; one of 2049 to 4095 at half, a cliff none of the samples sits
on) and averages a high ISO's grain down; the log removes the
exposure's gain.
The level, -8 to 8, is put on 0 to 1, so the term reads `focus>0.6`.

**On the samples** (33 frames, ten bodies) the sharp frames read 0.43 to
0.79, and one with nothing sharp in it 0. Each preview blurred by a box
of radius 1, 2, and 4 scores lower, in that order, but for one small
dark preview whose last two blurs both sit at the floor. At an edge of
1024, the bursts' old size, a radius-1 blur of a preview wider than 4000
lowered the level by 0.13 to 0.54; at 2048 by 0.52 to 1.94, which is why
the edge is 2048. What reads well: frames of one body and one preview
size, which is what a burst is. What does not: a body whose preview is
1024 wide (some DNGs) is measured at its own size, so it scores near the
top of the set while a 45-megapixel body's is measured at a quarter of
its preview, and across bodies the number is rough. The test frames hold
no burst with a missed focus, so the burst order was tried on the
blurred copies, not on a real miss.

**The cost, and the scorer.** The index decodes no raw, but the preview
is a read the pass never made: 4 to 110 ms to read one and 3 to 28 ms to
score it. Scored inside the pass, on its thread, a fresh pass over the
samples took 2.2 s against about 0.16 s without it, and the grid filled
at a tenth of its speed. So the pass writes its rows with the focus NULL
and reads no preview, and a scorer after it (`greycard_library::score`)
reads the NULL rows' previews on three threads, writing in batches of
32, each a short transaction. In the editor it runs on a thread and a
connection of its own, woken by each pass and each save, and the window
reads its rows again as the scores land (every 2 s and at the end), so a
sort by focus and a `focus` term fill in behind the list. On the samples
a fresh pass now takes about 0.1 s and reads 7 to 8 MB, and the scorer
after it 0.95 to 1.4 s (ours and the review's runs) and 451 to 473 MB.

**On a network mount** a NEF's, an ARW's, a DNG's, or an RW2's preview
is read from the whole file, 22 to 61 MB a frame, where the pass reads
about 2 MB a raw; four at once under the pass's 20 s limit, a slow share
would have been taken for not answering, and schema 14 would have read
every such raw of an archive whole on the next pass. There the scorer
reads only a CR3's or a RAF's, whose preview comes from the file's ends,
one file at a time, each on a thread of its own and waited for up to 60
s, outside the pass's reads and their limit, so the root is never marked
as not answering. A read that does not answer is counted against its
share, the topmost folder above it on the mount, until its thread comes
back, and every sweep meanwhile asks that share nothing, so a hung share
strands one thread and not one a sweep; a share whose own pass has a
read out is asked nothing either. Which folders are on a mount is asked
once a folder a sweep. The rest stay NULL, so a frame of another format
on a network drive has no score. On the samples taken for a network
mount the pass read 7 MB and the scorer 91 to 96 MB for the 24 CR3s and
RAFs, in 1.7 s, against 451 to 473 MB scoring all 33 locally.

**In the index** the score is `files.focus`, schema 14: NULL for a row
not scored yet, which the scorer scores, the pass reading nothing for
it; below nought for a file read and found with no preview, or one that
would not read at all (a failed probe's among them), which is not read
again until it changes. A file changed on disk is NULL again. The step
also empties `looks`, whose focus was the variance of the Laplacian; a
burst's order reads the row's score where it has one and the look's, now
the same score, otherwise.

**The sort** is Focus in the Sort menu, sharpest first when picked from
another order, the unscored last either way up, and `focus` or
`focus-desc` for `library list --sort`. **The term** is `focus`
with the number terms' comparisons, `:` within half a hundredth. **A
burst**: Stack bursts puts the frame of the highest score on top, as it
put the highest variance before; a frame already in a stack keeps the
user's order.

**Left for later.** A JPEG or a TIFF is decoded whole by the scorer,
where the pass reads only its header. A CR3's ends are read twice, by
the probe and for the preview, and Stack bursts reads the preview a
third time for its signature. The command line's passes do not score.
