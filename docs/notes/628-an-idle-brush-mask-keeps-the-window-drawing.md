# §628. An idle brush mask keeps the window drawing (2026-11-11)

A tester's frame with a painted brush drew about 59 frames a second at
rest, each drawing the viewport and reading the scopes back (issue
122); they traced it to the bake themselves.

**The loop.** The window bakes the locals on every frame it draws (the
rendering notifier in `startup.rs`), and a brush's bake ran
`Arc::make_mut(raster).update(strokes)` whether or not the strokes had
moved. The last frame's view still holds that raster (the renderer's
drawn view, the scopes' key), so `make_mut` copied it every time. A
`RasterRef` is equal by allocation and version, so the copy was a new
mask: the scopes' key missed, a read back went out, and a read back in
flight asks for the next frame. That frame baked another copy. The loop
needed nothing but a brush and an open frame.

**Ask the raster first.** `Raster::is_at(strokes)` says whether it
shows the strokes already: those painted, then only strokes with no
points yet. The bake copies and updates only when it is false, and
`update` returns early on the same test, so the two cannot disagree.
Baking per frame stays: with nothing changed it is a comparison of the
strokes, and an idle window draws no frames to bake for.

**A slip in `update`, mended on the way.** A stroke begun with no
points used to copy the paint into `base`, taking the stroke under way
for finished; that stroke repainted (a straight line's end moved) then
painted over its old self. Nothing to do now touches nothing. Live
painting starts every stroke with a point, so this was reachable only
from code.

**How long.** The bake and the scopes' key are the same in 0.10.0, so
we read 0.10.0 as looping too; we found no change in
`v0.10.0..v0.11.0` that would start it, and did not run 0.10.0 to see.

**The other masks.** Learned masks are the worker's rasters, kept as
they landed; refined masks are cached by what they are made from;
`finish::rasterize` makes fresh rasters, but only for the export. One
thing is merely suspicious: the brush texture's layers are keyed by a
raster's address and version, and a freed raster's address can come
back for another at the same version.

**Tested.** The raster's test and `update` agree on three unchanged
and seven changed cases; two bakes with the first held give the same
raster; and on a GPU, the notifier's bake, render, and analyze settle
to five idle frames that neither draw nor read back. The window itself
was not driven: the headless backend does not fire the notifier.
