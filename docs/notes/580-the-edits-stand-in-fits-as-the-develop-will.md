# §580. The edit's stand-in fits as the develop will (2026-11-06)

A tester (issue 76) on a 5120 × 2160 screen saw every edited frame
come up small, about Full HD, and snap out to fill the view when its
develop landed; unedited frames came up right, and so did edited ones
in a smaller window.

An unedited frame stands in with the camera's JPEG, whose size is the
frame's own, so its fit is under 1 and fills the view from the first
frame. An edited frame stands in with its edit's picture (§270), kept
at a 2048 px long edge, and a fit never enlarges (`zoom::effective`):
on a view wider than 2048 px the copy was drawn 1:1, and the develop,
the frame's own size, landed larger than it. In a window narrower
than the copy both fits are under 1 and nothing moved.

Lifting the cap would be wrong the other way: the copy carries the
edit's crop, so a frame cropped smaller than the view's room would be
blown up to the room and snap smaller when the develop landed at
1:1. What the stand-in needs is the frame's size, and the worker
says it before the develop: `Opened` carries the decoded raw, a few
hundred milliseconds ahead of the develop on a cold frame and at
once on a shelf hit (§378). The window keeps the last eight opens'
develop sizes by path (`Develop::opened_sizes`), so the frame still
standing in while the next one opens keeps its size, and versions of
a file share it; `stands_for` turns the size by
the sidecar's turn, crops it by the edit's geometry, and brings it
back to the copy's orientation. `cull_frame` then takes that size in
place of the copy's own, and the tile math that already draws a copy
on the GPU at the zoom of its source magnifies the copy exactly as
the develop will fit; a zoom asked for while standing in is in the
frame's pixels too.

Before the open has spoken the size is unknown. Rather than draw the
copy small and move it, an edit's stand-in that would sit inside the
view with room to spare (`waits_for_size`) is held back, so the last
picture stays the moment longer, and `size_opened` puts it up as
`Opened` lands; a zoom that stands across frames (`--zoom`) holds
it back too, since the copy's 1:1 is not the frame's. A copy no
smaller than the view goes up at once as before. Left as they were:
a picture that is not a raw carries no size, so an edited JPEG or
TIFF past 2048 px still snaps once; leaving Cull onto an edited
frame not yet opened keeps Cull's picture up and it grows once at
the open. In Cull the copy is the picture, and 1:1 means its pixels.
