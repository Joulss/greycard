# §522. The learned masks kept by their base

*2026-11-01*

The engine track's item from §454: the learned-mask cache keyed by the
base it was made from, so an export's masks are the viewport's (§236's
one known difference).

**What it was.** A Sky, Subject, Background, Object, Landscape, or Part
raster was kept twice. In memory, per worker `Ai`, by the component
and the turn, with the shape; on disk, beside the models, by a hash of
the file's path, the model, the shape, the learned stages' versions,
the turn, the raster's width, and a retouch's print. Neither knew the
picture the model saw. So:

- whichever run made a raster first decided what every later one
  drew: the viewport's (its CA on the GPU), the window's export, the
  command line's, or the worker's `MasksFor`, which makes it from the
  reduced develop (binned, through the Detail section) and put it in
  the slot the open frame reads;
- an edit that changed the base kept the old raster: a white balance,
  a denoise, a demosaic, or a lens correction, which moves pixels, so
  a sky made before a distortion correction stayed where the sky was
  before it, in the viewport and in every export;
- a frame whose cache was cold (another machine's, a fresh
  `XDG_CACHE_HOME`) made its own from the CPU's base and differed
  from the window's (§236 measured AE 174 on a Subject frame).

**The print.** `mask_base::print` names the base: the file (its path
canonicalized, its length, its modification time), the edit trimmed
to what the stages before the masks' tap read (the white balance, the
camera profile, the demosaic, the noise, and the lens; for the
reduced develop also the geometry, which picks its bin, and the Detail
section, which its models' picture runs through), hashed by the
develop key so the profile's and a hand-picked LCP's contents and the
lens database come in as they do there; the quarter turns; and the
size, full or reduced. It is worked out from the edit, not from the
decoded frame, so the reduced develop asks it before the decode as it
asked the old slot. A test moves every leaf of an edit's JSON in turn (a number
up and to zero, a switch flipped, a word to each of the edit's words)
and holds that where the print stays, the ops up to the base stay
too, full and reduced, turned and not: a field a base reads cannot be
left out of the print unseen, wherever it is added. (With the noise
left out of the trimmed edit, it fails at `/noise/enabled`.) Another
holds the print still for what only the finish reads (exposure, tone,
color, sharpen, crop, vignette, grain, a mask's own adjustments).

Both caches are keyed by it. On disk a slot is named as before (the
file, now canonicalized, the model, the shape, the stages' versions,
the turn, the width, a retouch's print), with the size hashed in for
the reduced develop alone, so the full develop's raster and a proxy's
of one frame are kept side by side and the full one's slot keeps the
name it had. The print's hash is in the file's name beside the slot,
`<slot>-<print>.png`: a raster of another base is not read. Writing
one keeps the slot's newest three by when they were made, so virtual
copies of one frame whose white balances differ are read in turn
rather than made again, and removes the slot's raster from before
prints (`<slot>.png`), which no print names: the folder holds a few a
slot however often the base moves. In memory an entry carries the
print it was made under and is read only under the same one. One cache, the worker's
`Ai` over the disk, reachable from the viewport's `Job::Mask`, the
window's export, `greycard export`, and the reduced develop alike.

**The viewport's print is the edit's now.** A develop sets the base's
print from the edit it was asked for, not the one the base was made
under. An export works the print out from its own edit; were the
viewport's the made-under edit's, a field the print takes that a
develop did not need to make the base again for (a lens choice with
the lens off) would part the two. The panel was given a raster under
a print (`Ai::given`), and a develop whose print differs asks for the
raster again (`Ai::of_another_base`): the same picture made again
costs a model run, never a mismatch.

**Asked once a drag has settled.** A mask's model runs for seconds (a
Sky 7 to 10 in our debug runs) and cannot be stopped, so one asked after
every develop would start in the first pause of a white balance drag
longer than a develop, and the next develop would wait behind it. A
develop's asks of a new base are held on the queue
(`masks::Settling`) for a second, each develop's in place of the
last's and the second counted again from it, and moved to the masks
wanted once it passes with no develop since; the worker's wait wakes
for it. A drag asks once, after its last develop; a develop back on
the mask's own base lets go of what is held. Asks whose fills moved
are made at once, as before: a fill is one action, not a drag. A test
runs five develops a third of a second apart and finds no mask job
until a second after the last, and one then.

**Left out of it.** The global look the models' preview is finished
under (exposure, curves, the look): a mask is not made again for a
slider that does not touch the base, as before, and whichever of the
viewport and an export makes it first, the other reads. A preview
finished under one exposure and read under another is a small
difference we accept for not running a model on every exposure drag.

**What it costs.** Every raster made before this is in a file no
print names: each frame's masks are made once again, and the old
files stay in the folder unread (nothing prunes it). A frame whose
masks `MasksFor` made for its reduced pictures makes its own when it
is opened, where it read the proxy's before. A white balance,
lens, or noise change now makes the masks again, a second after the
last develop of it.

**Measured.** On a Canon frame with a sky
(`an_export_s_sky_is_the_viewport_s`, ignored, `GREYCARD_SAMPLE` and
`GREYCARD_MODELS`), the viewport developed with its CA on the GPU, the
Sky made on WebGPU, 31.8 percent sky: the window's export and
an export with no window over the same disk drew the viewport's
raster, 0 texels of 2048 by 1365 different. An export with a cold
cache made its own from the CPU's base: 73 texels different, 1 of them
across the half. That gap is §236's and stays: one disk, one raster;
two disks, two makers.

**What it fixes.** Before this the window's export read the viewport's
raster from memory and a run with no window read it from the same
disk, so they agreed already, on a stale raster as readily as on a
right one. What this fixes is the staleness, which the viewport and
every export shared: a mask made before a lens correction, a white
balance, or a noise change, or the reduced develop's proxy, drawn on
the full develop.
