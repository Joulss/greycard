# §592. The strip holds still under a delete (2026-11-07)

Issues 83 and 84 (one report, filed twice, on 0.10.0): deleting the
selected frame while culling threw the filmstrip so the frame chosen
next sat at its far edge, a jump on every delete.

The cause was the strip's rule for a new list. Every `rebuild_browser`
puts a new `thumbs` model, and the strip lays itself out afresh from its
start whenever the row count changes (`strip-relay`): scroll to zero,
then the least reveal of the selection from there. That is right for
a new folder. After a delete it is wrong twice over: the selection is
empty at that moment (the next frame is chosen a timer later), so the
strip went to its start, and the select then revealed the frame from
zero, which puts it at the right edge. The grid did the same on `changed
thumbs`: to the top, then the selection revealed at the bottom edge.

We now tell the window when a list is the old one less some frames.
`drop_files` (a delete, the rejects moved out) and a version's delete
(through `versions::settle`) set `rows-held` after their rebuild;
every `set_rows` clears it first. The window's change handlers run
after the Rust call returns, so they read the drop's value, and the
next list put from anywhere else reads false again; a drop that leaves
the count alone, frames moved out from under a filter, leaves nothing
stale. When the rejects moved out make or empty a rejects folder,
the grid's folder tiles are read again, and a tile count that crosses
a row still lays the sheet out from the top: a new row of tiles moves
every cell, so there is no place to hold.

On a drop the strip and the grid hold their scroll, held to the shorter
range, and then take the least scroll that shows the selected frame
whole, the reveal they already use for a key press. So the rule is:
hold when the frame chosen next is whole on screen, else move the least
that shows it. The frames before the deleted one stay where they stood;
those after slide in by one cell. A strip at its end loses a cell's
length and the clamp pulls it back by that much; nothing holds both.

The frame chosen next is the one after the deleted frame, the one
before only when the last went (`nearest_row`). The report speaks of
the previous frame; we kept what the code does, since the next frame
slides into the deleted one's place and so lands under the pointer,
where the user's eye and next click already are. A width change and
the grid opening still lay out afresh.

Two paths still jump, left for later. Taking frames out of a collection
while it is open reopens the view (`open_view`), whose rows come back
from a read off the window's thread, so the flag would have to ride
with the read to its landing. And `versions::settle` serves more than
a delete (a version made, a stack opened or closed); those grow or
reorder the list, and whether they should hold is its own question.

**Later the same day.** The reporter found the same jump on stacking
and unstacking. Both rebuild the list through `stacks::settle`, which
now sets the flag too; it is `rows-held`, since the list is no longer
only the old one less some frames but the old one changed in place,
folded or unfolded. Unstacking makes the row longer, and the hold
within the row is the same rule.
