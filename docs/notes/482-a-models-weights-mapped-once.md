# §482. A model's weights mapped once

*2026-10-31*

The roadmap's last Mac bug: the remove's UNet loads on WebGPU with no
word in the window, 27 s for Fast's and 40 s for Best's in the editor
on the M5 at a frame's first fill (§477). Its session has no warm-up
(`remove.rs` opens it without one), so the whole wait is the build.

## Where the time went

Timed alone, the Fast UNet's WebGPU session took 10 to 13 s at every
optimization level, Disable the slowest of them, so it was not the
graph's optimization; the 27 s in the editor came after a restart and
with other models loading. A sample of the build put three quarters of
the main thread in the `mmap` system call. The UNet's graphs name 3216
tensors in two external files, `unet.onnx_data` (5.1 GB) and
`unet-lora.onnx_data` (0.39 GB), and ONNX Runtime maps each tensor's
bytes on its own (`GetFileContent` in `tensorprotoutils.cc`, 1.28: map
first, read only if the map fails), about 4 ms a call here. It has no
option to do otherwise.

What it does have is `AddExternalInitializersFromFilesInMemory`: the
caller hands over a file's bytes by name, ONNX Runtime takes each
tensor from them, copies it into the graph, and "the buffer can be
freed after session creation". Read into memory, the Fast UNet built
in 4.1 to 4.4 s, the read 0.4 s of it, but the peak footprint doubled
to 12.4 GB. Mapped once a file instead, the bytes are the file's own
pages, which the footprint does not count:

| Fast UNet, WebGPU, M5 | build | footprint after | peak |
|---|---|---|---|
| ONNX Runtime's maps | 10.4 s | 5.5 GB | 6.1 GB |
| each file mapped once | 1.7 to 2.6 s | 6.2 GB | 6.9 GB |
| each file read in | 4.1 s | 6.8 GB | 12.6 GB |

The 25-step UNet: 9.2 s, now 1.4 s, its footprint after 5.5 GB, now
6.1 GB.

## The change

`runtime::try_open`, where every model's session is built, maps each
`*.onnx_data` beside the model with `memmap2` and offers it under its
file name, which is what a graph's `location` fields name, through
`build_session`. ONNX Runtime takes the tensors a graph names from the
files it names and passes over files it does not name, so the remove's
VAEs, with none, and its 25-step UNet, which names only
`unet.onnx_data`, are unchanged. It does not read a file it was not
handed once a table is given: a graph naming any external file missing
from the table fails to build ("not found from the table user
provided"). So the files are mapped all or none, and a build with the
table that fails, for that or any reason, is made once more without
it, from the files as ONNX Runtime reads them; a model whose weights
are named otherwise (`model.onnx.data`, a subfolder) opens as it did.
The CPU is kept off the table: there ONNX Runtime uses its own maps of
the files as the weights themselves, file pages the system can drop,
where a table's bytes are copied onto the heap, 5.5 GB for the UNet.
The maps live in an `ExternalWeights` kept until the session is built.
The model store only ever renames a finished file into place, so a
mapped file is not truncated under the map. SAM 2.1, the store's other
model with external weights, goes through the same path.

## Checked

- The remove bench, the committed build against this one, one removal
  a sampler at 512, seed 1: Fast's loads 13.56 s, now 1.84 s, the
  removal 17.8 s, now 6.4 s; Best's loads 10.29 s, now 1.53 s, the
  removal 25.7 s, now 17.0 s. The two builds' answers are the same
  PNG to the bit on both samplers.
- SAM 2.1's test on the real model passes, its two sessions built in
  1.83 s.
- A unit test holds which files are offered: the `.onnx_data` beside
  the model, by name and bytes, nothing else, and nothing beside a
  model with none. Another builds two small models with external
  weights through ONNX Runtime (`tests/fixtures/external`): one named
  as the table holds it, which builds from the table; one naming
  `add.weights.bin` beside an unrelated `other.onnx_data`, which the
  table alone fails, as the test also shows, and which the fallback
  opens. Both answer right.
- One run of six of the Fast removal, the first after a rebuild, took
  10.5 s to load against 1.8 to 2.0 s for the rest; the bench logs
  nothing, so whether that build fell back to the files or read pages
  the build had pushed out of the cache is not known.

## What it costs

On WebGPU on the M5, about 0.6 GB more footprint while a UNet session
is open: tensors ONNX Runtime keeps on the host are copies on the heap
now, where before they pointed into its maps, which the footprint left
out. The remove's memory policy (`Memory`, "the UNet kept open, 5.1 GB
held" on 24 GB) is not retuned for it. The CPU, kept off the table,
costs what it did.

## Not measured

The editor's own first fill after a restart, where §477 saw 27 s and
40 s: only the bench was timed, with the files most likely in the
page cache. Linux and Windows, where ONNX Runtime's maps may cost
nothing like 4 ms each, and an M1, where the pixls tester's 47 s first
load (2026-10-28) was likely the same maps.

## Review

A fresh review read ONNX Runtime 1.28's handling of the table and
found it copies the tensors into the graph before any provider sees
them, so the maps are safe to drop once the session is made, and the
drop order sound. It found the first version wrong in three places:
it claimed a file left out of the table would be read from disk, when
ONNX Runtime fails the build instead, so a partly mapped folder or a
model with another naming scheme would have stopped opening, and been
recorded as failing; it handed the CPU the table, losing the CPU's
zero-copy weights; and `files()` handed out a `'static` slice from
safe code. All or none, the fallback, the CPU kept off, and slices
tied to the maps are the fixes, with the test through ONNX Runtime
that the first version lacked.
