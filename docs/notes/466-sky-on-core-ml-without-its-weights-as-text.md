# §466. Sky on Core ML without its weights as text, and the race remembered

*2026-10-31*

The roadmap's bug: a model's first run on a Mac is a compile with no
word in the window, Sky and the denoisers on Core ML (§422), tens of
seconds. Timed on the M5 (macOS 27.0.1, ONNX Runtime 1.28), the
denoiser was not the slow one: denoise-v20 opened on Core ML in 0.75 s
with both compile caches cleared. Sky was. Its Core ML session took
27 s cold and 4.5 s from the cache, at every launch, on one core in
greycard's own process.

## Where the time went

A sample of a cached load put 80 percent of it under Core ML's
`E5BundleCacheKeyUtils::ToE5BundleCacheKey`, which prints the whole
MIL program to text to hash it, float by float through `snprintf`.
The program ONNX Runtime had written was 336 MB of text: 130
constants inline, every attention and MLP weight of the twelve
layers as hex floats, against 8.9 MB in the weight file. A cold load
pays the same printing several times over: in that cache key, in the
MPSGraph validator's linear pattern, in the Neural Engine's
validator, and in the lowering pass.

The inline weights are named `linear_onnx__MatMul_NNNN_t`. ONNX
Runtime's `MatMulAddFusion`, a Level 1 transformer, turns each MatMul
and the Add of its bias into a Gemm. The Core ML provider maps a Gemm
with `transB` off to `linear`, transposes the weight itself, and adds
the result through `ModelBuilder::AddConstant`, which writes an
immediate value whatever its size (`gemm_op_builder.cc`, 1.28). An
initializer passed by name goes to the weight file once it has ten
elements or more, and a MatMul left alone becomes Core ML's `matmul`
with its weight passed by name.

## The fix

A Core ML session is built with `MatMulAddFusion` off
(`optimization.disable_specified_optimizers`), at whatever level the
caller asks for. Core ML fuses the pair itself, from the weight file.
The other providers keep the fusion. Measured on the M5:

| Sky on Core ML        | before  | after   |
|-----------------------|---------|---------|
| cold, both caches     | 27.4 s  | 1.1 s   |
| from the cache        | 4.5 s   | 0.23 s  |
| a warm run            | 0.086 s | 0.080 s |
| `model.mil`           | 336 MB  | 0.5 MB  |

The answers match the CPU's: on a made-up input, the class logits
within 1.7e-5 of a range of 10.9, the mask logits within 2.7e-4 of
110.7. The Sky tests pass on the real model, the made-up sky labeled
46.9 percent sky as before. The denoisers have no MatMul and are
unchanged.

The option's name joins the Core ML cache key, since ONNX Runtime
does not recompile a cached model for a changed graph from the same
file. Making a file's compile folder now removes that file's older
compiles: ONNX Runtime writes the source's path into each folder's
`model.txt`, and greycard writes the fixed shape into a
`greycard-shape.txt` of its own. A folder naming the same path at the
same shape under another key is older, and so is one with no shape
written, which only a build from before this leaves. The same file at
another shape is in use, not older: the bench's `--tile` beside the
editor's default tile, each of which would otherwise sweep the other
and compile cold at every switch. Without the sweep, every Mac that
had run Sky kept a dead 431 MB folder. A 0.9.1 and a later greycard
on one Mac at once could still sweep each other's unmarked folder. A file no longer offered to Core ML
leaves its folder: Subject's from 0.9.0, 747 MB here, stays (§441).

Apple's own compile cache, `com.apple.e5rt.e5bundlecache`, is kept
per process name or bundle id under `~/Library/Caches`, not under
the folder greycard names. The CLI is `greycard`, so its copy lands
in greycard's own cache folder; the app's is under
`org.greycard.greycard`. Each test binary and example pays its own
first compile, which is why a timing taken from one must clear the
right folder.

## The race remembered

§422 opened both Core ML and the GPU provider after it at every load
and kept the faster warm run, and left remembering the winner for the
Mac. Each side's timed run is now kept in `providers.json` with its
outcome, as an optional `race` holding the run in microseconds and the
fixed shape it ran at (`h=1024,w=1024`; empty for Sky). A later load
of the same file at the same shape, where both sides worked and both
entries still apply to the adapter and build, opens the winner first,
warmed but not timed, with the other after it only if the winner
fails. An untimed load keeps the race already on record, so a settled
race stays settled. A new greycard, a new ONNX Runtime, a system
update, or another tile is a new build, adapter or shape, and races
once again. A record from before reads, with no race.

| load on the M5        | racing  | settled |
|-----------------------|---------|---------|
| denoise-v20 (Best)    | 2.1 s   | 0.4 s   |
| Sky                   | 0.93 s  | 0.30 s  |

Core ML won both: the denoiser's 1024 tile in 0.22 s against
WebGPU's 0.66 s, Sky in 0.085 s against 0.20 s.

## What was not checked

- The editor's window: the load paths are the same functions
  (`Ai::denoiser`, `Sky::load`), timed through the CLI, the Sky
  tests, and an example; a Sky mask and the learned denoiser in the
  app were not run.
- WebGPU winning a settled race: the reorder is covered by tests and
  not seen live, since Core ML won both here.
- The roadmap's other half, a first run under WebGPU sitting for
  minutes, which names no model and is not this.
- Subject's Conv (§441), which fails Core ML's parser and is
  unrelated to the inline weights.

## Review

A fresh review walked every path through `open_fixed` and found the
race and the record sound, and an older record readable. It found the
sweep taking the same file's compile at another tile for an older
one, which is where the shape mark came from, and the record's choice
of race untested inside `open_fixed`, now `race_to_record` with tests
of its own. The probe built its Core ML session with the fusion on,
unlike the editor; it now turns it off too.
