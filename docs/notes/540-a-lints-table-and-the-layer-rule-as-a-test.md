# §540. A lints table and the layer rule as a test

*2026-11-03*

**The table.** The root `Cargo.toml` carries `[workspace.lints.rust]`
and `[workspace.lints.clippy]`, empty, and every crate opts in with
`[lints] workspace = true` (fuzz/ is a workspace of its own and stays
out). The table holds lint levels that differ from the defaults, any
lint we allow or raise workspace wide, so the editor and CI agree; none
do today, and no crate root allowed one, so nothing moved in. The
function-level `#[allow]`s stay. It holds no `warnings = "deny"`,
though that alone would make a plain `cargo clippy` match CI: it would
also stop every plain `cargo build` and `cargo test` on a warning, mid
edit or on a newer stable toolchain's new lint, for whoever builds
from source. CI and the pre-push hook carry `-D warnings` instead.

**The test.** `crates/greycard-core/tests/layering.rs` reads `cargo
metadata --no-deps` once (a `OnceLock`) for the workspace and for
fuzz/, and holds our crates' edges among themselves against
docs/architecture.md section 1's "Depends on" column, kept in code as
`EDGES`. An edge is a dependency on one of our packages by name,
whatever its source (path, registry, or git). The test fails naming
the edge when a manifest gains one the list lacks, when the list keeps
one a manifest dropped, and when a crate is on one side only.
Dev-dependencies are looser: one may name anything the crate's normal
dependencies name (most do, for a `testing` feature), plus
`DEV_ONLY`, today ort-dylib's tests on core alone.

A second check scans greycard-ui/src and greycard-slint, outside test
code, for any way to core's develop: a path through `develop` or
through a name core's lib.rs re-exports from it (read from lib.rs at
test time), a `use greycard_core::{..}` group taking one at its top
level (nested groups included), a glob of core's root, and the same
through a `use greycard_core as` alias. Sites are counted per file and
path, so a second import of a listed one fails. One site is listed
today, to shrink: the viewport's defringe dropper, whose one import
reads `MIN_RADIUS` and `MAX_RADIUS` and calls `deviation_hue` and
`nearer_window`. Sync's copy, which compared against core's
`DemosaicMethod::Bilinear`, now compares the edit's own
`Demosaic::Bilinear`.

It uses the scanner the scratch-removal guard used, now shared in
`tests/scan/`. It sits in core because every CI test job builds core
and core already held the scanner; core gains serde_json as a
dev-dependency for the JSON.

The edges:

| Crate | Depends on |
|---|---|
| core | none |
| edit, lens, gpu, ai, match | core |
| edit-io, render | core, edit |
| library | core, edit, edit-io |
| viewport | core, edit, gpu, render |
| engine | core, edit, edit-io, lens, gpu, render, ai, match, library |
| slint, ort-dylib | none |
| ui | core, edit, edit-io, lens, gpu, render, ai, library, match, engine, viewport, slint |
| cli | core, edit, edit-io, lens, ai, library, engine |
| bench | core, ai |
| fuzz | core, edit, edit-io, library |

**Left.** `greycard_core::horizon::horizon`, at the window's
panel/crop.rs:395, is a core op the window calls outside the develop
module, so out of this check's reach; a roadmap line.
