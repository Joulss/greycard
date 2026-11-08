# §609. A name cut short, whole at the pointer (2026-11-07)

Issue 88: long file and folder names are elided, and nothing showed
them whole. We now put the whole name on the window's one hover layer
(§311's `Tips`) after the same rest, in the four places the tester
named: the strip's names, the grid's names (and its folder tiles), the
open frame's name in the Develop panel's head, and the folder tree;
and, on the user's word, a fifth: the grid header's line naming the
chosen frame (its tip is the line whole, "and N others" included when
it says so). Only when the shown name is cut short (the Text's
`preferred-width` over its `width`, as the roots list already tests);
a name that fits says nothing. The tip is the name as the row shows it
whole: the file name with its extension, a version cell's version name
alone (as the grid's cell shows it), the folder's name and not its
path. No new wording.

The tip comes up only with the pointer resting over the name itself:
in the strip and the grid, the cell's picture says nothing, and a
folder tile's icon and count say nothing. The tree's rows and the
Develop head are the name, near enough, and take the whole row.

`TipArea` could not carry it: the cells already own TouchAreas with
press, menu, and drag logic, and the grid's name sits inside an `if`,
out of reach of an id. So we added `NameTip` to controls.slint, a
TouchArea-less piece that lies over the name's own Text and reads the
row's TouchArea: its hover, both over the row at all and over the name's
box (the owner's gate), its pointer place, the view's scroll and layout
number, and a count of its presses of any button (`pressed` is
left-only, and a right press asks for the frame menu). It writes through
`Tips` as `TipArea` does, so there is still one hover text, and both
take their rest from `Tips.rest`, so the two cannot drift.

What takes the name down:

- Leaving the name.
- A press, of any button, and it stays down until the pointer leaves
  the row: a menu or a reorder drag may follow, and a name back over
  either would fight it. Leaving the row, not the name, is what
  re-arms it, so a held press slid from a picture onto its name stays
  quiet.
- A scroll of the view the row is in (the strip's offset, the grid's,
  the folder tree's pane), which carries a cell from under a still
  pointer that never hears it as a leaving; the name stays down until
  the pointer moves and rests again. A scroll moves the cell under the
  pointer without making it again; only a whole-window jump does, and
  that comes from a key (which already clears the text) or the scroll
  bar (where the pointer is on no cell), so the window does not clear
  the text on a grid scroll. It did at first, which wiped a header
  button's word whenever the grid held its place as thumbnails came
  in. We first watched the Text's absolute position instead of the
  scroll, but a change handler on it left a click in the folder tree
  unheard (the tree's click test failed).
- A list replaced. A reset destroys every cell, and the one under the
  pointer never hears it leave, so its name would stay up until a key
  or another tip; files coming or going on disk, the queue view
  relisting, and counts landing during a scan all do it with no key
  pressed. The window clears the text when `thumbs` changes (a new
  model each time) and from Rust when the folder tree's rows or the
  grid's tiles are replaced in place (`hide-tip`).
- A re-flow of the grid or the strip (`grid-shape`, which moves on as
  thumbnails bring their shapes in, in either view), which moves cells
  without renaming them: each name's tip watches the number and goes
  down until the pointer moves and rests again. We first cleared the
  window's text on it instead, which wiped every other control's word
  whenever a thumbnail landed, and left a name's own tip believing it
  was still up.
  A scroll or re-flow marks a row only while the pointer is over it:
  a row moved with the pointer elsewhere is entered afresh, perhaps at
  the very spot it was left, which moves no pointer coordinate to
  clear the mark (the strip test caught this).
- The row given another name (the head on another frame).
- A frame menu up (`menu-shown()`), in the strip and the grid; a sheet,
  as before, by the layer itself.

The layer now also keeps the tip inside the window's height, so a
strip name at the window's foot has its tip over the name rather than
cut off below.

Not done: the other elided texts in sheets and lists, which the
issue did not name.

Verified:

- `name_tips` tests (crates/greycard-ui/src/name_tips.rs, six): for
  each place a long name shows whole after a rest, leaves with the
  pointer, goes on a press and stays gone while the pointer rests; a
  short name shows nothing where the place has one beside it. The strip
  also takes a right press, says nothing over the picture or for a press
  held on the picture and slid onto the name, takes the name down on a
  new list (`cells::set_rows`) under a resting pointer, and keeps Undo's
  word up through a re-flow, and up again on the next rest. A scrolled
  grid holding its place as wide pictures come in (`reshape`) leaves the
  Sort button's word up. The grid also says nothing over the picture or
  mid-drag from it, takes the name down on a re-flow (`note_aspect` then
  `take_aspects`) and shows it again after a move and a rest, and takes
  it down on a wheel scroll and shows nothing after.
- The new-list test fails with the window's clear on `thumbs` taken out
  (checked before the re-flow clear moved into the name's tip).
- `cargo test -p greycard-ui`, fmt, clippy `-D warnings`.

Not verified:

- How it looks on a real screen (placement over the strip's foot, the
  clamp), and on a native macOS or Windows frame menu, where
  `menu-up` may not track the menu.
- A window resize that makes a cut-short name fit does not take its
  tip down; it goes when the pointer leaves.
- Dragging the left pane's width under a still pointer over a name
  leaves its tip where it was until the pointer moves. A panel shown or
  hidden already takes it down.
