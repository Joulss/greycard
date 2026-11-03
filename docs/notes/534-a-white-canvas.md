# §534. A white canvas

*2026-11-02*

A tester asked for a white surround, beside Black and the three
greys: a picture bound for a white page, or a print on white paper,
is judged against white. It is the fifth choice wherever the four
are: the MONITOR section's swatches, the Settings sheet's Canvas row,
the settings file ("White"), and the shader's margin outside the
frame (`render::CANVAS_NAMES` and its levels, #ffffff).

What is drawn over the surround was made to read on black. Most of it
still reads on white: the crop's shade darkens what it covers, the
level and the perspective guide are the accent, and the plates with
the brighter words (the placeholder's, the soft proof's, the guide's
hint) keep about 5:1. The ones with the secondary words do not: on a
#00000099 plate over white, #a9a9a9 falls to about 2.4:1. So on a
light canvas (`canvas-light` on App: its value over a half, which
only white is) the plates and badges over the viewport take
`Theme.over-plate-dense`, #000000cc, and the words keep about 5:1:
the status plate, the loading card, the edge arrows, the compare
view's names, the badge flash and culling's badges, and culling's
notice. On the four dark canvases nothing changes.

The white strokes that run past the frame, the brush's ring, the
repair's, the radial shape's edge, and the linear shape's two outer
lines, vanished over white. Each now draws a dark copy under itself,
3px of #00000080, as the dropper's icon does; the faint feather rings
and the linear shape's middle line stay as they were. The crop's
handles gain the 1px #00000080 edge the mask's handles have.

A chosen swatch is ringed in the text's color, near white on Charcoal
and Grey, so a chosen White showed no ring. A pale swatch (value over
a half, saturation under a fifth; no mixer band is) is ringed in
`over-plate-dense` instead, which reads on it in every theme.

In crop mode the view is the turned source's whole extent, and its
corners hold no source; the shader painted them 0.03 grey, near the
black canvas but dark wedges on white. They take the canvas color
now, as outside the frame. Out of crop mode they never show: the
frame is a crop that fits inside the source, or the largest that
does (`Geometry::effective_crop`).
