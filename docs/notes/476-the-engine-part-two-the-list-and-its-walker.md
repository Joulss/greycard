# §476. The engine, part two: the list and its walker

*2026-10-31*

The first half of §454's part 2, on §469's type: the engine crate
exists, with the stage list and the walker, and core's `finish` has
lost what was never its own. Nothing calls the walker yet; the three
develops run as before, and 28 exports across eight raw formats are
byte-equal before and after, as 16-bit TIFF and as JPEG, with the
dates pinned through a preloaded clock. Master itself was found
nondeterministic on one Nikon TIFF by one unit in a corner patch,
which later checks allow for.

**The crate.** `greycard-engine`, between render and the window; it
depends on core, edit, render, gpu, ai, and lens, never on the window
or Slint, and the GPU is lent to it, never made. `stages.rs` holds
the types of §469 and derives the list for each purpose as the three
develops do today: Bin in place of the demosaic under Reduced, the
learned tier and the profiled denoise skipped there, the defringe
scaled by the bin, the fills refused, the planes inline, the sharpen
skipped, Prefetch ending at the base, Export placed all on the CPU.
Two stages are split out of core's combined paths, CA after a
prepare with CA off and the profiled denoise after the demosaic,
each bit-exact against the combined path on a frame large enough
for the CA to act. A list's options compare resolved, the DCP by
pointer, and the prepare options destructure every field so a field
added later cannot slip past the cache.

**The walker.** `walk.rs`: the buffers between stages, the plan that
places each step, the kept points, the Models and Hold traits, and
the walk with the kept-prefix rule. A declined device run goes to
the CPU from the last allowed kept point with the context kept; any
other device error drops the context and replans the rest on the
CPU; a device point serves only a walk whose next step runs on the
device, since nothing on the device writes the texture but a run. A
Reduce stage does not commute with a turn, since it drops the pixels
past the last whole block. `finish.rs` holds one `FinishInput` for
the shader and the CPU finish.

**Core's finish, trimmed.** Dehaze and the sharpen, their settings,
and their stats are gone from core; the edit stops handing them
over; the command line carries them in a local, in the old order;
its output is byte-equal.

**Where this half departs, stated.** ModelsSee reproduces today's
picture, through local contrast and dehaze, rather than §469's cut
at the base, which lands with the pixel-changing set after. The
masks' rasters are made at the end from the base and the patched
picture, so the models stay out of the stage loop. Prefetch keeps
the Learned stage; refusing a learned tier or a picture stays with
its caller. Three helpers are copied into the engine until their
modules move in the second half, which must delete the originals.
The second half must also forget the kept device buffers before
lending a new context after a lost device, and rerun the bar once
the walker is called.
