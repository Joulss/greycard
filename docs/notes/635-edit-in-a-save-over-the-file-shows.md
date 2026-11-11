# §635. Edit in: a save over the file shows (2026-11-11)

Issue 119: a tester saved over the handed file in the other editor
and saw nothing change until the frame was left and reopened.

§499 promised the app's save shows. It did once at most: the first
save's stacking rebuilt the list, so the grid and the strip asked for
the file's picture afresh, but the frame open in Develop kept its old
develop, and a later save showed nowhere until the user left the frame
and came back. The watcher indexed and stacked the file, and nothing
let go of what the window held under its old content: the cell's
picture, carried over by the relist as any unchanged frame's is, the
loupe's and cull's copies, the edit's pictures, and the open develop.

**A save is a rewrite.** A merge made again over its own file is the
same event, and §541 already handled it. We lift that path out of the
merge into `edited::file_rewritten` and call it from both: the cache's
thumbnails under the old key go, the edit's pictures under the old
hash go and each cell asks again (the lightbox's and cull's copies,
a versions compare's tiles, with them), and the stack's reference
takes the new hash. Even the first save is a rewrite: under a watched
root the file is listed as soon as greycard writes it.

**Read again, not opened again.** A save arrives unasked, so it must
not do what the user opening a frame does. §541 reopened the frame
through the open itself, which fits the view, saves the panel (a
history step and a sidecar write, mid-drag at a half-dragged value),
pushes the edit onto the sliders, puts down the tool in hand, the
mask chosen, and the crop's start, and lets go of a pick waiting on
its sidecar. `browser::reread_open` sends the worker the open of the
same file under the panel's edit as it stands, at a new generation,
and nothing else. The view keeps its zoom and center for a picture of
the same size; a develop of the old bytes still out is dropped by the
generation; a slider mid-drag stays under the hand and is what the
reread develops, its own save to come. A merge made again takes the
same path, so its frame keeps its view too. No reread in culling,
whose caches are let go instead, and none while a pick waits: that
open wins and reads the file as it is now. The frames the arrow keys
reach are sent again with the reread, as an open sends them; else the
worker drops those of the last open and refuses every later change
to them as an older open's, and the next step is a cold decode.

**A develop never drops a waiting open.** The worker has one slot for
its next develop, and a newer job took it whole. An open waiting
there while the worker was busy was dropped by any develop sent after
it: a slider's after its rest, the Original's key, a model arriving.
The develop then ran on the base before, `Opened` never came, and the
save stayed hidden until the frame was opened again; an ordinary open
had the same hazard. A develop now carries the frame it is for, the
path and version current when the window sent it (`Job::Develop::of`,
`edit::develop_of`); none for the worker's own. A develop or aside
whose frame is the waiting open's becomes that open under its edit,
generation, and turn (`handle::into_slot`). Any other develop takes
the slot as before: a develop sent after an open is not always the
open frame's, since a pick waiting on its sidecar, an offline frame,
and a landed answer each move the current frame without an open, and
folding those opened the last file under the new frame's generation,
its size, camera, and As-shot white on the new frame's panel. With
the frame matched, `Opened{g}` is only ever of the frame current at
`g`, so the window needs no path check of its own. An open of another
file over a waiting one replaces it as before. An aside so folded in
opens the file under the Original's edit, and its generation is the
one the window waits on for the Original. A fold also keeps the
prefetch: the neighbors sent at the open's generation are retagged to
the fold's, and the window's later asks at the old one map to the new
(`Queue::folded`).

**The Original stays on.** Both pictures it keeps, its own and the
edit's it stands over, are of the old bytes and go; keeping the
edit's would put the old picture up for good when it is turned off,
since a picture put up asks for no develop. On show, it stays on:
once the worker has opened the new bytes, its Original of them is
developed aside in place of the edit's develop of that open. A save
turns nothing off. The worker develops the edit of that open all the
same and the aside leaves it in hand, so the edit's pictures of the
new bytes are asked to be kept from it, as a develop landing asks;
else the strip and the grid would show the camera's picture of an
edited file until the Original was turned off.

**The masks and the people.** The worker forgets every model's work
at any open, and the mask cache's print holds the file's length and
time, so the learned rasters are made again on the new pixels; the
window keeps the old ones drawn until then, as for the same frame
opened again. The people a Part's pick shows are another matter:
their boxes are where the old pixels' faces were, and a click would
write a person taken from them into the edit. A new part's pick in
hand stays in hand and its people are asked for again, the pick armed
on them as before. A placed part's asks go, and its pick in hand with
them: the worker makes the parts again on the new pixels and asks
again where it is still unsure, and a fresh ask for the part whose
pick is in hand now replaces it rather than being ignored.

**The old key is kept per watch.** The window's pictures are kept
under the file's content hash and stamp, so a save must know the ones
before it. The first look reads the export's key, off the window's
thread like every other read, and only while the stamp still matches
the export's, read before and after, so a save that beat the first
look is never taken for the export. Each taken save's key replaces
it, and each watch has a number of its own, so a look that read the
disk for one watch never lands on another handed over under the same
name meanwhile. When the old key is unknown, Edit in lets go under
the new hash: a hash that moved misses the old pictures anyway, and
one that did not (a change past the hashed 64 KB head at the same
length, which an uncompressed TIFF retouched low in the frame is) is
the old one; a test shows such a save stays hidden without it. A
merge made again keeps §541's rule: an old file that would not hash
had nothing kept under a hash.

**Pictures made from the old bytes.** A plain thumbnail of the old
bytes still being made when the save lands lands before the new one:
the pool never begins a frame another of its threads has in hand, so
the new ask waits for it. The edit's pictures are another matter, and
one race is known and left. They are kept under the head's hash and
the develop key, with no stamp. When a save changes the file only
past its first 64 KB at the same length, the hash holds; a keeping or
a reduced develop of the old bytes still running when the save lands
can store its old pixels under that key after the window let go of
it, and the grid and the strip then show the old picture until the
edit changes. Develop is right: it reads the file again. We tried
refusing such a store when the file's length and time had moved, but
that stat ran under the cache's lock, which the window's thread takes
on a save, a delete, and the Preferences' cache actions, so a share
that dropped out would hold the window for the file system's
timeout, and a refused reduced store asked again without end. The
fix for later: a "let go" count per path, raised under the cache's
lock when the window lets go of a file's pictures, read by each
making when it reads its pixels and compared under the lock at the
store, with every stat outside it.

**Tests that leave nothing behind.** Stopping the worker joined its
thread only; a thumbnail thread or the keeping of the edit's pictures
could still write into a test's folder after it was removed, which on
Windows is the open-files flake. `Worker::stop_all` (testing only)
waits for the pool's threads to end and the keeper to go idle, and
the Edit in and merge rigs check nothing is left. The Edit in rig
holds its worker busy at a gate the test opens, rather than on a large
decode, so a job waiting for the worker waits for certain.

**Known limits.** The cache entries of a save that beat the first
look are left for the cache's own eviction. A picture whose size
changed with the save is centered again at its zoom, since its old
center means nothing on it. A People or Mask answer already being
made from the old pixels when the save lands (a model job running at
that moment) can still arm a pick on the old faces. A read of the new
bytes that fails never sends `Opened`, and with the Original on, its
old picture stays up until it is turned off and on. The engine's own
test rig for the edit's pictures (`testing::edited::rig`) never
removes its scratch folder, which is older than this work. Checked on
Linux only; the reporter's macOS path is the same code. Nothing was tried in a real
window.
