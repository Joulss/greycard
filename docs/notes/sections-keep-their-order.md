# Sections keep their order (2026-11-15)

The `Section` lists can now be arranged by dragging a header: Library
and Loupe in the left pane, and Cull, Masks, Develop, and Crop in the
right. Retouch uses the same stack but has only one section to move.
The order belongs to each view; moving a section does not change the
order in which an edit is processed.

`SectionStack` holds the drag and its destination, and `SectionSlot`
wraps each section without moving it while the pointer is held. A
movement beyond 6 px starts the drag; a click still folds the section. The
destination is a 1 px blue line on the existing border or rule where
there is one, and is shown above the first section and below the last.
It takes no layout room, so the sections do not jump as the destination
changes.
Near the scroll view's ends, the drag scrolls the list. A tall open
body does not put its insertion threshold beyond the visible header.

Each view's order is saved in `settings.json` by stable section name
and restored at startup. A saved order keeps known names once and
appends sections introduced later in their default order. Sections
temporarily hidden by the current view keep their place. The left
pane's list weights remain attached to their lists, and the resize
gaps follow the lists that become neighbors after a move. The export
queue is a row, not a `Section`, and stays outside the order.

## Tests

`panel/order.rs` covers restoring names, dragging a Develop section
to the first slot without changing its fold, keeping the views'
orders independent, and moving a Library list without changing its
height weight. A drag into the last slot has no dedicated test yet.
