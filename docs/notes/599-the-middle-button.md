# §599. The middle button (2026-11-07)

**What a tester asked for.** Discussion 59 asked the middle button
for a drag that moves the picture, a click that zooms to 200% as the
left click zooms to 1:1, straight lines for gradients and brushes, and
a sync "without any questions". We give it those four, each where the
pointer already is: the viewport, and the Sync button at the head of
the develop panel. A middle click anywhere else does what it did,
which in a text box on Linux is the platform's paste of the primary
selection, Slint's own.

**Slint gives the middle button half a TouchArea.** A press of any
button grabs the pointer and calls `moved`, but `pressed` and
`clicked` follow the left button alone. So the viewport's pan is not
read off `pressed`: one function on the viewport (`middle-event`)
keeps its own press, drag, and canvas latch, fed by the picture's
area and by every area above it that would otherwise swallow the
press, the crop's body and handles, a shape's handles, a repair's
handles, the level, and the guide. `CropHandle` passes its middle
events on through a `middle` callback. It pans only from a press on
the picture, as a left drag does. A press of another button while it
pans is nothing, though Slint still marks a left one pressed, so the
picture's `moved` reads its own `held` rather than `pressed`; that
press's release drops the window's grab, so it ends the pan.

**A pan outlives nothing.** An area over the picture can go mid-pan
with no word to it (a key leaves crop, level, or guide, or hides the
handles), and its release then lands elsewhere. So the viewport keeps
which kind of area had the press: one over the picture ends its pan
when any of those modes change or when a move reaches the picture's
own area, which only happens once that area has lost its grab; the
picture's own pan ends when the view loses its hover without the
grab.

**The middle's release ends any left drag.** It takes the window's
grab on the left press with it, so whatever that press was doing (a
pan, a crop drawn or a handle dragged, a shape's body moved, a
dropper, a repair's stroke, the level, the guide) ends there as at its
own release, rather than freezing until a release that may land off
the area. The line chord is one case of it.

**200% and back.** A middle click that did not drag zooms to 200% at
the point; at 200% the next one goes back to the zoom and the place it
came from, fit or not, and to fit when the picture changed size since.
The left click's toggle goes to fit from any zoom; this one goes back,
since a 200% look is mostly a detour from somewhere chosen. The memory
is the frame's (`Develop.zoom_back`, by its version key and size, so another version or a turn fits) and lasts only until
another change of the zoom (the wheel, the menu, a key, a left click);
a pan at 200% keeps it, as looking round there is the point.

**Held in a drag, it holds the line.** The press of the middle during
a left drag placing a shape sets `place-line`, read on every drag. A
linear or a radial takes Shift's hold, so the middle is Shift's alias
there: a linear on 15-degree steps, which take in level, plumb and 45
degrees, measured on the view; a radial a circle. A brush's stroke
becomes two points, its press and the pointer, which the raster
stamps between at the brush's spacing as it does any two points. The
stroke is replaced on each move, not grown, which `Raster::update`
took for a change to all of them and painted every stroke again. Now
when only the stroke under way changed, it copies `base` (the finished
strokes) back and stamps that stroke alone. The
middle's release drops the window's grab on the left drag with it, so
a middle let go first ends the stroke there, and the left release
after finds nothing to end.

**Sync without the sheet.** The panel's Sync button takes a middle
click (`PanelButton.middle-clicked`, and `TipArea.up` under it); no
other button does. It readies the sheet's own ask, the frame, the
targets, the source as shown, then applies it without opening the
sheet, on the choice the last sync or paste took, through
`Checks::with_masks` as a paste's sheet opens, and on the sheet's
defaults before there was one. The sheet itself still opens on its
defaults. The guards are the sheet's: a tool in hand, the queue, no
targets.

**Not done.** The tester also asked for slider drags at a finer scale
with the middle button; that is a slider question, not the viewport's,
and waits. A repair's stroke is not held straight.

Tests: a middle drag pans from a mask's handle, a repair's handle,
the level, and the guide, none taking the press, and a pan whose level
goes away mid-drag ends; a left press in a middle pan drives nothing,
with no tool, under a brush (no point added, the center the middle's
alone), and in crop mode; a middle click in a crop drag ends it and
the next drag moves the crop; the 200% click takes nothing on the
canvas or in the compare view, fits on another frame, forgets after a
wheel zoom, and keeps its place after a pan; a linear with the middle
let go first lands on the step; a replaced last stroke paints what
painting at once does, cut too. And: a middle drag pans under a mask's brush and the heal brush and
lays nothing, and a left press inside it lays nothing; under the crop
from its body and from outside it, the crop untouched, and a middle
click on its body zooms; the 200% toggle from fit and from 1:1 off
the middle, and no toggle after a drag; a linear on a step and a
brush stroke of two points, and the middle let go first; a middle
click on Sync syncs Light alone after a sheet's Light-only sync,
with no sheet.
