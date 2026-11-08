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

## What was built

As decided, with these particulars and departures:

- **The ceilings never go under the defaults.** A third of a window
  1000 wide is 333, short of the panel's 320 only past 960; a third of
  800 high, less the strip's pad, name line, and foot, is under the
  91 px pictures. Held to a bare third, a narrow or short window would
  have shrunk the panes below today's layout. So each ceiling is the
  larger of a third and the default: a narrow window lays out as before,
  and a wider one lets the pane grow. The strip's ceiling is the whole
  strip a third of the window's height.
- **The window's sizes are asked, the layout's are held.** `left-size`,
  `right-size`, and `strip-size` are what a drag (or the settings) asked
  for; `left-w`, `right-w`, and `strip-photo` are those held to the floor
  and ceiling. A window made smaller holds them down; grown back, they
  return to what was asked.
- **The ceilings follow the window by `changed` handlers, not a binding.**
  The window's least size is laid out from the panes', so a binding on
  the window's width looped through the layout (Slint warns, and calls
  it a future panic). The ceilings are set at `init` and as the width,
  the height, or the strip's foot change.
- **The edges sit inside their panes, 4 px along the line**, not across
  it: across, the viewport's outer 3 px went to the edge, and a drag from
  the canvas at the viewport's corner resized the pane
  (`only_a_drag_that_starts_on_the_picture_pans_it` caught it). The left
  pane's scroll band keeps its thumb, 4 px in from its outer edge. The
  edges are declared after the left pane and before the arrows, so an
  arrow keeps its whole face. The left edge overlaps the outer 4 px of
  the left pane's scroll band, by choice: there a press resizes rather
  than pages, and a hover does not light the band, but those pixels only
  widened the band's press area; its thumb stands 4 px in and keeps all
  of its own. Moving the band in instead put the lists 4 px short of the
  buttons above and below them, out of line with the Library and Loupe
  row and the Settings, Help, and Report rows. The Develop panel's and the strip's edges
  exist only in the Loupe; the left pane's over the grid too.
- **The strip's arithmetic takes the row's height** (`strip::photo_width`,
  `cell_width`, `Layout::new`), read from the window's `strip-photo`. The
  layout's cache is keyed by the shapes' number and the height. When
  the height changes, `strip-sized` lays the strip out at the old height
  and the new one, moves the layout's number on so the cells ask again,
  and holds the strip still about the selected frame with the same
  `strip::held` the shapes use. The height held against is noted at the
  strip's first layout; in the first build it was noted only by a
  resize, so the first move of the first drag had nothing to hold
  against and threw a far selected frame off screen (the review saw it
  jump 261 px in the real window, then drift as `held` kept the first
  frame on screen instead).
- **The pool's long edge** is `strip::render_size`: a landscape's width
  at the row's height (`photo / UNKNOWN_ASPECT`), never under the default
  strip's 170, capped by the grid's `MAX_RENDER` (360). It is set in
  `strip-range` as the grid sets its own in `grid-range`, both ways, and
  the frames on screen made a quarter too small are asked for again,
  once. A picture landing late at the old size is asked for again on
  the strip as on the grid (`deliver.rs`, by the view that is open), and
  only when not yet asked at the size wanted. A behavior change: the Loupe after a zoomed grid used to keep the
  grid's long edge for new strip pictures; it now asks for the strip's.
- **Settings**: `left_width`, `right_width`, and `strip_photo` (the
  pictures' height), each written only when not its default, read
  leniently (a size that is no positive number is the default). Kept
  whether the pane is hidden or the lightbox is on.
- **Culling's Picture row at 280**: the label and its two buttons beside
  it left the buttons 82 px each, cutting "Camera JPEG" and "Developed".
  In a panel too narrow for the label and both buttons whole, the label
  goes over them, as the filter's group names do; at 320 and wider the
  row is as before.

## Verified

- `panel::sizes::tests` (headless window, 1500 x 900, real pointer events
  on the edges): each of the three drags moves its pane's line by the
  drag, the viewport gives what the pane took; past the ceiling held at a
  third of the window (500 px wide; the strip at 300 high), under the
  floor held at 200, 280, and 60; a double-click resets each; the left
  edge drags over the grid, where the panel's and the strip's edges are
  gone; a window made 900 x 600 holds a 480 px pane to 300 and the
  panel to its 320 default, the strip to a third of 600, and at 360 high
  the strip's pictures to their 91 px default, and grown back each
  returns to what was asked; `remember` writes only the dragged sizes,
  through the file's text, and a new window reads them back; hiding all
  three and showing them keeps their sizes, the edges going with their
  panes; the lightbox hides all three and the edges, the settings keep
  the sizes through it, and leaving it restores them; a drag on each edge,
  fitted and at 1:1, leaves the develop's center and zoom as they were;
  the strip at 160 px asks the pool for 240, its rings are 160 high, and
  a double-click brings the ask back to 170; at 280 the Culling Picture
  row's label is over its buttons, and each button is no narrower than
  at 320; with the strip scrolled to frame 60 of 99, and to the last
  (End), a drag of 2 px a move keeps the selected frame whole on screen
  at every move, its left edge moving no more than its cell grows (fails
  without the first-layout seed: frame 60 off the right edge at once);
  a picture landing at 176 after the strip's ask rose to 240 is asked
  for again at 240 (fails without the strip in `deliver.rs`).
- `strip::tests`: a taller row's widths, and `render_size` at the
  default, the floor, 160, and past the cap. `settings::tests`: the sizes
  read leniently and absent from a default file.
- The whole of `cargo test -p greycard-ui` (171 + 1381 + 3 passed, 5
  ignored), `cargo fmt --all --check`, and `cargo clippy --all-targets
  -p greycard-ui -p greycard-slint` with `-D warnings`, clean; no Slint
  binding-loop warnings.
- Captures, debug build, headless weston (pixman), the window fit to
  1500 x 800, a fresh weston and config each, the panel's width set in
  the settings file: the Develop panel at 280 and 480 with each section
  opened alone and scrolled to (Light, Color, Curves, Color Mixer,
  Black & White, Color Grading, Tint, Noise, Lens, Detail, Sharpen), the
  scopes and tabs at the top, the Masks tab, and Culling. At 480 every
  section reads as at 320, wider: the sliders' tracks lengthen, the
  curve's square grows with the panel, the grading wheels grow and
  spread, the tint ring stays its 176 px centered. At 280 every Develop
  section fits; the tracks are shorter (about 120 px). Culling at 280 cut
  its Picture buttons, fixed as above (before and after captured). One
  capture with the left pane 320 and the strip's pictures 180: the tall
  strip's picture is sharp.

## Not verified

- The drag and the resize cursor by hand, and the feel of dragging the
  Develop panel with a develop running (the viewport resizes every move,
  as in a window resize).
- The Masks tab with masks in its list at 280 (a fresh config has
  none), and the sheets: they are drawn over the window at sizes of
  their own and do not follow the panel's width.
- The review drove all three edges in the real window: they work.
- "Luminance" in Color Mixer reads "Luminan..." at 280 and at 320 too
  (the review saw it): the slider's label column is a fixed 68 px, and
  it predates this section.
- A double-click on an edge with its presses 240 ms apart did not reset
  in the review's probe: DragEdge's double-click is Slint's click timing,
  shared with §603's gaps, and wants a real mouse to judge.
- A cached 360 px thumbnail is not served for a 176 px ask (predates
  this section).
- A strip past 240 px pictures: the pool's long edge stops at the grid's
  360, so taller pictures are scaled up from it.
- A window under 960 wide or 800 high on a real screen, Windows, and
  macOS.
