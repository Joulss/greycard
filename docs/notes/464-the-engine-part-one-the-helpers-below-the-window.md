# §464. The engine, part one: the helpers below the window

*2026-10-31*

Part 1 of §454's order: pure moves that shrink the window crate's
surface before the engine crate is cut. Bodies identical to before
but for paths, visibility, and the logging macro; the test set the
same 2445 names; the window's old paths kept by re-export, so the
window's own callers did not change.

**The sidecar helpers, down.** A frame's sidecar as an open reads it
from disk, the `.gcd` or the default, the `.xmp` beside it merged in,
an older build's brought up to date, is `greycard_edit_io::on_disk`.
§454 had it in greycard-edit; it reads the XMP, which §463 put in
edit-io, so edit would have depended on a crate that depends on it.
Reading a frame's XMP into its sidecar is reading another tool's
file, which is edit-io's job. The library gains `held` (a row held
from disk or from nothing, with its versions), `files` (the listing,
and a sidecar's raw), `export_record` (an export recorded in the
sidecar on disk), and `sync_note` (a save noted, a pair of a raw and
its copy). The reduced develop, the masks, the headless export, and
the engine half of the edit's pictures call the new homes directly.

**The edit's pictures, split.** `edited_pictures.rs` is the engine
half, no Slint type, `State`, or `App` in it: what is kept, what is
made, the recipes, and the sizes. `edited.rs` keeps the window half,
the notes to the list and the refreshes. What the engine half still
names of the window, `grid::MADE`, `previews`, and the worker's thumb
cache, part 2 places with the worker.

**The version key, out.** `version_key::Key` has no window
dependency; `versions.rs` re-exports it.

**On the way.** Moved code logs through `log` rather than `tracing`,
and the log's own-crate list names the two library modules with an
info line, so they reach the file as before. The disk-read counter
the window's tests read is behind the library's `fixtures` feature,
which greycard-ui's dev-dependency turns on. The moved library tests
remove their scratch the library's way; none holds an index or a map
open at the removal. Every file touched reads its imports explicitly
in place of the crate root's glob.
