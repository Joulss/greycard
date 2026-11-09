# §620. Back and forward come back on the frame left (2026-11-09)

**Back landed on the first frame as often as on the one left**
(discussion 59). §594's stacks held places, not frames, so which frame was
current after back fell out of older rules: a folder opened on the
file last developed, a view kept the frame on screen. Both are frames
of the place being left, rarely of the place gone back to, so back
landed on the frame left only when the two places shared it.

**Each place on a stack keeps the frame it was left on.** A stack
entry is the place and the file current when the list was replaced
(taken in `open_loaded` before it clears the current frame, or in
`tree::change_in_place`, and handed through `recent::opened` to
`Visited::landed`). Going back or forward
records the place left the same way, on the other stack, so forward
returns to it; places passed over in a several-step back keep their
own. A file, not a row, as the selection keeps files (§156): the
filter and the sort move rows. One frame only: a selection of several
comes back as its current frame. A place left with nothing current
keeps none, and so does a place left from over the export queue, whose
frame is the queue's. Session-only, with the stacks.

**The frame rides the open.** A folder's list arrives after it is
asked for, so the frame is not looked up on the window's thread: it
goes with the read as `Select::Left(frame, last)`, matched to the list
off the window's thread where `Select::Last` is, by the folder's
canonical form. A view takes it as `On::Left`, which replaces
`open_view_on`'s `Option<Option<PathBuf>>` (the queue's way back is
`On::Frame`). A frame no longer in the list (deleted or moved) falls back to what the open did before, the frame on screen
or the file last open, then the first, with no message. One hidden by
the filter lands as any hidden frame does, on the nearest shown.

**Only back and forward use it.** An open from the tree, ROOTS, a
collection, or Recently opened is as before. The frame lands through
the same `invoke_select` as before, so the grid and the loupe behave as
they did; only which row differs.

Tests: a place keeps the frame it was left on, back and forward give
the place left its frame and the places passed over keep theirs; the
queue's frame is not the place's; the stack's cap keeps frames; two
folders from the disk come back on their frames, and a frame deleted
falls back to the first; a root's folder, every root, and a collection
come back on their frames.
