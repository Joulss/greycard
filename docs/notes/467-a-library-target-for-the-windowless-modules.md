# §467. A library target for the windowless modules

*2026-10-31*

Item 4 of the layering review: greycard-ui was a binary alone, so
nothing in it could be reached without the window, and its tests
ran as one process of 1600. It has a library target now, `lib.rs`,
holding the modules that name no Slint type, no wgpu, and no
`State`: display, files, filter, log, mounts, naming, outline,
scrolling, selection, tags, and wheel. The files did not move; the
binary names them at the paths it always has through re-exports,
and its tests run the same code as before. Visibility widened where
the binary reaches in, and a ten-line shift formatter went from the
time shift to the tags, whose notices used it.

**What did not move, and why.** The roadmap named archive, import,
collections, camera match, reduced, thumbpool, and the measurement
runners. Each is tied to the window by a little: import calls four
helpers of the panel's sync and startup; archive needs import and
the rejects half of cull; camera match and the runners take the
worker's frame develop; collections names a Slint chip; zoom uses
cull's tile rects; the GPU marker's tests use the binary's scratch
helper. Reduced, thumbpool, the previews, the queue, and the AI side
are §454's part 2 and move once, to the engine crate. Each of the
rest is a small real change, and a pure move is reviewed as one, so
they are their own items.

**The hazard found.** A module's `cfg(test)` substitution stops
applying to the binary's tests once the module is in the library,
which the binary's tests build without `cfg(test)`. Delete swaps
the system trash for a stand-in under test; moved as is, a test of a
delete would have sent its scratch to the real trash. The stand-in
moves to the one caller first. Every later move checks for the
shape.

**Not yet a crate.** The library's manifest still lists Slint and
the GPU crate, so a consumer would pull them in; a separate crate
is a later step, after part 2 has taken what it takes.
