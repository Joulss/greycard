# §502. No bare folder removal in a test

*2026-11-01*

The Windows CI failure that came back five times (§372 and after) was
always the same shape: a test removed its scratch folder while the
index's SQLite file, or a map, was still open under it. Linux removes
such a folder and the test passes here; Windows refuses and the
runner fails. The helper that waits for the open file and names it
existed in greycard-render's testing module, but the lower crates
could not reach it (render depends on edit), and 468 tests called a
bare `remove_dir_all`.

**One helper, every crate.** `remove_dir_retry` moves unchanged to
greycard-core behind a `testing` feature, so every crate's tests
reach it as a dev-dependency; render re-exports it and edit-io's
private copy goes. On Linux it waits up to 20 s for any SQLite file
or memory map held under the folder, read from /proc, then panics
naming them, so the Windows failure fails loudly here first. Called
while unwinding, from a drop guard in a failing test, it removes once
and never panics, so the first failure is the one reported. Every
bare call in test code was swept to it, 476 in all, each a
like-for-like swap; four were scenario steps rather than cleanup and
lost an unwrap on a folder made a few lines earlier.

**A guard that reads cfg.** A test in greycard-core scans every .rs
file under crates/, blanks comments, strings, and chars, and takes as
test code a crate's tests/ folder, a file or module declared under a
test cfg (and the files below it, one inline level deep), a file
under `#![cfg(test)]`, and the body of any item under such a cfg or a
`#[test]`. A test cfg is one that is off whenever test is, on every
platform: a small three-valued evaluator over all, any, not, and
key = "value", so `not(any(test, ..))` and `any(target_os = "linux",
test)` are production and a shape it cannot read fails naming the
file. A removal that must stay bare says why in a comment that starts
`remove_dir_all allowed:`; only the helper's own two do. The guard
runs in under half a second.

**Known.** The dev-dependency builds all of greycard-core for lens
and ort-dylib's own tests, which a workspace build has anyway. A
sweep script that rewrites flagged lines is kept outside the repo
for the branches still out.
