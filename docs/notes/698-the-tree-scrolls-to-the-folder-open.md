# §698. The tree scrolls to the folder open (2026-11-15)

A folder opened other than by a click on its own row (Recently opened,
Open folder..., the command line, a root row or All roots, back and
forward, a collection that opens a place, Import's add in place, a tile
or the grid's up and into, the last folder at startup) was marked in
the FOLDERS list but could sit outside its scrolled view. Master
already unfolds the folders above the one marked anew (`tree::reveal`);
the row existed, out of sight.

## What

Every path ends in `tree::show`, which builds the rows with the open
one marked. It now passes the open row's number to the pane and, when
the open folder differs from the one last brought in and its row
exists, bumps a count (`folder-tree-reveal`). A folder whose tree has
not landed yet is brought in when it lands. A click on a row records
its folder, and that folder's marking bumps nothing: the row is under
the pointer.

`FoldersSection` answers the count on a one-shot 1 ms timer, after the
rows have their heights (§693's lesson), and also once when the list is
built. A row wholly in view moves nothing; otherwise the least scroll
brings it in, with one row to spare where the view has room, clamped to
the list's reach. Nothing else sets the scroll, so folds, a drag of the
section's height, a resize, and the wheel after are left alone.

## Limits

- The row's place is computed from its index, the fixed row height, and
  the list's padding and spacing; rows of another height would need it
  read from the layout.
- A list built anew (the tree shown again, its first rows landed)
  starts at its top, as before, and then brings the open row in.
- The review ran the other paths too (Open folder, a root row, a grid
  tile, a collection and back, startup before the tree lands, the pane
  hidden or FOLDERS folded meanwhile): each brings the row in. Only
  Recently opened, back, forward, a row in view, and a click on a row
  the list's foot cuts are in the suite.
- The folder already open, chosen again from Recently opened after the
  list was scrolled away, is not scrolled to: the same folder shown
  again does not move the list.
- A click on the open folder's own row leaves it recorded as clicked;
  any other folder opened clears it.
