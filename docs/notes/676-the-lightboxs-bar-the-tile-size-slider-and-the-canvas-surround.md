# §676. The lightbox's bar, the tile size slider, and the canvas surround (2026-11-14)

## What the user sees

- **The bar.** In the lightbox a bar comes over the top of the sheet,
  44 px (a control and its padding), when the pointer moves over the
  sheet, the tiles, or the scroll bar, and when the lightbox opens or
  comes back from its loupe, so it is seen once. It fades after 2 s with
  the pointer still (120 ms in, 400 ms out). It stays while the pointer
  is on it, while its slider is pressed (dragged, however long the hand
  rests), while the slider or the close holds the keys, and while the
  pointer rests in the strip the bar covers (the move that woke it may
  land there, and the bar, come up under a still pointer, hears nothing
  of it: the pointer's place over the sheet, kept at each event the
  sheet hears, says so; kept, not worked out from the scroll, or keys
  scrolling the sheet under a still pointer would carry it into the
  strip). Let go
  of, the rest starts again from then. Keys alone never bring it up: a
  user stepping through tiles with the arrows sees only the tiles.
- **It overlays.** No layout change: the sheet, the tiles, and the
  scroll are where they were; the bar covers the first row's top while
  it is up. It ends short of the scroll bar's band, whose thumb sits
  under the bar's strip at the top of the sheet and is grabbed there as
  anywhere. Faded out it is not there for a press; a press there goes
  to the tile under it, and wakes the bar.
- **Its scrim** is the viewport's plates' own: `#00000099`, and
  `over-plate-dense` (`#000000cc`) on a white canvas. Its words and
  icon are `over-text`, its slider on `over-track` with the accent, the
  same in every theme as the plates are.
- **Left: close.** The `x.svg` icon (Lucide's x, already in the set for
  the reject flag), a button to a screen reader named "Close lightbox",
  its default action the click; the tip at the pointer "Close lightbox
  (L)", the key in brackets after the words as the View menu's "Full
  screen (F11)" shows one; there the key is inside `@tr`, here outside
  it (listed in `ui/untranslated.txt` with the other keys), so the one
  new string is "Close lightbox" and the key is the same in every
  language. It leaves as L does: straight to the
  grid, the panes as they were, a set kept (Escape's set-first order
  is Escape's).
- **Middle: the grid header's words.** The frame on screen, or "f00
  and 2 others", then " · " and "{n} frames": the header's own two
  `@tr` strings, no new ones. Centered, cut short with an ellipsis on a
  narrow window.
- **Right: the tile size slider** (below), 160 px, the left box the
  same width so the words sit in the middle.
- **The left edge arrow** is quiet in the lightbox (it shows under the
  pointer only), so nothing is on screen while the user only looks;
  before, its plate sat at the left edge for good. Its ground there is
  the canvas, so a white canvas gives it the dense plate.

## Escape

Already as asked on master (§307): with more than one frame chosen the
first Escape goes back to the frame on screen, the next leaves. Kept,
and tested again with the bar up. With the slider holding the keys,
Escape goes on to the window (the slider passes every key but its own
arrows, Home, and End, through `ButtonKeys`), so it leaves the same way.

## The tile size

**One mechanism, two sizes.** We kept the grid's cell and the
lightbox's tiles across apart, as §307 decided: the lightbox is counted
in tiles across (2 to 6) so its tiles always fill the width, the grid in
pixels (96 to 512); the lightbox's tiles run to 1250 px, past anything
the grid's thumbnails could fill; and §307's tests hold that one does
not move the other. One slider component (`SizeSlider`, controls.slint)
in the grid's header and on the lightbox's bar, one callback
(`tile-size-moved`) that sets whichever is on, and its keys and a screen
reader's step go through the very `grid-zoom` and `lightbox-zoom` that
plus and minus call. Both sizes were already in settings.json
(`grid_cell`, `lightbox_across`), written as the window closes; the
slider writes the same properties, so it is kept the same way.

- **The grid's slider** is continuous, whole pixels, on a log scale from
  96 to the most (below): each of the keys' six steps a similar
  distance along (0.13 to 0.23 of the track), as a zoom's steps are. The
  header's "· 176 px" sits beside it. The Settings sheet's Cell size row
  follows a drag as it follows the keys. The slider's right end keeps
  the largest step, 512, as plus's last step does, whatever the sheet
  holds it to now; so does End with the slider's keys. The keys step on from where the
  slider left the size, to the next step past it.
- **The lightbox's slider** snaps to its five counts, right fewer and
  larger, with a mark at each.
- **The most keeps two across.** At 1366 px with the left pane at its
  widest (a third of the window, 455 px), the grid's sheet is 911 px and
  512 px cells went one across. The cell laid out is now held to two
  across the sheet (`grid::most`, `grid::laid`, and `tile-cell` takes
  the least of the size asked and the most): 441 px there. The size
  asked is kept as it was, so a wider window, or the pane put away,
  gives it back. The slider's right end is the most laid out (512 kept),
  and the keys step from the cell laid out: minus from a held 441 is
  360 at once. The header says the size laid out; the Settings row the
  size set, which is the setting (one line in the guide's Cell size). The
  lightbox's 2 to 6 already keeps two.
- **The least** stays 96 (a name still reads under it), and the
  lightbox's 6 across.
- **Pictures.** The grid's thumbnails are made at the cell's size up to
  360 (`MAX_RENDER`; made sizes 128, 176, 256, 360), as before: a
  441 or 512 cell shows a 360 picture enlarged, as the last step always
  has, and no new size is asked for. The lightbox's tiles are made for
  their size (256 to 2048) as before. The virtualized window is the
  grid's arithmetic at whatever cell, unchanged.
- **Accessibility.** The slider is a slider to a screen reader, named
  "Tile size", its value the tile's size in logical pixels between the
  least and the most (the lightbox's: six across's size to two
  across's), increment and decrement the keys' steps, and a step of a
  whole pixel for the grid's (the drag's resolution) and the range over
  four for the lightbox's (its intervals, on average). No new words but
  the name: the value is a bare number, since a unit would be words to
  translate.
- **Its keys.** A press on the slider takes the keys for the drag and
  gives them back to the window on release, as the header's buttons
  do, so the arrows step frames again; and on a press canceled with no
  release (the pointer leaves the window, the grab is lost), which
  `TipArea` now reports as `canceled`. While it holds them (a drag, or
  a screen reader's focus) its arrows step the size, Home and End go
  to the ends, and every other key goes on to the window.
- **Asking for larger pictures.** The grid and the strip ask a frame on
  screen for a larger picture when its cell outgrows it, once each size
  asked. A drag of the slider moves the size a pixel at a time, and
  each pixel was a new ask for every frame on screen: the pool drops an
  ask for a frame already waiting, but one made or being made was made
  again (a cache hit), and a frame under an offline root
  (`Job::CachedThumbnail`) spawned a cache lookup on rayon each time,
  with no de-duplication at all. A frame is now asked again only when
  the size the pool makes it at (`made_size`: 128, 176, 256, 360) grows
  (`grid::asks_again`), in the grid, the strip, and a late picture's
  ask in `deliver`.
- **What was asked follows what is in hand.** `thumb_asked` records the
  size a frame was asked for and only rose; but the pool makes at the
  size wanted when it begins the job, not when it was asked. A frame
  asked for at 300 and begun after the cells shrank to 128 lands at
  128, and with `asked` still 300 a growth back to 330 found it asked
  already: a 128 picture in a 330 cell until the folder reloaded. (The
  same hole was on master by the keys alone: 176 up to 360, down to
  128, the frame edited so its picture is made again at 128, and up
  again.) So `asked` is brought back to what is in hand wherever the
  two can part: a picture landing smaller than asked lowers it to the
  picture's size, before the late picture's own ask, which then asks
  again for a frame on screen too small; word that no picture came
  lowers it to the picture held (at least 176); a new list, which
  drops the pool's queue, carries no more asked than held; and a root
  back online asks again for its frames whose pictures are smaller
  than the cells want (a miss in the cache at the larger size while it
  was away left the smaller one up).

## Touch and pen

A press with no move before it wakes the bar as a move does, and
starts its rest; the press does what it always did (a tap on a tile
chooses it). We chose waking over a tap that toggles: a toggle would
take the bar down on the very tap that chose a frame, and two taps to
choose would differ from the mouse. With no hover, the bar fades 2 s
after the last touch unless a finger is on its slider.

## The surround

The window decides the sheet's ground (`grid-ground`: the grid's grey,
or the lightbox's surround) and the sheet draws it with no choice of
its own. The lightbox's ground is MONITOR's canvas shade (`canvas-colors
[canvas-choice]`, the loupe's surround), through a new
`lightbox-surround` property, in place of `Theme.lightbox`, which is
gone. The default canvas is Black, `#0f0f0f`, exactly the old fixed
color, so the default renders as before; no special case was needed.

On the shades (WCAG contrast against the ground; the theme changes none
of these, all are in theme-independent colors):

| canvas | current ring | set ring | "Developing" | bar text on scrim |
|---|---|---|---|---|
| Black #0f0f0f | 7.9 (accent) | 1.55 | 4.0 (#737373) | 17.2 |
| Dark grey #262626 | 6.3 | 1.58 | 6.4 (#a9a9a9) | 16.2 |
| Mid grey #2e2e2e | 5.6 | 1.56 | 5.8 (#a9a9a9) | 15.9 |
| Light grey #444444 | 4.0 | 1.46 | 4.1 (#a9a9a9) | 14.6 |
| White #ffffff | 8.5 (#174a96) | 2.04 | 4.7 (#737373) | 10.7 |

- **Rings.** On white the accent blue (2.4:1) gives way to
  `Theme.accent-dark` (#174a96, Light's own darker blue) and its muted
  ring `accent-dark-muted`. The set's thin ring is as weak on the greys
  as it was on black before (1.5:1); unchanged, as it was.
- **"Developing"** takes `over-text-secondary` on the three greys,
  where the dark grey was 2.0 to 2.9:1, and stays `dark-muted` on black
  (unchanged) and white.
- **Badges and the stack's badge** (J, a closed stack) sit on the
  picture on their own scrim; on white they take the dense scrim, as
  the viewport's badges do (`ThumbStack` gained `light-ground`).
- **No label tints** are drawn in the lightbox, so none to check. The
  "Unreadable" box has its own ground and border. The scroll bar keeps
  the theme's thumb (Charcoal's 2.6:1 on white, as the theme notes).

## Tests

In `lightbox/bar.rs`, on the testing backend: the bar up as the
lightbox opens, down after the rest and gone after the fade, keys not
waking it, a move waking it and restarting the rest, the pointer on it
holding it, off it the rest again, none in the grid; a press with no
move (touch, pen) waking it; the pointer resting in the bar's strip
after the move that woke it holding it for 6 s; a drag of its slider
holding it for 5 s and its release starting the rest; a press on the
slider canceled with no release handing the keys back (an arrow steps
the frame, Escape leaves, the bar rests); a press on the bar's scrim
not choosing the tile beneath, the same tile chosen below it; the
scroll bar's thumb, under the bar's strip on 600 frames, dragged; back
from the lightbox's loupe the bar up and then resting; the close
named, its tip "Close lightbox (L)", its click and its default action
leaving with the panes back; Escape's set-first order with the bar up;
the middle's words with one frame and with a set; in the lightbox the
slider's ends, a snapped place between, plus, minus, a screen reader's
step, and Ctrl with the wheel agreeing with the knob and the size said,
the grid's cell left alone; in the grid the slider's ends (the least
and the most), a continuous place in whole pixels, the Settings row
following, plus and minus and a screen reader's step from there, and
the arrows back to the frames after a drag; the step a screen reader
is told, each slider; on a held sheet (1366, the pane at 455) the
slider's end keeping 512 while the header says the size laid out and
the Settings row 512, the pane put away and back and a wider window
giving 512 and taking it back, and plus and minus from the held cell;
a drag through every pixel from 177 to 340 asking a frame on screen at
most once a made size; the sizes the slider sets kept through
`remember`, the settings file, and a new session's start, knobs where
they were; the sheet's ground the grid's grey in the grid whatever the
shade, and each canvas shade in the lightbox, the default `#0f0f0f`;
and the grid's header at 1366 by 768 with the left pane at 200, 240,
and 455 px, English and French, at the largest size, every word as wide
as on a 2560 px window (the count, Rejects, Lightbox; the size's number
aside, which the hold changes), the slider whole, the cell laid out the
most and two across it, held under 512 at the widest pane. Each of the
fix round's tests was run with its fix taken out and failed (the band,
the strip's hold, the cancel, the step said, the slider's end, the
asks); and the second review's (the keys' scroll under a still
pointer, a picture landed small by the slider and by the keys, no
picture, a new list, a root back). In `grid.rs`: `most`, `laid`, `asks_again`, and the slider's
place and cell as inverses, the steps evenly along. In `lightbox.rs`:
the lightbox slider's places and counts.

## Open

- The testing backend's text is not the real font: the header at 1366
  in French is checked by width against itself, and by the captures.
- A screen reader can step the slider but not set a value (no
  `accessible-action-set-value`).
- The slider holding the keys by a screen reader's focus alone (no
  press) holds the bar; nothing in the testing backend focuses an
  element by itself, so only the drag's and the canceled press's
  paths are tested.
- The ground the sheet draws is not read back: the testing backend has
  no renderer. The test reads `grid-ground`, which the sheet draws as
  it is; the captures show it.
- The set's thin ring is 1.5:1 on every dark shade, as it was on black.

