# §461. A sheet sized from its wrapped notes

*2026-10-31.* The Import sheet (0.9.0) lost its last line, and in a
window too short for it nothing scrolled. The sheet takes its height
from its body's `preferred-height`, and so did the pane's content.
Read as a property, a layout's preferred height is measured with no
width to wrap at, so each note that wraps to two lines on screen
counts as one; the body comes up a line short per wrapped note, the
pane holds the rows to that height, and the rows past it are clipped
with nothing to scroll. A wrapped `Text` given `height:
self.preferred-height` measures at its own width, and a body of such
rows adds up to what is drawn. The pane's content height is the
body's, so a short window scrolls the rows under the buttons.

The Lightroom, RawTherapee, Export, and Settings sheets hold
wrapped text in the same kind of pane; their notes are short enough
not to wrap at the sheet's width today, and take the same line if
one does.
