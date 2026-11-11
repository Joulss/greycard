# §630. The Sync button's middle click sets (2026-11-11)

**What changed.** §599's middle click on the Sync button took the
sheet's shift choices as last left, so with Exposure on Shift two
clicks moved the targets twice. §626 made a frame's middle click
always set. The button's now does the same: the sections the last sync
or paste took, every one laid as a set, so a second click changes
nothing. A middle click reads as "make these like this one"; a shift
is a deliberate move, and the sheet is where it is chosen and seen.
The sheet's own apply still shifts as its modes say.

**One path.** The button's click no longer readies the sheet and
fires its apply; it is a frame's middle click with the open frame as
the source (`sync_quick`, which was `sync_from_frame`). It gains the
frame click's one-at-a-time flag, and the source's checks are made
once its sidecar is read. `sync_ready` now only opens the sheet, so
its `last` flag is gone.

**The last choice is read, never written.** The shipped button wrote
`sync_last` through the apply, and §626 had a frame's click store it
too, "as an apply does"; this section amends that line. Neither middle
click writes it now: only the sheet's apply and a paste remember a
choice. The stored checks were the same sections, so storing could
only lose from them, two ways. A click from a picture stored the
display curve off, and the next click from a raw, or the next paste
sheet, dropped the curve. And a click from a frame with no masks
turned a choice of one mask of several into none, so the next click
from a masked frame no longer carried it. The shift choices were never
touched.

**No new words.** The button has no tooltip; its label and status
lines stand. The guide says both middle clicks set.

**Tested.** The button's middle click twice, with Light last and
Exposure on Shift, leaves the target at the open frame's value in one
step, `sync_last` and the shift as they were; the sheet's apply after
it moves the target by the open frame's +1 EV from its own value. The
§599 and §626 tests pass unchanged. That the last choice survives a
click from a maskless frame or a picture is read, not tested, and
nothing was tried with a mouse.
