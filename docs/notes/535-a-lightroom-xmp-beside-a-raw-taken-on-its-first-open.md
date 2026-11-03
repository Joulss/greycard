# §535. A Lightroom XMP beside a raw, taken on its first open

*2026-11-02*

Issue 61: a tester exported their Lightroom library with the XMP
sidecars beside the raws, imported the folder, and saw none of their
edits. The develop came across only from a Classic catalog (§329,
§337) and from presets (§52); an XMP beside a frame gave its meta
alone (§124). Now the develop in it comes across as a catalog's does.

## What is taken, and how

`greycard_edit_io::lightroom::beside` is the catalog's per-image path
for one file: the `crs:` settings mapped with the same mapper, scales,
and body offset (`lightroom::read_for`, the XMP twin of
`read_table_for`), over the frame's starting edit (its learned blend at
its ISO), its crop and its turn laid on by the same code, recorded as
one state, "Imported from Lightroom". To share that code the crop and
turn placement (`crop_onto`, `turn_onto`, `Cropped`, `Turned`) and
`starting_of` moved from `lightroom_catalog` into edit-io; the catalog
keeps thin wrappers that read its Lua table and orientation code. An
XMP has no catalog size, so the crop's shares are taken against the
decoder's size; Lightroom's orientation is `tiff:Orientation`.

**The meta.** The take lays the packet's rating (and a -1 rating's
reject), label, title, and caption, and its keywords from
`lr:hierarchicalSubject`, slashed, so they read `Places/Scotland/Skye`
as a catalog's do, on each field the `.gcd` on disk leaves empty. Every
other field is the frame's as the open's adoption left it: ours, or the
XMP's when the XMP changed since the `.gcd` last took its word (§124's
rule, which the take does not undo). What came from the XMP on that
read, the adoption's fields and the take's, is recorded in the sidecar
at the values laid (`lightroom`, `meta::Laid`), for a catalog import to
know which fields are Lightroom's. The take marks the XMP as adopted
when it is the file the adoption reads, so the next open lays nothing
of it over what the frame holds. The field is optional both ways: a
build before it skips it on read and drops it on its next save, and
an empty record makes every field that says something ours, the
conservative side. A two-places join (§233) carries it, the newer
copy's first, field by field, and a take chains its record on any the
`.gcd` already holds (a catalog's, for one never developed there).

**Which file.** `IMG.CR3.xmp`, and `IMG.xmp` unless another raw in the
folder has the stem; of those, the one that holds `crs:` settings, the
newer if both do, so an XMP of ours with the meta alone under one name
does not hide Lightroom's under the other. Unlike the adoption's rule
(§124), a JPEG beside the raw does not stop the stem's name: by the
pair rules (§332) the JPEG is the raw's companion, and Lightroom keeps
a JPEG's settings inside it.

**When there is nothing to take.** Lightroom writes `HasSettings`
true on a frame nobody touched, with every slider written at its
start, so the test is on Lightroom's side, not ours: settings with
every slider the mapper reads where Lightroom starts a frame (the 2012
process's zeros, Sharpness 40, Color noise reduction 25, the older
processes' starts for a file from before 2012, the mixer's bands at
zero), the white balance as shot, the 2012 curves straight, no black
and white, no crop, and no masks, spots, or filters are a frame
untouched, and nothing is taken, whatever `HasSettings` says. The lens
profile's switch is left out of the test: Lightroom lets a user change
its default, and either way is a frame untouched. Taken, an untouched
frame would have had the GFX's body offset and the lens profile as
Lightroom's default had it, where the catalog leaves it alone.

## Where it runs

Only a reader that knows where the editor's sidecars go takes, and it
is told by the caller, never by a global: an `Option<Placement>`, none
with sidecars off.

- **On an open.** `on_disk::load_sidecar_said(f, sidecars)`, which the
  window reads through (`held::from_disk`, the rows), and the headless
  export, takes after the adoption when the sidecar holds no edit of
  ours (no history, snapshot, or version, and the default edit but for
  the learned blend), and writes the `.gcd`, so every reader after it
  finds the edit on disk and the window never holds a default that a
  later save would write over the take. A `.gcd` that will not read is
  never written over. The readers that do not know the setting (the
  reduced develop, the masks, the edited pictures, the export record,
  the merge) read through `read_sidecar_said` and take nothing, so no
  binary or test writes a `.gcd` into a folder by reading a frame.
- **In the index.** `Library::set_lightroom_takes`, as
  `set_jpegs_apart` carries the JPEG switch, set by the window's
  indexer from the setting before each pass and by the command line's
  library commands; none by default. A pass takes for a frame with no
  edit of ours (no `.gcd`, or one with meta alone) whose XMP holds a
  develop, after the carry of a moved raw's
  sidecar, so a raw moved by hand without its hidden `.gcd` gets its
  own edit back, not Lightroom's. The row then reads the `.gcd`; a
  second pass, or a rebuilt index, finds it and takes nothing. A pass
  that takes hashes a frame's XMP with a mark ("looked at"), so a row
  whose sidecar and XMP have not changed since is not looked at again;
  a pass that takes nothing (sidecars off) leaves the mark out, so the
  first pass that takes looks at every such row once. Without it a
  pass re-read and
  re-parsed every such XMP and listed the folder for each, about two
  seconds per two thousand rows and growing with the folder. A row from
  an earlier build lacks the mark and is looked at once, a `.gcd` with
  meta alone and an XMP among them. Measured on
  2000 stand-in raws with untouched XMPs: the first pass 5.4 s, the
  next ones 3.2 s against 3.1 s for the build before this one. What is
  left of those 3.1 s predates the take: the adoption's own choice of
  XMP (`xmp::path_of`) lists the folder for each frame with a short
  name, and its read of the raw's orientation for a packet that names
  one costs a quarter of a second a frame on a large RAF, every pass.
- **On a card import.** The XMP lands beside the frame under its new
  name, the develop is taken where the sheet's setting puts sidecars,
  and the preset is laid over it. The report counts the frames, "N
  with a Lightroom edit".

One take at a time per frame: two readers of one frame (the window and
the index) wait for each other, and the one that waited finds the
other's `.gcd`; readers of other frames go on.

## Indexing takes, and the XMP is often stale

Indexing a folder is enough to take: a frame never opened gets its
`.gcd` from the pass. Lightroom Classic writes XMPs only when told to
(automatically, or by Save Metadata to File), so an XMP beside a raw
is as likely as not older than the catalog. A catalog import used to
hold back any frame with a history, and `--force` would clobber real
edits to reach these. The rule now has two halves. Whether the edit
is Lightroom's alone is judged on the history and the state only
(`lightroom_only`): exactly
the one "Imported from Lightroom" state over the frame's start, with no
later step, snapshot, version, or export. Such a sidecar's develop is
replaced by the catalog's without force, from the frame's start, not
stacked on the XMP's; one with a step after the take is ours and held
back as before. The meta is judged field by field
(`Laid::over`): a field the sidecar records as laid, still at the
value laid, or one that says nothing, takes the catalog's; any other
is ours and kept, whatever its stamp, so a rating given here before
the take, in a save of its own or in a `.gcd` from before stamps,
stays when the `.gcd`'s XMP mark matches the XMP; with no mark, or
another, the open takes the XMP's rating first, as §124 has it.
A rating given and then taken off says nothing, and takes the
catalog's. Every catalog write records what it laid the same way, so
an import over an earlier untouched one replaces its fields and keeps
the ones changed here since; a forced import lays the catalog's whole,
as before.

## Not done

- Virtual copies: an XMP holds none.
- A capture time corrected in Lightroom: the adoption never reads the
  dates (§124), and neither does the take.
- An XMP whose save fails (a read-only folder) is taken again on each
  open, in memory.
- A process-2010 file's point curve is not judged: only the 2012
  curves are, since the 2010 default curve is not straight.
