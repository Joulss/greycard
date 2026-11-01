# §475. Four small asks from the testers

*2026-10-31*

Four tester asks, each small, taken together.

**A new mask's name takes the keys** (discussion 51). The New mask
button opens the MASK section and hands the name field the focus
with its placeholder selected, so typing names it at once; Enter,
Escape, or a click elsewhere gives the keys back. The focus is taken
by a one-millisecond timer rather than in the click, since the
target change's close of every field would otherwise hand the keys
straight back; the timer exists only once the rows do and runs after
that handler. M and Shift+M stay as they were: their shape waits for
a drag, so the keys stay the window's.

**A section opened out of sight comes into view** (discussion 59).
Under One section open at a time, a section opened by its header is
followed for 400 milliseconds while the folds animate, and the
panel's pane scrolls it whole into view, or its header to the top.
A wheel scroll during the follow lets go of it. A slider focused
elsewhere lets go of the keys at such an opening, since its section
folds.

**A pan with a tool in hand** (reddit). With the heal brush, the
fill, or a mask's shape in hand, a drag with Space or Shift held
moves the picture; the cursor is the hand and the tool's circle is
hidden; the tool stays in hand. Space's zoom then happens on
release, and only for a tap. The hand is armed only while the window
holds the keys: a key a focused slider passes on never arms it, so a
Space passed on still zooms on the press, which a review caught the
first cut losing.

**Ctrl+E exports again, with no sheet** (pixls). Once an export from
the sheet has set its choices, Ctrl+E sends the selection the same
way the sheet's Export button does, into the last folder; before
that, it opens the sheet as Ctrl+Shift+E does. The sheet's choices
are what it kept already; a flag in the settings says an export has
set them. The Keys sheet lists it as "Export with the last settings".
