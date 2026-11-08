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
