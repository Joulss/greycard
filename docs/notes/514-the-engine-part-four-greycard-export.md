# §514. The engine, part four: `greycard export`

*2026-11-01*

Part 4 of §454's order, and the last. The command line exports an
edit as the editor does, through the same run, the same sheet, and
the same presets.

**The sidecar helpers were down already.** §454 put their move first;
§464 made it (`on_disk`, `held`, `files`, `export_record`,
`sync_note`), so this part had nothing left to move there.

**What moved first, as is.** What the export's plan names reached into
the window's library for three things: the naming patterns, the
export's file names, and the preset laid at the start. They are the
engine's (`naming`, merged with the engine's own `safe`; `export_name`,
with the modified day it falls back on; `preset_fit`), and the window's
library re-exports them at their old paths.

**One reader of settings.json.** `greycard_engine::settings::Shared`
is what a run with no window reads: the sheet, the export presets and
the one last chosen, the `.xmp` and sidecar-folder switches, the
JPEGs beside raws, and the last file. The window's `Settings` flattens
it as its export half, so each name in the file is defined once, and
the path rule (`XDG_CONFIG_HOME`, else the platform's configuration
folder) is the engine's. It reads leniently: every other key is the
window's and passed over, so a window setting written wrong costs an
export nothing. The three readers it replaces were the window's,
headless export's use of the same type, and the command line's own
`editor_setting`, which read two switches key by key for the library
subcommand. The sheet as the run opens it (`--export-preset`,
`--long-edge`, `--on-exists` over the remembered one) is
`Shared::opening_sheet`, which the window's start calls too.

**The run, in the engine.** `greycard_engine::headless` holds §236's
plan and run as they were, behind a `Request` of plain fields in place
of the editor's clap struct, with the overrides, the target's folder
rule, and the `--also` and `--all` checks. The editor's `--export`
fills a request from its flags, refuses the window's flags as before,
logs at warning that it is now `greycard export` and goes in a later
release, and leaves through `_exit` as before. `greycard export` fills
one from its own flags (the editor's names, kept) and ends through
`greycard_ai::runtime::leave`, which takes `_exit` once ONNX Runtime
was loaded, Dawn's teardown being the same crash from either binary.
Exit codes are §236's: 0, 1, 2.

The subcommand takes the frames, then the target: `greycard export
IMG_0001.CR3 out.jpg`, `greycard export a.CR3 b.CR3 out/`, `greycard
export shoot/ out/ --all`. One path is the editor's rule, a folder's
frame being the one last open in it; several are each a frame of the
set, into a folder, with the overrides on every one as `--all` has
them, a folder among them only with `--all`, and never `--also` or
`--preset`. Clap takes them as one list and the last is the target, so
flags may sit anywhere among them. A file target must name a format an
export writes and must not be one of the frames: `greycard export
a.CR3 b.CR3`, a slip for two frames into a folder, would otherwise
write a JPEG over the second raw (or beside it as `b-1.CR3`). The check
is in the plan, so the editor's `--export` has it too.

**`--exposure` means one thing.** The editor read `--exposure 0` as no
override, its clap default; the subcommand as an override to 0, and on
a +1.5 EV edit the two exports differed (AE 32208). Now both take the
flag as an `Option`: absent is the edit's own exposure, given is the
override, 0 included, which is the only way to reset an exposure from
a script. A negative value parses in both (`--exposure -1`). The
window's own `--exposure` at an open reads the same way.

**The Subject model's chooser.** The step for a learned shape (ask,
offer, wait) and the Subject edge's offer are the engine's `ai::step`
and `ai::edge_offer`, pure over the store, the providers, the
session's declines, and whether a sheet is free. The window passes its
declines; an export asks `ai::unattended`, the same chooser with none
and no sheet. Before, an export asked `model_for` alone, so where the
store held the Subject original, on record as failing on WebGPU, it
named the rewrite it had not got and went without the mask (exit 2);
the window, its offer declined, ran the original. Both run the
original now. `subject_model(None)` also names the disk cache's key
for the reduced develop's `raster_ready`, so in that store state the
cache is looked up under the original's key, the one the window's ask
writes under. The learned-mask cache keyed by the base (§454's other
small thing) is not part of this and stays on the track.

**The command line's weight.** greycard-cli depends on the engine
now, which brings render, gpu (wgpu), and the worker. The release
binary, stripped, went from 25.6 MB to 33.2 MB on Linux; the
editor's moved by 0.1 MB. ORT is behind the engine
as it was behind ai, loaded at the first model.

**Held equal.** One CR3 under an edit of its own (exposure, tone, a
level), exported three ways from release builds: master's `greycard-ui
--export`, this build's `--export`, and `greycard export`. As a 2048
JPEG and a full-size 16-bit TIFF, all three are pixel-identical
(`compare -metric AE`: 0), each in about 1.2 s. The bytes that differ
are the moment of writing (EXIF `DateTime`, XMP `ModifyDate`, and in the
TIFF the embedded profile's date); the writer is `greycard 0.9.1` from
both binaries, and each run noted its export in the sidecar the same
way. With a Subject mask and the models at hand, `greycard export`
loaded ONNX Runtime's WebGPU provider, ran the model, and exited 0.

**Tests.** The test names move rather than go: the plan's and the run's
18 tests are the engine's, the `--also` check's from the window's start
with them; the window keeps its flag tests and gains one that every flag
reaches the request; the two integration tests have twins on the
`greycard` binary; the chooser's six tests are the engine's, the window
keeping the fetch-failed status line's. New: several frames named, a
file target that is no export format or is a frame, `--exposure 0` and
`-1` from both entry points, the unattended chooser's two store states,
and the settings reader over a file with window keys it would refuse.