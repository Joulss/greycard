# §585. A crop fitted into an unknown frame (2026-11-06)

One frame of a folder would not open: its sidecar said "invalid type:
null, expected f32", the editor started it from the default edit and
wrote nothing over the file, and the grid's row said it held an edit.
The file's crop was four nulls, in the current state and every history
state after the first; the angle beside them was zero.

The size of a frame is not known between a switch to it, which zeroes
`source_size`, and the landing of its develop. A geometry change in
that window, an Angle step or a level, refits the crop into a frame of
nothing: the largest fit's bisection divides by the plane's zero size,
NaN comes out, the check for a vanishing fit does not catch it (a NaN
compares false), and `set_crop` takes it. serde writes an f32 NaN as
null, and reads null back as an error, so the frame was shut out of
the editor by its own sidecar. The window is widest while a camera
picture stands in for the develop (§580).

**The fix, in two places.** `refit_crop` leaves the crop as it is
while the size is unknown, as it already did for an edit still waiting
on its frame, and when the fit it made is not finite, which it says in
the log. `Geometry::crop` is read leniently: a crop with a null or
infinite side is no crop. A file written before this opens again, the
whole frame shown, and its next save writes the crop as a number or
not at all; the history keeps its states, with no crop in them.

Tests: a refit on a zero size sets nothing; a crop with a null side
reads as none, a whole one as itself.
