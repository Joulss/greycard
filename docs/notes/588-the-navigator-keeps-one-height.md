# §588. The Navigator keeps one height

*2026-11-07*

A tester found (issue 81) that the Navigator's preview took its
height from the frame's shape,
and while the next frame loaded its image was empty, so the section shrank
to its header. Folders, Presets, Snapshots, and History jumped up, then down
when the preview landed; a portrait after a landscape moved them too.

The preview now sits in a box of one height, the pane's width at 3:2, the
common landscape shape. A landscape frame fills it; a portrait one is
fitted inside at its own shape and centered, with space either side.

The box also holds its place while a chosen frame's picture is on its way.
The image alone cannot say so: a loading frame, entering Cull, and nothing
open all leave it empty. So the section reserves the box whenever a frame
is chosen outside Cull (`selected >= 0 && !culling`), and with nothing
chosen, or in Cull, it shows the header alone as before. The navigator is
cleared only on entering Cull and while a camera picture stands in for a
develop, so the rule covers the loading gap exactly. We test `culling`, not
the cull mode, so Cull's Developed switch, which fills the navigator, keeps
its box. The Library hides the section as before.

The viewport's rectangle and a press on the preview still work on the
fitted picture, not the box: both live inside it, so the fractions they
read and draw are the frame's. With no picture yet the picture is hidden,
not unmade, which keeps the clip's rounded corners refreshing (the reason
the rectangle was always there) and stops a press on the empty box from
navigating.

A peek from a preset or snapshot row used to leave the navigator as it was,
so the rows would not move under the pointer and end the peek. With one
height that cannot happen, so the navigator now follows the peek, as the
scopes already did.

A UI test checks a 3:2, a 16:9, and a portrait picture and a chosen frame
with none yet: one section height, the header then a 3:2 box, the picture
at its own shape and centered both ways, a press a quarter in navigating to
a quarter, and a press on the empty box navigating nowhere. With nothing
chosen, the header alone.
