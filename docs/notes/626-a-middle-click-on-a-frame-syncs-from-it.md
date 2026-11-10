# §626. A middle click on a frame syncs from it (2026-11-10)

**What was asked.** Issue 56: a middle click on a frame in the
filmstrip or the grid syncs that frame's edit onto the selection, as
Resolve does with a middle click on a clip. The frame clicked is the
source whether it is open or not; the targets are the selected frames
other than it.

**The Sync button's middle click, from another frame.** §599 made the
button's middle click the sheet's sync without the sheet, on the last
sync's or paste's choice. This is the same sync with the source named
by the click. A strip cell and a grid cell read the middle button off
their own `pointer-event`, as §599's `PanelButton` does through
`TipArea.up`, since Slint's `clicked` is the left button's alone. A
middle press counts when it closes no menu and comes in no left press;
its release counts over the same cell with no left press under way
(nor, in the grid, a reorder drag). A cancel, the grab lost as the
pointer leaves the window, forgets the press, so a later bare release
is nothing. The window passes the click on as `frame-sync-asked(row)`
unless a sheet is up; Rust refuses it in the queue's view and with a
tool in hand, with the button's words.

**It always sets.** The button's middle click takes the sheet's shift
choices as they were last left; this one does not. A shift moves each
target from its own value, so a second click on the same frame would
move the targets again (+1 EV, then +2). A click on a frame reads as
"make these like that one", which a second click should not change, so
it lays the source's values as they are, whatever the sheet says. The
button keeps the sheet's shifts (§599, shipped).

**One at a time.** A click while the last one still waits on its
reads is dropped. The wait is a flag shared with the waiting action
(`Sheets::frame_sync_waiting`, an `Rc<Cell<bool>>`), let go by a guard
the action carries to its end, so an action dropped unrun (a folder
left) frees it too, with no borrow of the state in a drop.

**The source's checks wait on its sidecar.** The choice is the last
one over the source's masks (`Checks::with_masks`), the sheet's
defaults before there was one, and the display curve off a picture,
as `sync_ready` readies the button's. The source need not be open, so
its sidecar may still stand in: the checks are made inside the
request's action, once `rows::request` has read it. The sheet's apply
and the clicked frame's sync share that chain (`run_sync`): the
request, the mosaics read off the window (`rows::after_mosaics`), and
`sync_now`. A middle click leaves `sync_last` as an apply does.

**The open frame can be a target now.** A sync's targets never held
the open frame before. Outside culling the panel is that frame's, so
`sync_now` records the panel first (`record_panel`), lays over its
line as over any target's (one history step, body fits alike), and
puts the line's new state back on the panel (`take_current_with`, the
file already written by the lay); the status words ride the develop,
as a paste's do. When the frame had the sections already it does not
move, but what the panel held and the save timer had yet to write was
recorded, and the timer then sees nothing new: so that state is
written there, as a reset's `reset_line(..) || recorded` does, unless
the lay wrote the file already through another of its lines. The
sheet's own path targets the open frame only in a race: the sheet
applied while reads are out and a target opened before they land.
The lay then went onto the line behind a stale panel; it now records
the panel and takes the state back, as here. In
culling the line is the truth, and the lay is all.

What the panel does after is a paste's, through the same
`take_current_with`: with the crop up the crop stays up and the
frame's own crop stays (no sync carries one); a chosen mask that the
sync's replaced masks no longer hold goes back to the global look; in
Cull drawn developed the frame leaves for Develop to show its new
state, as a paste onto it does.

**The cases.**
- Clicked inside a set of several: onto the rest of the set, the open
  frame among them.
- Clicked outside the set: onto the whole set, the open frame
  included; with the open frame alone selected, onto it.
- Clicked on the only frame selected: no targets, the button's words
  ("choose the frames to sync onto...") and nothing done.
- A version: a cell like any other, its own line the source or a
  target; one write a file however many of its lines moved.
- Under an offline root: the source is not read, and the status line
  says the root is offline, nothing to sync from. An offline target is
  left out as a sync's always is.
- The selection and the open frame never move. A middle press let go
  on another cell is nothing; a middle drag did nothing in the strip
  or the grid before and does nothing now.
- The lightbox is the grid's sheet, so it takes the click for free.
  Cull's compare view draws no thumbnail cells and is left alone.

**Left out.** No new words: the button's status lines say it all.
There is no visible cue that a middle click syncs; the guide says so.

**Found in review, and left.** Middle down, left down, left up,
middle up on one cell: the left click opens the frame and the middle
release then syncs, the left release having ended the press the latch
checks. A real mouse, an export under way, and the lightbox were not
tried by hand.

Tests: a middle click on a frame of the set that is not open syncs it
onto the rest, the open frame through its panel, Light alone after a
Light-only sheet sync, the set, the open frame and the source left as
they were, and an undo takes the open frame's step back; a middle
click outside the set lands on the open frame alone, and a middle
press let go elsewhere, one on the only selected frame, one with a
tool in hand, and one under a sheet sync nothing; a middle release in
a left press, and one after the pointer left the window mid-press,
sync nothing; an unsaved slider on the open frame reaches its sidecar
on disk when the sync finds the frame had the sections already, and
when it moves it; two clicks with the sheet's Exposure shift on leave
the targets at the source's value, one step; a sync and a paste onto
the open frame with the crop up keep it up and the frame's crop, and
with a mask chosen whose masks are replaced both go back to the global
look; in Cull drawn developed both leave Cull; a click while the last
waits on a read is dropped and the flag let go once it runs; the
grid's first cell syncs as the strip's do; a version syncs onto the
file's own, open, one step, undone; an offline frame is no source,
said, and the open frame's sidecar unchanged.
