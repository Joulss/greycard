# §463. greycard-edit split by part, its readers beside it

*2026-10-31*

Item 7 of the layering review (§454): greycard-edit's lib.rs held
5900 lines, and the crate held the readers of other tools' files, the
profile and look stores, and the sync algorithm beside the schema.

**The split.** lib.rs is 102 lines of module list and re-exports. The
edit and its look are `edit.rs`, the sections `sections.rs`, the
migrations `migration.rs`, the changes laid over a set of frames
`frames.rs`, the sidecar `sidecar.rs`, and a line's history `line.rs`,
with the test helpers in `testing.rs`. Every root path stays: all 81
public items resolve where they did. A pure move, line for line, but
for `migrate`'s doc, whose first half had drifted onto `turn_into`'s
and is joined again.

**The readers.** The Lightroom, RawTherapee, and XMP readers, and a
preset file's import into the store, are a crate beside the schema,
`greycard-edit-io`, with their 76 tests and the four fuzz targets. It
depends on greycard-edit and greycard-core; nothing in greycard-edit
reaches back. The mark of the XMP a sidecar carries, `xmp::Adopted`,
is a `Sidecar` field and stays with it, re-exported from the new
crate's `xmp`: one type under two paths, and two modules named `xmp`,
which a pure move accepts. The store's import is a free function,
`preset::import_into(&store, path, replace)`: it had one implementor
and no polymorphism, and a trait only kept the method syntax at the
cost of a trait import at every caller.

**What stayed, and why.** The profile and look stores: `settings()`
reads the camera profile and the look table through them, and the
cache key hashes their folders, so the store folder goes with the
schema until a loader is handed in; the listing half could follow
the readers then. The sync algorithm does no file I/O outside its
tests, and `join` reaches private `Sidecar` and `Line` methods: it is
data and conversion, and belongs where it is. The error's `Xmp` and
`Pp3` variants are constructed only by the new crate; an error of its
own is a later step.

**On the way.** rawcolor's last use outside tests left with the
readers, so it is a dev-dependency now, and fuzz/Cargo.lock follows.
