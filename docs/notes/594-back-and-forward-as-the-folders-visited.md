# §594. Back and forward as the folders visited (2026-11-07)

**The side buttons go where the grid has been.** §249 gave a mouse's
back and forward buttons the frame step of ← and →. Over a grid of
folders that is the wrong reading: having opened a subfolder from its
tile, a user presses back to return to the folder, as a browser goes
back a page. We give the buttons that meaning and leave the frame step
to the arrows. The grid's `side-press` is unchanged; it now asks for
`visit-folder(back)` in place of `grid-step`.

**One place records it.** Every list the browser takes lands through
`recent::opened`, so the history (`panel::visited`) is fed from there
and never told by an opener: a tile, a tree row, Recently opened, Open
folder, a root, and a collection all count, and a way to open a list
added later does too.

**A folder is one place however it is listed.** A place is a folder,
all the roots, or a collection. A folder opened from the disk, the
same folder from the tree, and a root's own view (which is what the
tree's root row becomes with the subfolders switch on) are all the
folder: how it is listed is the switch's and the roots', not the
place's. So flipping the switch makes no history, and going back to a
folder under a root opens it as its tree row would, the switch as it
stands; one under no root opens as Open folder does. So a root's view
gone back to after the root was removed comes back as a plain folder
from the disk, and a folder opened from the disk that has since come
under a root comes back as its tree row. A folder reached through a
link is kept under Recently opened's spelling of it, the tree's under
the index's, so the two are taken for two places; it is rare, and we
leave it. The export queue
and files from several folders are no place: landing on one changes
nothing, so leaving the queue lands on the place still here.

**The stacks move only when a place lands.** A new place pushes the
one before it and clears forward; a list read again is the same place
and moves nothing. A press only asks: it notes the place, how far down
its stack it is, and the places nearer it found gone, and when that
place lands the stack gives up it and every place passed over to the
other stack, nearest last, as a browser's back of several steps does.
Moving the stacks on the press instead records a place never shown:
a tile clicked before the place's read lands supersedes it, and a
folder emptied meanwhile opens to nothing. In both cases the history
keeps what was shown. A second press the same way before the first
lands goes on past the place asked for, so an empty folder never
traps the button; should the first's place land after the second has
asked, the stacks move for it and the second's ask stays, nearer by
the places taken. A second press that finds nothing further leaves
the first's ask alone, dropping only the places it found gone beyond
it. Any open by hand (a tile, a tree row, Recently opened, a root, a
collection) lets go of an ask, so only a landing of what a press
asked for moves the stacks its way.

**A place gone is dropped; one not answering is kept.** The folders a
press might go to, up to sixteen, are looked at all at once off the
window's thread, each its own look with the roots' wait, and settled
nearest first as the answers come: a press waits no longer than the
nearest folder there takes to answer, and one look at most however
many sit on a sleeping share. The walk stops at its sixteenth look
rather than passing places it has not looked at; we did not use
`answers_of`, which waits every path to its deadline and takes those
past a silent batch for silent without a look. A folder not there is
dropped, with a line in the log; one not answering is passed over and
kept, to go to once its share is back. A folder under a root already
known offline is listed from the index and needs no look; a deleted
collection is known without the disk. A press while a look is out is
dropped, and a look that comes back after another list opened, or
after a place landed and moved its stack, is dropped too, the log
saying which.

**Develop is unchanged.** The loupe never took the side buttons (§249
left them out of the viewport's press on purpose), and the ask is
about the grid after a subfolder; the lightbox is the grid, and goes
back and forward too. No key was bound to back or up, so none is kept
or added, and nothing new is said on screen.

Tests: a subfolder opened from its tile, the real back button over a
cell lands on the folder, the forward button over the other tile on
the subfolder, and a new tile from the folder clears forward; the
lightbox's side buttons ask for back and forward and step no frame;
a folder deleted is dropped and one not answering kept; a folder is
one place as a tree folder, a root's view, and a disk folder; a
superseded ask and a failed one leave the history as shown, the next
press going past; a second press finding nothing keeps the first's
ask; a first press landing late keeps the second's; an open by hand
lets go of an ask; places gone are dropped from the farthest; the
walk stops at the first place there and at its reach; several steps back at once; the queue changes
nothing; a look after another open is dropped; the side buttons over
a cell and over the sheet ask for back and forward.
