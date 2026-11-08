# §613. Six tester items after 0.10.0

*2026-11-08*

Issues 94, 96, 97, 98, 100, and 101, the day after 0.10.0 went out,
each small enough to decide here.

**Zoom inside a tool (issue 94).** With the brush or a retouch patch
in hand the wheel sizes it, and Ctrl and the wheel, which zooms
everywhere else, sizes it too: the tool's branch in the viewport's
`scroll-event` comes before the Ctrl one. Ctrl and the wheel now zooms
whatever is in hand, and the plain wheel keeps sizing the tool. The
keys `+` and `-`, with or without Ctrl, which size the grid's tiles
and the lightbox's row, zoom the Loupe's picture by one step (1.25,
the wheel's) about its center; the help line gains the Loupe's half.
A tool stays in hand through both.

**Escape ends a retouch tool (issue 97).** Escape clears the brush's
`placing` and leaves `retouch-mode` as it is, so Heal, Clone, and
Fill stay in hand until their button is clicked again. Escape clears
both, in the same place, and a patch half placed is dropped as a
click on the button drops it.

**The keyword field keeps its height (issue 98).** The Keywords
section is a chip flow over a `LineEdit`, and the section's height is
fixed, so five rows of chips squeeze the field to a line. The field
gets a minimum height of one line and the section grows with its
chips; the pane scrolls as it does for any section.

**A learned mask does not flash on a switch (issue 96).** On a frame
switch the worker develops first and asks for the frame's learned
masks once the develops have settled, so the first picture is drawn
with the shape's raster missing and the next with it, and the masked
area jumps. The open frame's learned rasters are read from the mask
cache before its first develop, a disk read, so a frame whose masks
are made draws right the first time; a frame whose mask is not made
yet behaves as today. The builder reproduces the flash first, as a
test on the worker's order or a capture, before changing it.

**Before and after on one key (issue 100).** No key shows the frame
as it was: Reset to Original and undo move the crop too. The
backslash toggles the viewport between the edit and the frame's
Original, the geometry (crop, turn, straighten) carried over from the
current edit so the picture holds still, at the current zoom and
pan. The Original's develop is made once per frame and kept while
the frame is open; the status line reads "Original" while it shows,
and any edit, key, or switch turns it back. In a mask or retouch tool
the key works the same; the tool's overlay is hidden while the
Original shows. The Original is the default edit for a raw and the
picture defaults for a picture (§608).

**Lens corrections on new edits, a setting (issue 101).** A fresh
raw edit has lens corrections on, which the tester reads as the
picture being stretched. Lightroom's own box is off by default (what
looks automatic there is the correction a mirrorless raw carries in
its metadata); darktable and NX Studio are off. We keep on: the
distortion, vignetting, and CA fixes are part of what the profile is
for, and a tester who wants the lens's own drawing can turn them off
once. A Settings switch under Develop, "Lens corrections on new
edits", on by default, decides what a fresh sidecar starts with
(`fresh_sidecar` takes the preference); existing sidecars and
pictures (§608) are untouched.

## Not changed

Moving files between roots by drag or a menu (issue 99) is a release
item of its own, on the roadmap.

## What was built

**A learned mask drawn with the first picture (issue 96): what it was.** We reproduced it on the worker before changing
anything. An open delivered its picture with no mask beside it; the
window, finding a learned shape it held no raster for, drew it as
nothing (a missing raster weighs 0) and asked; the worker answered
from the mask cache, no model run, and the next draw had it. With
the edit's stand-in picture (§270, sized by §580), which carries the
mask, the sequence a tester saw was masked, unmasked, masked.

**What we built.** `Job::Open` carries the learned shapes the window
would ask for at once, each with the model its own `step` names
(`Step::Ask`; `panel::mask::asks_at_open`). Once the open's first
develop is made and before its picture is delivered, the worker reads
each of those from the cache (in memory, else its disk slot, under
that model; never a model run, `Ai::raster_cached`, the first half of
`Ai::raster` split out) and delivers it ahead of the picture,
recorded as given with that model, so a later change of base asks
for it again as the window's ask would have. Left to the window's
ask after the picture, as before: a shape whose step is an offer or
a wait (so the offer, the Subject rewrite's among them, still comes),
one not in the cache, one a Fill patch lies over (remade on the
retouched picture), and one found empty with a word for the status
line (a Sky with no sky): the picture's own line would cover the
word, and an empty raster draws as no raster does, so it cannot
flash. The window takes an answer it did not ask for quietly
already.

**Measured.** On lavapipe/CPU, a 96 × 64 test frame, run alone three
times: an open with its raster cached 0.051 to 0.066 s, one without
0.040 s. Under a parallel test run one sample read 0.299 s, so the
test says the times and holds them to no bound. Timings on a
software device are shapes, not numbers.

**Verified.**
- A worker test opens a frame whose Sky raster is in the cache: the
  window gets the mask, then the picture. Before the fix the same
  test saw the picture alone, and the mask only after an ask.
- A frame with no cached raster develops first, nothing is asked by
  the worker past the settle, and the window's ask is answered after.
- A made Sky not in the open's list (the window would offer), and a
  made Sky found empty, are not delivered ahead of the picture.
- A mask delivered with the open is given with the window's model; a
  key the window did not list is not read.
- The window lists a Sky whose model is in the store and leaves out a
  Subject whose model it would offer, and a shape switched off.
- `cargo test -p greycard-engine` and `-p greycard-ui`, clippy with
  `-D warnings` on both, fmt.

**Not verified.**
- In the editor on a real frame: no capture of the switch was taken.
- The read happens after the develop is made rather than before it;
  nothing is delivered in between, so the window cannot tell.
- A reopen of the same frame still asks again for the rasters it
  holds; the worker answers from memory.
- Between the `Mask` and the `Developed`, a render with the old
  picture still held and no stand-in could draw the old edit with the
  new raster for one frame; before, that gap drew it unmasked.
- `asks_at_open` does not offer the runtime the way the window's ask
  does (`runtime_first`); a cached read needs no runtime, and the
  window's own ask after the picture still offers it for any shape
  not read.

**Zoom inside a tool, Escape, and the Original (issues 94, 97, 100).**

- **Zoom in a tool (issue 94).** The viewport's `scroll-event` takes Ctrl
  first, so Ctrl and the wheel zoom with the brush or a repair in hand;
  the plain wheel still sizes the tool. In the Loupe, `+ = - _` and their
  Ctrl forms zoom a step (1.25, or 0.8) about the view's center through
  the wheel's own `zoom` callback; the grid and the lightbox keep theirs.
  The help line gains "in the Loupe, zoom in or out".
- **Esc a repair (issue 97).** Escape lets go of a chosen patch first, as
  before; then it puts Heal, Clone, or Fill down through `stop-placing`.
  A patch half drawn when Escape comes is removed (`patch-changed(-1)`
  drops the stroke's patch), so none is left behind or developed.
- **The Original on backslash (issue 100).** A new `panel::original`.
  - The Original is `reset::original_of` (the default edit for a raw,
    the picture defaults for a picture) under the edit's geometry: the
    crop, turn, straighten, and aspect, and the lens's drawing of the
    picture, so the crop frames the same picture. Carried from the
    edit: the distortion (on only where the edit's section, profile, and
    distortion are), the manual distortion, the scale and auto scale,
    and the lens chosen; with the edit's lens section off, no
    distortion, no manual, scale 1. The vignetting and the CA stay the
    Original's own.
  - Its develop is a new worker job, `Job::DevelopAside`
    (`worker::develop::develop_aside`): a walk from a clone of the open
    frame's kept points (a clone shares their pictures) into a
    `FrameKept` of its own that goes with the walk, with the device's
    caches left alone. Nothing of the frame's own develop is written:
    its kept points, base, stamp, the edit its masks see, the shelf, and
    `developed_as`; `shown` and the fills due are not touched either. So
    the next develop of the edit goes on from the base kept.
  - Once up, the Original's picture is kept as the renderer's texture
    (`Renderer::developed`, new) with its own white, guide, planes,
    size, and turn, taken from the `Landed` it came in, and so is the
    edit's picture it stood over. Later presses swap the two as texture
    develops, `shown_turn` restored with each, no worker job. The kept
    Original is of a frame and a turn and a lens drawing; a turn, a
    model arriving, or the look or profile directory moving drops it,
    and a frame's open (every switch, culling's included) or entering
    Cull drops everything kept, the textures with it.
  - Every press bumps the generation, so a develop on its way is
    dropped. The edit's picture is put back only when it was its latest
    develop with no fills pending; otherwise the edit develops again.
  - An edit's develop landing while the Original shows (a change by a
    path that did not turn it back) turns it off as it lands, with no
    generation bump and nothing put up, so the fill rounds after it land.
  - While it shows: `view_edit` and the preview white are the
    Original's, the status line reads "Original" (the develop's line
    goes to the log; the sharpen's measured radius and threshold are not
    taken from it), the tools' overlays are hidden (`bare` in the
    viewport), and a press with a tool in hand turns the edit back and
    is consumed, its drag and release included. It turns back on a
    second press, any other key, a look or develop control, a history
    move, or a switch. The scopes show the Original: they read the
    drawn view.

**Verified.**

- greycard-ui tests: Ctrl+wheel zooms with the brush and with Heal and
  leaves their sizes, and the plain wheel sizes the brush; `+`/`-` zoom
  the Loupe and keep the tool in hand, and size the grid's tiles over
  the grid (keys); Escape ends Heal and Clone, and mid-stroke drops the
  stroke's patch and then ends the tool (retouch); the backslash asks
  the Original's develop, says "Original", draws exposure 0 under the
  same crop, and a second press, J, and an exposure change each turn it
  back with the status line restored; the kept Original holds its own
  white, not the edit's base; a turn drops the kept Original; an edit's
  develop landing turns it back at its own generation and its fill
  round still lands; the lens drawing is carried and the vignetting
  and CA are not; the key is in the table (original).
- greycard-engine: a develop aside leaves the frame's stamp, the edit
  its masks see, `developed_as`, and the base picture as they were, and
  the next edit develop keeps the base (the default edit's own develop
  does make another, so the aside's base was a different one).
- `cargo test -p greycard-ui -p greycard-viewport -p greycard-slint -p
  greycard-engine` on lavapipe; clippy `-D warnings` on the four; fmt.

**Not verified.**

- Not run in the editor: no look at the swap on a real frame, the kept
  textures across presses (tests have no renderer, so the kept-texture
  path is untested), Fill's Escape, or a picture file's Original.
- GPU memory: two full-size textures are held while a frame whose
  Original was shown stays open; not measured. The aside walk on the
  device holds its own pictures beside the frame's kept ones while it
  runs.
- "A demosaic preference" names no setting we found; a change to the
  profile or look directory, a model arriving, and a lens database
  fetched (which opens the frame again) drop the kept Original.
- While the Original shows, a re-open of the same frame (a lens
  database arriving) forgets it, and until the open lands the panel's
  look is drawn over the Original's texture for a moment.
