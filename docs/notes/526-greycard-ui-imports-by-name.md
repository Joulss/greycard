# §526. greycard-ui imports by name

*2026-11-01*

Every file in greycard-ui now names what it imports. Before, 49 files
opened with `use crate::*;`, a test module had one more, and nine
files under `roots` and `instance`, and the archive panel's rejects,
took their parent's whole namespace with `use super::*;`, which through
the parent reached the crate root's too: 50 crate globs, 63 globs in
all leaving out the test modules' `super::*`. Now there are none of the
first and 3 of the second. A test module keeps its `super::*` and
imports by name only what its parent no longer brings.

The root's re-export list was the editor's prelude: std, anyhow, clap,
slint, 54 `greycard_edit` and `greycard_core` names and `xmp`, and the
panel types the window state holds. With no glob reading it, that list
is gone. What the root still re-exports, and why:

- The moved modules at their old paths: greycard-ui's library half,
  the render crate's `detail`, `export`, `finish`, and `watermark`, the
  viewport's `render`, and the engine's `worker`, `queue`, `previews`,
  and the rest. Files name them as `crate::worker::Worker`, and the
  paths read as they did before the moves.
- `Shared`, which nearly sixty files name as `crate::Shared`.
- `greycard_slint::*`, the generated window and its structs, named as
  `crate::App` and the like everywhere. It is the generated crate's
  whole surface; a list at the root would be a second copy of the
  `.slint` files' exports to keep in step.
- `Aspect`, `Crop`, and `Geometry`, under `cfg(test)`, which the crop
  tests name by `crate::` path.

The two `rayon::prelude::*` inside function bodies stay; a prelude is
what that glob is for.

Code split by `cfg(test)` or by platform now says which side an import
serves: `#[cfg(test)] use super::tests;` in the roots files, a test-only
`RefCell` in four panels, and `#[cfg(not(test))]` on the imports only
the event-loop path uses. The named pipe's file is compiled only on
Windows, so we checked it, and the macOS code, with `cargo check
--target` for x86_64-pc-windows-gnu and aarch64-apple-darwin; both are
clean. rustdoc's link warnings are as before: two links name their
paths now, one that went through a removed re-export (`gpu_lost.rs`)
and one that went through the glob (`instance/socket.rs`).
