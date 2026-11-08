# §603. The left pane's sections share its height (2026-11-07)

Issue 79 (0.10.0): the left pane's lists, Folders, Presets, Snapshots,
and History, each sit in a box of their own height, Folders up to
320 px, Presets 200, History about the same, and the pane as a whole
scrolls. On a tall window the pane runs out of sections with room to
spare while Folders scrolls inside its small box; collapsing a
section frees nothing for its neighbors. The tester asks for the
sections to divide the pane's height, for a drag between them, and
for the division remembered.

## The decision

The open sections' lists share the room the pane has after its fixed
parts: the headers, the Navigator (which keeps the picture's shape),
and the roots list (which is short and does not grow). Each sharing
section has a weight; every list gets its floor of three rows first,
and the room past the floors is divided by weight, each list held to
a ceiling of what it has to show, and what a list gives back under
its ceiling goes round again to the others. The floors differ a
little in pixels (a snapshot's row is taller than a preset's), so
equal weights leave equal growth, not equal heights. So collapsing Snapshots and History hands their
room to Folders and Presets, a lone Presets takes it all, and a
Folders of six rows takes six rows' worth and no more. A list within
its share scrolls on its own, as each does today.

The pane scrolls as a whole only when the floors alone exceed the
room, a short window with every section open, which is what it does
today for every window.

The gap between two open sharing sections is the drag: moving it
takes room from the one and gives it to the other, and the two
weights are set from the heights that result, so the division holds
as the window changes. The gap shows its hand on hover and nothing
else is added. A double-click on a gap evens the two. The weights are
remembered in the settings file by section name, as the folded
sections are (`collapsed`), and a section never dragged weighs one.

Sharing: Folders, Collections, Presets, Snapshots, and History. The
Develop panel's sections hold controls, not lists, and keep their
heights.

## The mechanism

Slint lays a `VerticalLayout` out by its children's stretch and
their minimum and maximum heights, and a list box today pins its
own maximum. The sharing section's box takes `vertical-stretch` from
its weight, a minimum of three rows or its content when shorter, and
a maximum of its content; the layout's own distribution then does
the sharing above, and the pane's `Flickable` falls back to a scroll
when the minimums do not fit. The drag writes two weights; nothing
in Rust lays anything out. The weights cross to Rust only to be
remembered and read back (`settings.rs`), by the section's name.

## Verified
(To be filled from the build.)
