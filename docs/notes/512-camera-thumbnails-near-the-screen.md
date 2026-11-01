# §512. Camera thumbnails near the screen

*2026-11-01*

The Library made a camera thumbnail for every frame under the root at
its first open, about a tenth of a CPU second a cold frame and twenty
minutes on a 12,515-file root (discussion 51); since §452 and §495 the
edit's pictures and the previews behind the thumbnails follow the
near-screen rule, and now the thumbnails do too.

**The rule.** Every frame asked for is still looked up in the cache,
a tenth of a millisecond, so a warm root fills whole as before. A
frame the cache lacks is made only near the screen, the rows on view
plus as many again above and below, the set §452 hands the pool; while
culling, within the reach §495 gives the previews as well. A far miss
leaves the queue every thread walks and waits apart, looked up once,
until a scroll brings it near; the pool is idle otherwise. At the
window's first list, before the strip or the grid has said what it
shows, the rows about the frame to be selected stand in for the near
ones, so the first cold thumbnails are those and not the list's start;
until the window says anything, every frame is near. The status
line's count is of what will be made, the near ones, and the log's
line at a pass's start gives no count before the near rows are
known.

**What it measured.** 2,000 cold CR3s opened in the grid, headless:
before, 2,000 thumbnails made, 41 s to idle and 340 CPU seconds; now
50 made in 0.8 s, the pool idle from then on, 33 CPU seconds in all
including the window's start, and the round's line "50 of 2000
files in 0.92 s". The rows on screen showed their pictures at 0.74
and 0.76 s. `--time-scroll 20` on the same cold folder ran and quit
at 2.4 s. A scroll to the end of the root makes that screenful and
its neighbors, about a second's work on eight threads.

**What it changes.** The grid lays out its rows by the pictures'
shapes, so far rows take their shapes as a scroll reaches them; the
scroll is held under late shapes as since §460. The sort and the
filter read the index, never a thumbnail. The folder's first round,
the log's "thumbnails: N of M files in ... from the folder's open",
closed only when every frame had arrived, which a cold root's far
frames now never do: the pool now says the end of every pass, told or
not, and the round closes once the pool is idle, the outbox drained,
and the rows on screen filled. A picture still on its way to the
outbox when another thread ends the pass can leave the line's count
a few short; it is a log line. The hidden `--time-scroll` starts
from the rows on screen drawn with their pictures (or the wait for
them given up), not from the whole folder in.
