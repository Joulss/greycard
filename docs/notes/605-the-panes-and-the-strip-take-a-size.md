# §605. The panes and the strip take a size (2026-11-07)

The left pane is 240 px, the Develop panel 320, and the strip's row
the height of a 91 px thumbnail; each can be hidden (F7, F8, F6, Tab)
and none can be sized. §603 gives the left pane's sections a drag
between them and a place in the settings file for what it set; this
section carries the same drag to the panes' edges and the strip's.

## The decision

A drag on a pane's inner edge, the left pane's right edge and the
Develop panel's left edge, sets its width; a drag on the strip's top
edge sets its height, which is its thumbnail size. Each holds to a
floor and a ceiling: the left pane from 200 px and the Develop panel
from 280 px, each up to a third of the window's width; the strip's
pictures from 60 px, so the names under them stay readable, up to a
third of the window's height. The picture keeps its place at every
size. A double-click on an edge resets its default. The edges show
their hand on hover and add nothing else. The sizes are remembered in
the settings file beside the hidden flags, and hiding works as it
does; the lightbox keeps hiding both panes.

The strip's pictures grow with it: the strip asks the thumbnail pool
for the long edge its row needs, as the grid asks for its cells, so a
tall strip is made at its size and not scaled up from 91 px. Fewer
frames fit across; the strip's scroll and reveal rules are unchanged,
since they work from the cell's size already.

## What it costs

The Develop panel's controls were laid out for 320 px. The build
looks at every section at 280 and at 480: the sliders' labels and
readouts, the curve's square, the grading wheels, the scopes at the
top, the Masks tab's rows, and the sheets that open from the panel.
A section that is right only at one width is a finding of the build,
fixed in the section and not by a narrower ceiling.

## The mechanism

The drag handle is §603's, used at three edges; the sizes are
properties the window lays out from in place of today's constants
(`left-w`, `right-w`, `strip-photo`), and cross to Rust to be
remembered and read back (`settings.rs`) and, for the strip, to set
the pool's long edge (`browser.rs` puts `strip::PHOTO` today).

## Verified
(To be filled from the build.)
