# §544. Reset to Original

*2026-11-04*

Two testers asked (discussion 59) for a way back to a frame's
starting edit without scrolling HISTORY to its bottom row, often from
a folded panel. We put "Reset to Original" in two places: in the
HISTORY section's head while the edit is not already there, and in the
frame menu over the selection, hidden in the export queue's view.

Both record one step, labeled "Reset to Original", through one
function (`panel::reset::reset_line`). A click on the Original row
walks the history back (`Line::go_to`), which leaves nothing for Ctrl+Z
to take back; a reset is a change, so it is a step undo reverses.

The Original is the line's earliest state when that is the frame's
default, seeded learned blend and all: what the Original row holds.
When a long history has dropped it ("Earliest kept"), it is the
frame's default afresh, a raw's with its ISO's blend, or the earliest
state's blend when the ISO is not known. The head's button follows
the panel's live edit, so it shows as a slider moves, before the
save records the step.

The menu goes the way a paste onto a selection does: the frame on
screen through its panel (what it holds recorded first), the rest
read in, reset on their sidecars, each written once, and its cell put
out again. A frame at its Original is left alone, history and file.

`Section` gained `action`, a command in words at the header's right,
shown folded or not; HISTORY is its first user.

A batch write says in the log how many files reached the disk and
names any that did not (`saves::save_for`), and the reset says how
many frames moved. A raw whose earliest state is an unseeded default,
as a paste onto a never-opened raw leaves, resets to that blend, not
the ISO's; the Original row does the same.
