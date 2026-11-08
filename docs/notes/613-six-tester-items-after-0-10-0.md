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
