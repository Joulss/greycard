# §689. Sections keep the order they are dragged into (2026-11-15)

From an outside contributor's pull request (156), finished here.

## What it is

A section's header dragged up or down moves the section. That works in
every view that has more than one section: the right pane's Cull,
Develop and Masks, and Crop (Retouch has one), and the left pane's
Library (roots, folders, collections, metadata) and Loupe (folders,
presets, snapshots, history). The order is display only. The develop
runs its ops in its own order, and the edit, the sidecars, the presets,
and the exports never see this one (a test holds that).

The drag: a press turns into a drag once the pointer has gone 8 px up
or down. That is past a click's wobble, and GTK's own drag threshold is
8 px. Only vertical travel counts, since sideways moves nothing in a
stack. Nothing moves until the release; a 1 px accent line shows where
the section would land. A release back over the header that moves
nothing is still a click and folds, so a wobble past 8 px folds as
before. A drop just under itself on screen moves nothing, even with
hidden slots between, such as FOLDERS under ROOTS with no tree, or a
Develop-only section under one on the Masks tab: the stack knows the
first slot on show under the one in hand and passes it with the move,
and the Rust side refuses a drop there too. Esc while dragging puts the section back, and the release that
follows does not fold. Off the pane sideways a drop is nowhere. Near
the pane's top or foot, or past them, the pane scrolls 12 px every
40 ms. Any change of what the panes show mid-drag (a tab key, G,
culling, the folder tree, a pane put away by F8, F7, or Tab, the
lightbox) cancels the drag as Esc does: a stack or a
section taken away hears no release, one that stays commits nothing at
it, and the next Esc belongs to the window again. Anything else that
takes the header in hand away (a mask deleted from under MASK) is
caught at the release: the window's event hook (`side_buttons`) sees
every left release before Slint routes it, and ends any drag no header
will end. It only clears the flag that lets Esc cancel; a header still
in hand hears the release next and drops as ever, so the hook can never
take a drop. A header still in
hand lets go at the cancel, without waiting for a release that may
never come to it: its cursor is the pointer's again, and the stack's
drag is over. Another mouse button
pressed mid-drag cancels it too: the header loses the pointer's grab at
that button's release, so the left release might never reach it. The
drag starts only past the threshold, so the
header's switch, its action, double-click, and One section open at a
time are untouched: their own areas take their presses first.

## Storage and reconciling

`panel::order` names each view's sections and holds their first
order; Slint lays out whatever list it is given. The settings key
`section_orders` maps a view name to its list of names, and holds a
view only while its order differs from the first, and the key is left
out of the file while no view has moved, so a user who never drags
never sees it. A view left alone
therefore follows whatever first order a later version ships. For one
that was dragged, `reconcile` keeps the saved names it still knows,
once each. A name the file lacks, which is a section new since the
save, goes after its nearest neighbor above it in the first order that
is present, or first when none is. A section added after Detail lands
after Detail wherever the user put Detail, not at the foot.

The key is read leniently, like its neighbors. A view whose value is no
list of names shows its first order, and a value that is no map is
none. The rest of the file stands: `{"section_orders":{"develop":"look"}}`
no longer sets the whole file aside. An older build ignores the key
(serde skips unknown fields) but drops it at its next write. A file
without the key shows the first orders.

A drop moves the fewest rows of the pane's VecModel: those not in the
longest run that keeps its order before and after (`kept_rows`). Rows
are taken out and put back, so the repeater remakes only those, and
the others keep their state. Where moving the dragged section alone
would do, it is the one moved, never a neighbor it crossed: when
COLLECTIONS crosses FOLDERS, FOLDERS could as well be said to have
crossed COLLECTIONS. Down or up, one drop remakes one section; only
FOLDERS dragged in the Loupe can need more, since both views'
sections are ordered around it.

Library and the Loupe share the left pane's one model, as Develop and
Masks share theirs, but each keeps its own order. The model is both
views' sections with FOLDERS, theirs both, once: what each has above
FOLDERS, FOLDERS, then what each has under it (`pane_order`). Either
view's order is the model with the other's sections left out, and each
slot's `if` shows only the view's own. FOLDERS is then one instance
across G, Enter, and Esc, its tree scrolled where it was, as on master.
Swapping two models on the switch, as the first round did, remade the
whole pane at every switch. The left pane's helpers look back six
places and count seven, the model's length, which a test holds. Reset section order (Settings, Interface, next to One section
open at a time) replaces the models wholesale. It can be pressed only
while some order differs.

## Develop and Masks share one order

There is one list, `develop`: MASK first (the Masks tab's alone), then
Develop's sections in develop order. One stack serves both tabs. The
six sections a mask edits too (Light, Color, Curves, Mixer, Grading,
and Tint) are one instance there, as on master, so a tab switch keeps
them and their state; a test checks that exactly those six survive the
switch. A mask's own white balance and Detail are separate instances,
as on master, but take Develop's places. A slot whose section the tab
on show does not have holds nothing and takes no room, so each tab sees
its own sections in the shared order. A drop on the Masks tab lands
before the next section the Masks tab shows, so a hidden Develop-only
section between them keeps its place relative to the others.

New mask used to scroll the panel to its top, because MASK was first.
It now scrolls MASK's top to the view's top once it is laid out,
measured from the sections' own top, since the pinned mask list grows
by a row as this runs. If MASK has been dragged to the foot, the name
field taking the keys is still on screen. At the most masks none is
made, and nothing is left waiting to scroll later. Nor where the Masks
tab is not on show (a frozen frame shows none of the develop tabs): the
reveal is for the mask just made, whose name takes the keys now, never
for MASK when it next shows.

## Hidden sections and the export queue row

Slint layouts lay out every child, shown or not: the compiler has no
visibility handling in its layout lowering. So a stack has no spacing
of its own. Each slot that holds a section on show keeps its own gap
above it, and the gap is left out above the first one on show. The
left pane works out the list on show above a slot (`left-before`) for
that gap, and for the share edge that moves room between the two
lists. The edge used to be wired to fixed pairs; it now follows
whatever list is above. With no list on show at all (culling with no
tree) there is no stack, so no gap is left for an empty one.

The Export queue row is no section and has no place in the order. It
lives in the collections' slot, under the list, so it moves with the
collections and shows whenever the grid does, as on master. The pull
request had it after the third slot, which hid it whenever that slot
was the folders with no tree.

## Measured

The left pane's header tops (logical px, a 1500x1000 test window), on
master, on the pull request, and here:

| view | master | PR | here |
|---|---|---|---|
| Library, no tree: COLLECTIONS | 207 | 215 | 207 |
| Library, no tree: Export queue | 285 | 293 | 285 |
| Library, no tree: METADATA | 321 | 329 | 321 |
| Loupe, no tree: PRESETS | 140 | 148 | 140 |
| Library [roots, collections, folders, metadata], no tree: Export queue | n/a | missing | 285 |

With the tree shown, and in culling, all three agree. So the hidden
FOLDERS cost 8 px (one gap) on the PR, and nothing here. The queue row
went missing on the PR whenever the third slot was the hidden folders.

A drop (slot 6 moved to 1 of Develop's 22) leaves 21 of the 22 slots
the same instances; the old wholesale model replacement remakes all
22, and so does every other drop, up and down, from each place to
each: a test makes all 22 x 23 drops. Every drop in Library's and the
Loupe's view keeps the most rows a move can, and FOLDERS, unless it
is the one dragged, keeps its instance and its scroll. Across Library
and the Loupe, into culling and out, FOLDERS is the
same instance and its tree's scroll is kept; with the tree hidden and
shown again, the other lists stay the same instances. A switch from Develop to Masks keeps exactly Light, Color, Curves,
Mixer, Grading, and Tint, as master does. Tab order follows the tree
order, and the tree order follows the screen order after a move.

## Left out

The roadmap item's default folds, and the scopes folded on a small
screen, are not part of this.
