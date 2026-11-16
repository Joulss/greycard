# §695. ISO and focal length as ranges (2026-11-15)

The filter showed ISO and focal length as a chip a value. A card shot
across a day has a dozen ISOs and, on a zoom, a dozen focal lengths,
and the question asked of them is a span: "800 and up", "the long end".
Each is now one row: its caption, the low end's value, a two-handled
slider, the high end's value. The CULLING section has the two under its
stars, flags, and labels; the grid's header has them side by side on a
row of their own under its top row.

**The stops are the folder's values, evenly spaced.** A stop a value,
by index, not by magnitude: ISO 100 to 102400 on a linear track would
put everything up to 6400 in its first sixteenth. The values are the
whole folder's, not narrowed by the rest of the filter as a chip row
is: the stops stay put while the stars or flags change, and the range's
own ends never take values off its track. A frame behind a closed stack
is left out, as the chips' counts leave it. The two `GROUP BY`s are
kept against a hash of the index ids they were read for, so a keystroke
or a drag stop reads nothing; another folder, a frame indexed, or a
stack opened or closed reads them again.

**The range is the text.** It reads and writes the `iso>=`, `iso<=`,
`focal>=`, and `focal<=` terms the field already parses: a range set is
a filter typed, a filter typed or kept shows on the handles, and the
field shows what the range asks. An end at the folder's own end writes
no term, so the whole range asks nothing and reads as off; the fill
between the handles and the numbers' weight say on, as a chip's do. A
frame with no ISO (a scan, a DNG without the tag) passes only while the
range is whole, since a term on a field the frame lacks fails.

- **A bound on a value the folder lacks** (`iso>=1000` over 800 and
  1600) puts its end on the nearest value it lets through (1600), so the
  handles span exactly the frames that pass. The text is left as typed
  until that end is moved. Bounds that pass no value put both ends on
  the value nearest the one asked: an exact one, else the tightest low,
  else the tightest high, whatever their order in the text.
- **An exact `iso:800`** bounds both ends. Moving either takes it out
  and writes both ends as shown.
- **Moving one end** splices out that end's terms only, each with the
  gap before it; the other end's terms and every other byte stay as
  typed. New terms go at the end, after a closing quote when the text
  ends inside an open one (`"harbor` would otherwise take the term into
  the phrase and match nothing).

**No chips for either, anywhere.** Two controls for one question is
clutter, so the chips go, in the CULLING section and the header alike,
and with them their two `GROUP BY`s a filter change. The range is the
one control, and the text is the one state:

- **An active range** shows as its terms in the filter's words, with
  the count and Clear on, as a typed term always has, and in the fill
  and the numbers' weight. Clear clears it with the rest; the
  double-click clears it alone.
- **`--filter iso:800`** at launch goes to the text, under the field's
  own name (`mm:50` as `focal:50`), not to a chip waiting for the index.
  Only a term is renamed: a bare `mm` is a word and stays one.
- **A filter kept by a build with the chips** (the settings' last
  filter, a smart collection) loads as the range's terms appended to its
  text: one value as `iso:800`, more as `iso>=lo iso<=hi`. The range is
  a span, so chips 200 and 3200 load as 200 to 3200 and take the values
  between them too; we judged that closer to what was asked than
  dropping the filter, and it is in plain view in the field. A value
  that is not a number is dropped. Saved again, the terms are kept in
  the text and no chip is.
- **A value held out by such a build is dropped**, and the log says
  which, once, when the filter is put back on the bar (the last
  session's at launch, a smart collection opened). Reading a kept filter
  is quiet, since every rebuild reads every smart collection's to light
  its row. A held-out chip let a frame with no ISO through; `iso!=v`
  would hide it. A filter that widens in plain view is better than one
  that hides frames no one asked to hide.
- **An older smart collection lights when opened.** Its row is lit when
  the loaded filters are equal, not the saved shapes: its file still
  holds the chips, which the bar's state never will. Opening it does
  not rewrite the file; saving it again does.

A `!=` typed is not shown on the range, since a span has no gap in it;
the field says it.

**The control.** A new `RangeSlider` in `controls.slint`, drawn as
`EditSlider` is. A handle only drags: a press on one holds it, and a
press off the handles moves nothing. When the press is on both handles
(one stop, or stops closer than a handle), the drag's way picks one,
and that first move never takes it against the way: pressed left of
both and moved a little right, the high end could otherwise land on a
stop left of where it was. A
drag never carries one end past the other. A double-click anywhere on
the track asks for the whole range. A press takes no focus, as the
chips take none, since in culling the arrows are the frames'. Each
handle takes the keys by Tab, an arrow a stop, Escape back to the
window, any other key the window's.

**The filter applies when a move is done.** In culling, every filter
change is a listing, a grid rebuilt, a frame chosen again when the open
one is hidden, and the set pruned of what the filter hides. A scrub
past the open frame's ISO and back would leave another frame open and
the set cut, though the filter ended where it began. So a move only
holds the end, shown on the track and beside it; the text, the grid,
the open frame, and the set are untouched until it is done, and a scrub
back to where it began asks nothing. A drag is done on release; an
arrow when it is let go or the keys go elsewhere, so a held arrow,
which is a scrub too, asks once. The log says each ask, as it says a
reset.

**What a hold survives, and what ends it.**

- **A quick click after a drag is no double-click.** Slint counts a
  press within its double-click time and 10 px of the one before as a
  double-click's second, whatever the first did; on close stops a drag
  of one stop and a click beside it would reset the range. A
  double-click resets only when neither press moved an end, and an end
  moved is any change of its stop, not a distance.
- **Another button pressed mid-drag ends the drag**, asking nothing.
  Its release lets go of the pointer, so the left release can land
  where the slider never hears it; a hold left behind would be applied
  by the next click. Every left press drops any pointer hold first.
- **New stops under a hold end it** when the held stop's value changed,
  asking nothing: the stops are read again as the index gets further
  or a stack opens, and an index into the old values would name another
  value, or none. Release clamps the stop as well.

**Nothing moves the ranges under the pointer.**

- **In the CULLING section** they sit above the Camera, Lens, and Day
  rows, under the stars, flags, and labels: the facet rows narrow and drop with the
  filter a range sets, and under them a range moved after each release.
- **In the header** they have a row of their own under the top row and
  above the facets' rows, for the same reason. Each takes 420 px, a
  track easy to set at, down to 220 px at a narrow window, side by side
  so a laptop's header holds both; a wide window does not draw them
  long. A header narrower than the two at their least (464 px: the
  window at its least, 574 px, with the left pane open, leaves 334)
  puts one under the other, the header's width each. A facet with
  fewer than two values has none there.

**The panel's head keeps the shot time's line.** The head, above the
sections in every tab, drew the shot time's line only when the frame
had a time. A range release that hid the open frame could open one
without a time (a scan, a DNG without the tag) and move every section
up 14 px, the CULLING section and the range under the pointer with
it. The line now keeps its 14 px always, as the camera and exposure
lines already did, so the head is the same height for every frame. It
is 14 px taller than before only on a frame with no shot time and with
no frame open, in Develop as well as Cull; we took that over a panel
that jumps between frames.

The companion JPEG's line and the Sync button still come and go, and
can move the panel when a release changes the open frame or the set:
the line only between a frame with a companion and one without, the
button only when a set of two or more is cut to one. The test folder
of 34 frames has no companions, and its set is the open frame unless
one is made, so neither moved in it. We do not reserve their room: an
empty line or a missing button's gap would be there for every frame.

**For a screen reader**, both handles say the same kind of thing: the
stop as the value, among the stops that handle can reach as its minimum
and maximum, and the folder's value there as the description; increment
and decrement step it and apply. No new strings: the values are the
folder's.

**Brought into view.** Shift+Tab to a handle half cut by the panel's
edge scrolls it in: the panel's view does so for a focused item of
itself, as far as the handle and not the whole row around it. A reveal
like `EditSlider`'s changed nothing the test could see, so the handle
has none.

**One value or none.** No values (no index rows yet, or no EXIF) hides
the row. In the CULLING section one value shows the caption and the
value, no track: what the lone chip said, without a control that cannot
move. The header, whose rows are controls, leaves it out.

**Rejected.**

- A `for` over a model of ranges: a model put again rebuilds its rows,
  and a drag under way with them. Two fixed properties, `if` each.
- Ranges in `Facet::ALL`'s place, between Style and Day: it cost a third
  property, and the facet rows above would move the ranges as the
  filter changed. The two ranges sit together, their tracks lined up.
- A magnitude or log track: the stops would bunch where the values do.
- Stops narrowed by the rest of the filter: the track would change under
  the pointer as the filter it sets changes.
- Rebuilding the text from its tokens: it closed up runs of spaces.
- Applying at every drag stop or every arrow, as typing applies a
  character at a time: a scrub moved the open frame and cut the set.
- A held-out value as `iso!=v`: it hides the frames without an ISO the
  chip let through.
- A click on the track jumping the nearer handle there. To let a
  double-click reset without the click's jump applied first, the click
  had to wait out the double-click time, and the wait was the source of
  four defects in review: a click landing after Clear, a stop index gone
  stale under it, a click then a drag read as a reset, and a key that
  discarded it. A handle that only drags has none of them, and the keys
  and the double-click cover what the jump did.
- Dropping every hold when the stops' model is put again: show_filter
  puts a new model at every rebuild, an index pass's ticks among them,
  so a drag during indexing would never land.

**Open.**

- A zoom with a hundred focal lengths at a tenth of a millimeter puts
  its stops under a pixel apart on a 280 px panel; the keys still step
  one value.
- Slint's focus chain passes over a handle clipped out of view whole.
- `mm` names the focal length to `Facet::from_field` but is not in the
  field's `FIELDS`, so `mm<=35` typed is a word, not a term. The range
  writes `focal`; the gap is the parser's, older than this.
- On X11 an auto-repeating key may arrive as press and release pairs;
  there a held arrow asks at each repeat, as it did before this round.

**Known limits.**

- With a handle focused, Home, End, PageUp, and PageDown are not the
  range's: they go to the window, as any key but the arrows, Tab, and
  Escape does, and change the frame.
- Escape during a held arrow hands the keys back, and the focus leaving
  applies the hold, as letting go of the arrow would.
- A screen reader hears the stop number as the value and the ISO or
  focal length in the description, and both handles of a range share
  one label: the value range's minimum and maximum are what tell the
  low from the high.
- The grid's header grows by its ranges' row, 32 px, when a facet
  reaches two values during indexing, as it grows by a facet row then.
