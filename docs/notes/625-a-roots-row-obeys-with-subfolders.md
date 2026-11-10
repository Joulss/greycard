# §625. A root's row obeys With subfolders (2026-11-10)

**Reported** (discussion 59). A click on a root's row under ROOTS
listed every frame under the root whatever the switch said, while a
click on the same folder in the tree obeyed it. With the switch off the
pane contradicted itself: the switch read off, the grid listed every
subfolder, and the tree marked the root as if for its own frames.

**The rule.** A root's row opens the root exactly as its row in the
tree does: `tree::view_for(root, root, subfolders)`, the root's own
frames with the switch off (`View::Branch { root, folder: root, deep:
false }`), everything under it with it on (`View::Roots(Some(root))`).
The switch never moves for a click. `roots::picked_view` is the one
change in code; All roots (the empty path) is `Roots(None)` as before.

§209 set the root with the switch on as the root's own view and left
the chip, and later the ROOTS row, as "the everything-under-it one".
That half is what changes; the rest of §209 carries a `Branch` at the
root already, since the tree's root row opened one.

**The cases.**
- *Flipping the switch over a root's view* already re-listed it:
  `subfolders_changed` takes the root to `view_for` either way, merging
  in place when the frame on screen stays. Unchanged.
- *Back and forward* (`visited.rs`) keep a root as `Place::Folder` and
  open it with `view_for` as the switch stands. Unchanged; a root is now
  left the way it comes back, so it shows what it showed while the
  switch has not moved.
- *The start folder* (`startup::start_open`) already opens a root or a
  folder under one with `view_for`. Unchanged.
- *The launch's restored place* is the last file's folder, opened from
  the disk (`View::Folder`), never a root's view. Unchanged.
- *All roots* is every frame, whatever the switch. Unchanged.
- *An offline root*: a `Branch` at the root lists from the index,
  dimmed, as §209 opens any folder of an offline root's tree.
- *The watcher and the merge*: `kept_up` already reads a `Branch` again
  for a pass over it or above it, and with the switch off a pass under
  it is not read, since it cannot change the root's own rows.
- *The queue* returns to the view it was entered from, as it was.
  Unchanged.
- *Search and the filter* narrow whatever the view lists. Unchanged.
- *A root removed while open* goes to all roots from a `Branch` as from
  a root's view. Unchanged.
- *Recently opened* records a `Branch`, so a root opened with the switch
  off now goes to the front of the list, as its row in the tree already
  put it.
- *The sort*: a root with the switch off takes the folder's order (by
  name), as its row in the tree did; with it on, the order of all roots.
- *Rejects*: Move rejects, Delete rejects folder, and Remove rejects from
  an archive treat a root with the switch off as one folder, as they
  treat its tree row.

**Kept as they were.**
- *Import's Add in place* still opens the folder it adds deep (the
  root's own view, or for a folder under a root already that root's
  branch, deep), whatever the switch. §347 made it so because a tester
  asked to import a folder with its subfolders, and it is the only
  "root added just now" that opens anything (Add a root and Add this
  folder open nothing). With the switch off that view still shows the
  old mismatch until the switch or a row is touched; the switch turned on
  over it opens nothing, being the same view.
- *A smart collection* looks over its root or folder with everything
  under it (`collections::smart_view`, deep), as the guide says a saved
  filter covers "a root, a folder with the folders inside it, or every
  root". A smart collection is not a click on a root.
- *The tiles over the root's own view* (`tree::tiles_for`, §212) still
  follow the switch for a `Roots(Some)`, so the two views above that
  open deep with the switch off keep their tiles, as before.

**The count** beside a root's row stays the total under the root. A tree
row's count follows its fold, not the switch (§209): its own frames
unfolded, the total folded. A root's row has no fold, and reads as a
folded row does, so it matches the tree whatever the switch says.

**The empty case.** A root with no frames of its own, opened with the
switch off, opens empty, the tree marks it, and the status line says
"nothing directly in Archive: turn on With subfolders to see the frames
under it", the line §209 gave a folder of the tree. No new wording. A
hint in the empty grid itself, rather than the status line, could help
a first-time user who adds a root of dated folders and sees nothing;
not done.

**Tests.** On the headless window, a root's row clicked through its
callback (`a_roots_row_opens_the_root_as_the_switch_says`): with the
switch off the root's own frame, the row lit, its count the total, the
tree's root marked, the switch shown off, the day a tile; the switch
flipped on lists everything under it with no tiles, the row clicked then
keeps it so and the switch on; off again, and a click moves no switch;
the day opened from the tree and back comes to the root's own frame;
All roots lists every frame with the switch off; and a root with nothing
of its own opens empty and says why. One test pinned the old behavior:
`a_root_is_named_from_its_menu` clicks the row before the index is open
and checked the view it waits for was `Roots(Some)`; it now expects the
root's own frames, the switch being off. The tests that open
`Roots(Some)` call `open_view` with it directly, which is unchanged.

**Found in review, and left.** A smart collection saved from a root's
own view (switch off) covers the root's subfolders too, and its sheet
names only the root, where a folder's says "and the folders in it".
After Add in place with the switch off, a click on the lit row shrinks
the list to the root's own frames. And the empty root's status line
names the folder, not the label a renamed root shows. The first and
the last were true of the tree's root row before this; the ROOTS row
now reaches them.

**Not proven.** The real window: the tests drive the headless one.
