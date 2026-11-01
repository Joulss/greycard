# §509. The engine, part three: the viewport crate

*2026-11-01*

Part 3 of §454's order. The viewport's renderer, its two shaders, and
their parity tests are `greycard-viewport`, over core, edit, gpu,
and render, never the engine, Slint, or the window. A pure move in the
style of §316 and §318: paths change, behavior does not.

**What moved.** `render/` whole (mod, draw, inputs, locals, sample,
scopes, and the 3,600 lines of tests), `viewport.wgsl` and
`scope.wgsl` beside it, so the `include_str!` paths read as before.
The window names the module at its old path, `crate::render`, by
re-export. Three things the renderer or its tests reached up for came
down with it: the display table (`Lut3d`, the monitor and proof
profiles) out of `display.rs`, whose system half, colord and
ColorSync, stays in the window and re-exports the table; the loss
flag and its poll out of `gpu_lost.rs`, as §454 said, the window's
`Loss` now holding a reference to the viewport's flag so its quit
and start again are unchanged and its tests still make their own;
and `texel_of` and the pick's sample sizes, the shader's own map and
the squares `Renderer::sample` reads, which the placeholder and the
panel re-export. Two things it took from the engine went lower
rather than bring the engine's models, index, and lenses under the
viewport: `Halves`, the picture the engine makes and the viewport
uploads, is the render crate's, and a panic's message is the GPU
crate's, whose wait already read one; the engine re-exports both at
their old paths.

**Plain pictures at the edge.** The analysis picture and the
navigator come back from the device as `Rgba8` (width, height, a
`Vec<[u8; 4]>`), not Slint's pixel buffer; the window makes Slint's
image of the navigator as it sets it, and the readout reads the
analysis picture's pixels directly. No Slint type is in the crate.

**The scopes, split.** The binning (the bins, the weighed bins and
their normalizing, the clipping, the readout under the pointer) is
the viewport's, the reference `scope.wgsl` is held to. The drawing
and the marks stay in the window: they make Slint pictures for one
panel and place marks on those pictures, and nothing below the
window reads them. A test that draws stays with the drawing.

**The render crate's two reads of the shader.** The checks that the
shader's `AGX_WHITE_PAST`, `SHADE_BLACK`, and `SHADE_LIGHT` are the
render crate's constants moved to the viewport crate's tests, the
two constants of the fade made public for them. A dev-dependency of
render on viewport would close a cycle cargo allows but builds the
render crate twice, and the checks are about the shader, whose owner
the viewport crate is now. Nothing outside the viewport crate reads
the WGSL.

**On the way.** The log's own-crate list takes `greycard_viewport`,
so "viewport drawn", the crate's one log line, reaches the file as
before under the new crate's target; the ColorSync lines stayed with
the window's half of `display.rs`. The scopes' test pictures (a ramp,
a flat color, and colors in every scope) are the viewport's `testing`
module, shared with the window's tests of the drawing. The window
drops its `naga` dev-dependency. The `#[allow(dead_code)]` on the
reference binning went, a library's public functions being used by
definition. The moved render files keep §316's `use super::*` of
their module hub, which is not a crate root.

**Held equal.** The workspace's test names are master's but for
the texel test's new path and three new tests (the two shader
literal checks and the plain picture's round trip). The 32 render
tests run on lavapipe with `GREYCARD_REQUIRE_GPU` as master's 29 did,
plus the three. Before and after, on one CR3 under its edit, the
CPU export, the viewport's screenshot through weston, the navigator,
and the histogram's drawing (`GREYCARD_UI_NAVIGATOR`,
`GREYCARD_UI_SCOPE`) are pixel-identical; two runs of master agree
too.

**Left for part 4.** `greycard export` and the shared settings
reader. The window still owns the restart after a lost device and
the scope's drawing; neither waits on anything.
