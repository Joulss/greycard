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
as the window changes. A drag never pushes a list past what it has
to show, so it moves room only from a list grown past its floor to
one cut short; on a window where every list fits, no gap has
anything to move, and the gap shows its hand only where a drag can
move room. This is the automatic rule kept whole, chosen over a drag
that wins (the two lists held where the pointer leaves them, blank
room under a short one, double-click back to automatic). The two
doors store the same weights, so the other costs one rule in the
layout if the live gaps turn out too rare to be worth a handle; it
is on the roadmap. A double-click on a gap evens the two. The weights are
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

The hand is honest: a gap shows the resizing cursor only where one
of its two lists has room to grow and the other has grown past its
floor, as the lists last said. The gap asks at each pointer move over
it with no button held, at a press, and at a release, so a pointer
resting in a gap while its lists change under it (an undo, a preset
added by shortcut, a resize) sees the hand turn at its next move.
Slint picks the cursor up before the move's event runs and the lists
answer on the loop's next pass, so the hand shows the answer a move
behind. We first made a drag wait for both lists' answers before it
started and dropped that on review: Slint runs pending change
handlers before every pointer event, so the order it defended against
does not occur, and it would only have hidden a drag if a list ever
failed to answer. A press on a dead gap still writes nothing.

## What was built

As decided, with these departures and particulars:

- **Weights are shares of the room past the floors.** Slint's box layout
  starts each item at its preferred height and hands out the rest by
  stretch, so a list's box prefers its floor (three rows, or its list
  when shorter) and its weight divides the room *above* the floors. The
  floors differ in pixels (FOLDERS about 66, COLLECTIONS 68, PRESETS and
  HISTORY 74, SNAPSHOTS 92), so equal weights leave the lists up to about
  26 px apart in height. The ceiling (the whole list) and the round-again
  are the layout's own (`adjust_items` loops until no one can grow).
- **A wrapper per sharing section** in app.slint, a Rectangle that
  forwards the section's min, preferred, and max, carries its weight as
  `vertical-stretch`, and holds the drag edge for the gap above it.
  `Section` gains `shares`: open and settled, its body may grow to the
  body layout's max instead of being pinned to its preferred height; the
  fold's animation is unchanged. So the sharing panels bound every other
  child of the body (their Texts get `max-height: self.preferred-height`),
  leaving the list's box the one that grows. The header is untouched.
- **A sharing section's fold runs from its share and opens to the share
  the pane has now.** The fold's `full` animates between 0 and the
  body's preferred height, which for a sharing section is its floor, so
  a grown list folded by jumping from its share to the floor and opened
  by jumping back. A sharing section instead animates a 0-to-1 fraction.
  Folding, the body is bounded by the fraction times `held`, its height
  when last open and settled, so the fold starts from the share it had.
  Opening, its min, preferred, and max are the fraction times the body's
  own, so at the end they are the settled bounds exactly and the layout
  hands over with no step, however the window changed while it was
  folded and whether or not it was ever open. A section made folded
  lands folded. `held` is kept by a change handler that exists only in a
  sharing section, so no other section lays itself out as it is made.
- **A behavior change while open:** a change in a sharing list's content
  (History from none to forty steps on a picture switch, a preset added)
  now resizes the section at once, where master animated it through
  `full`. Folding and opening still animate.
- **The pane's layout** is stretched (no longer `alignment: start`); the
  Navigator, the open-folder block, the import block, ROOTS, the Export
  queue row, and METADATA take a stretch of 0, and a foot Rectangle with a
  stretch of 0.0001 takes the room no list can, so nothing else ever
  grows. The bottom padding gives up the foot's gap, so nothing moved.
- **COLLECTIONS** had no box: its rows now sit in a ScrollPane box like
  the others' (no border, as before), padded clear of the scroll band.
- **The drag** sets the two weights so the layout reproduces the dragged
  heights: with `k` the room a weight of one comes to (read from whichever
  of the two is short of its ceiling), each weight is its growth past the
  floor over `k`, so the other lists keep theirs exactly. A drag is held to
  each list's floor and ceiling; a weight never goes below 0.01, so a list
  dragged to its floor can come back. A drag that moves nothing (both at
  their ceilings or floors, or the pointer back where it pressed, as in a
  double-click's jitter) leaves the weights as they were at the press, so
  nothing is written. A double-click sets both to their mean.
- **How a list lies is told only when a drag asks.** A `changed` handler
  on a wrapper's height laid the section out as it was made, and its fold
  then animated as its list first filled (five existing tests caught it).
  So the press raises a counter the wrappers watch, they report their
  growth and remaining room before the next event, and the first move
  reads them.
- **DragEdge** (controls.slint): draws nothing, the resize cursor on
  hover, `started`, `dragged(length)` (the pointer's travel since the
  press in window coordinates, so the gap moving under the pointer does
  not count), `ended`, `evened` (double-click); `along-y` false for an
  x-drag, for §605's pane edges.
- **Settings**: `shares`, a map from section name to weight, only the
  weights that are not one (`panel::shares::read`/`show`); a weight that
  is not a positive number reads as one. The file's map is read
  leniently: a weight that is no number (or null) reads as one, and a
  `shares` that is no map as none, so a hand edit cannot set the whole
  file aside. Written at close with the rest.

## Verified

- `panel::shares::tests` (headless window, Loupe, FOLDERS, PRESETS,
  SNAPSHOTS, and HISTORY open with 60 rows each, 1500 x 1600 against
  1500 x 500 for the floors): equal weights give equal growth; PRESETS at
  two grows twice the others and the total is the same; folding SNAPSHOTS
  and HISTORY gives FOLDERS and PRESETS more, alike; five presets keep
  their whole list and the spare goes to the other three alike; a 60 px
  drag on the PRESETS/SNAPSHOTS gap moves 60 px between them and leaves
  FOLDERS and HISTORY within 2 px, the new weights hold as the window
  changes, `remember` writes just the two, and a new window reads them
  back; a double-click on the SNAPSHOTS/HISTORY gap takes 3 and 1 to 2
  and 2; at 600 high every list is at its floor, HISTORY is past the
  foot, and the wheel brings it in; in the Library FOLDERS and
  COLLECTIONS share alike and their gap drags.
- The fold: PRESETS grown well past its floor on a 1600-high window,
  folded, is 10 ms in between its share and its floor plus 20 px (not
  at the floor), and the other three have grown by just what it gave;
  folded it is under its floor; opened, 10 ms in it is between folded
  and its share with the others giving just what it took, and settled
  every list is back at its height before the fold.
- Opening: PRESETS folded on a 900-high window, the window made 1600
  high, then opened, frame by frame through the fold's 120 ms: the last
  step is under 10 px and the end is the settled height within a pixel.
  PRESETS folded from the start (never open) on a 1600-high window,
  opened: the same, and it ends on the share an open PRESETS has there.
- Drags that move nothing: every list at its ceiling (five rows each) and
  a jiggle up and down leaves the weights unwritten and the heights as
  they were; a press with a 3 px jiggle back to the press point on long
  lists writes nothing. A drag down into PRESETS at its ceiling (five
  presets) moves and writes nothing, and up it gives SNAPSHOTS 20 px; a
  drag 2000 px up holds PRESETS at its floor, SNAPSHOTS takes all PRESETS
  had past it, and PRESETS's weight is 0.01.
- `settings::tests::the_shares_are_read_leniently`: a string or null
  weight reads as one, an array, null, or string `shares` reads as none,
  and the rest of the file is read either way.
- The whole of `cargo test -p greycard-ui` (1364 + 171 + 3 passed),
  `cargo fmt --all`, `cargo clippy --all-targets` with `-D warnings`,
  all clean.
- One capture, debug build under headless weston (pixman) on lavapipe at
  1500 x 1400, fit to 1500 x 800 by the window's screen fit: short lists
  (three presets, no snapshots) look as before.
- The honest hand: five rows everywhere on a 1600-high window makes
  the PRESETS/SNAPSHOTS gap dead, sixty rows live, and five presets
  over sixty snapshots live; a pointer resting in the FOLDERS/PRESETS
  gap with the presets cut from sixty to five under it keeps the old
  answer until a 1 px move, then is dead, and is live again after the
  presets come back and a move. With the ask raised only as the hover
  begins, that test fails at the 1 px move. The real event loop,
  scripted under headless weston, answered the press's ask before the
  first move and moved the room it had.

## Not verified

- A before/after capture on a tall window with long lists: the capture
  harness fits the window to 800 high, and a fresh config has three
  presets and no snapshots, so the sharing does not show. The tests
  measure it instead.
- The drag and the hover cursor by hand, and the feel of a drag on a real
  pointer (the tests send events directly).
- Folds animating while a gap is dragged, and a drag while the pane is
  scrolled (the travel is in window coordinates, so it should hold).
- Windows and macOS.
