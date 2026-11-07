# §595. One brush row (2026-11-07)

*2026-11-07*

Two testers (discussion 59) painted on a Luminance mask and on a Subject
mask with the brush on Subtract and saw nothing happen. The brush row
(Add, Subtract, Erase) acted on the brush's own paint, and the shape row
(Add, Subtract, Intersect) joined that paint to the mask, so a Subtract
stroke could only lift paint the brush had laid. §531 rewrote a new
brush's first such stroke; a brush already painted kept the trap.

We give the row the mask's meaning. Each stroke is a map on the mask
value `m` beneath it, `k` its coverage times its flow: Add sends `m` to
`m + k(1 - m)`, Subtract to `m(1 - k)`, and Erase moves the map back
toward leaving `m` alone. A composition of those maps is
`m(1 - p - c) + p` for two planes per texel, the paint `p` and the cut
`c`, with `p + c <= 1`. An Add lays paint and lifts cut, a Subtract
lays cut and lifts paint, an Erase lifts both. The paint updates exactly
as the one plane did for every op, so it is the old raster to the bit;
the cut is a second eight-bit plane, made only once a Subtract lands.

A brush joined by Add and not inverted, which is every brush the panel
makes, takes the mask before it to `acc + p(1 - acc) - c acc`, which
is the Add join wherever nothing is cut. Any other brush (joined by
Intersect, or inverted) joins its paint by its mode as any shape does,
and its cut is not read; under Intersect a Subtract still narrows the
mask, since it lowers the paint. On the GPU a brush with a cut takes
two layers of the narrow raster array, paint then cut, as a shape kind
of its own; the rest keep one. The ops apply in order, so a brush
cuts only the shapes before it in the mask, as every shape's join does.

The panel gets no new control or wording. A new brush asked for under
Subtract lands joined by Add with its row on Subtract, so the shape
row's Subtract and the brush row's mean the same thing. The row goes
back to Add when that brush is put down, unless it was moved since, so
the next brush asked for under Add adds, after an undo too. Only
asking for a shape and putting the brush down put it back, so moving
to another mask or frame with such a brush in hand leaves the row on
Subtract until the next shape is asked for; that is harmless, since
every placing starts by asking for a shape. §531's
first-stroke rewrite is gone. An Erase on a brush with nothing painted
does nothing, because there is no paint or cut to remove.

Compatibility. The schema goes to version 8. An edit from before keeps
its picture. On a brush joined by anything but a plain Subtract, its
Subtract strokes become a Lift: Erase's map by the flow with the
feather, never offered, which is what the old Subtract did to the
paint. A "- Brush" (joined by Subtract, not inverted) becomes a brush
joined by Add whose Add strokes are Subtract and whose Subtract strokes
are Lift, so its cut is the old paint and it takes away what it took
away. The first day's `erase: true` flag reads as a Subtract before
either step. A build from before would read a Subtract as a lift, so it
refuses a version 8 sidecar.

The version 8 step cannot run twice: a second pass would turn a
converted "- Brush"'s Subtract strokes into Lift and drop its cut. An
edit from before version 4 with an Original crop waits on its frame's
shape, and it used to wait by keeping version 3, so every later step
ran again at each read. The wait is now a mark of its own,
`frame_pending`, written only while it waits, and the edit is written
at version 8, so each step runs once. A test reads such an edit, writes
it back, reads it again, and checks the brush is unchanged.

The test writes a version 7 edit with each
kind of brush and checks the read-back mask against the old rule
worked out from the paint, to 1e-5.

The shape row's Subtract is now redundant for a brush. Intersect still
means something a stroke cannot say: keep the mask only where the brush
has painted. Inverting a brush keeps the old meaning on its paint: a Subtract
stroke lowers the paint, so an inverted brush gets more mask from it,
and an inverted brush drops its cut, which is only read for a brush
joined by Add and not inverted. Under Intersect a Subtract narrows the
mask.

A cutting brush takes two layers of the GPU's raster array, so the
device's limit on layers (256 by default) arrives at about 128 cutting
brushes across the masks instead of 256. Past the limit the array
stops and the log says so once, rather than a validation failure; a
shape past it reads out of bounds, which WGSL bounds to zero or any
texel of the array. A cutting brush at the edge may keep its paint
layer and lose its cut layer. We
may later dim the row's Subtract while a brush is in hand.
