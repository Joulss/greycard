# §508. The fp16 denoisers on Core ML

*2026-11-01*

§481 and §489 found the fp16 denoisers 1.4 to 2.5 times quicker on
Core ML on an M5, within 0.01 stops of fp32 but in the deepest
shadows, at ISO 1600 to 12800, with a first compile under a second.
The three files are published beside the fp32 ones
(`denoise-v19-fp16.onnx`, `-v15-`, `-v20-`), made from them by
`tools/denoise/fp16.py`, which takes only a file the registry pins and
writes the same bytes §481 and §489 measured. This is how greycard
uses them.

## One file for Core ML, the tier's own for the rest

`registry::core_ml_file` names a tier's fp16 file. It sits in the
tier's store directory but is not one of the model's `files`, so a
store without it still has the tier (`Store::have`), and nothing
already working stops. `Denoiser::from_store` hands it to
`runtime::open_fixed` as the file Core ML opens; WebGPU, CUDA and the
CPU open the tier's own. The race (§466) then times Core ML's fp16
against WebGPU's fp32, as it timed the two fp32 sessions before, so a
Mac whose Neural Engine runs fp16 worse than its GPU runs fp32 keeps
the GPU. The other platforms never see the file.

What the provider record keeps of such a load is filed under both
files' hashes joined by `+`, so a race the fp32 file won or lost on
Core ML does not settle a load that opens the fp16 one; the first load
with the fp16 file races once. Core ML failing the fp16 file is
remembered under that hash too, and the next load, seeing it, hands
Core ML the tier's own file under its own hash, so a Mac whose Core ML
cannot run fp16 (an older chip or system, a file damaged on disk at
its full size) goes back to fp32 on Core ML, as before, rather than to
WebGPU for good; a new system or build tries the fp16 file again. The
denoise cache's key is left as it was: the provider was never in it,
and the fp16 answer is the same network's within about 0.001 stops (a
channel within 0.006, §489), so the file arriving does not throw away
a Mac's answers.

## How a Mac gets it

On a Mac, the editor's fetch of a tier, from its sheet, fetches the
fp16 file on a thread of its own after the tier is said to be here,
since the tier runs without it; the tier's first develop meanwhile
loads its own file, and the editor loads the tier again at the first
develop after the fp16 file is here, from either fetch; the CLI's
`models --fetch` fetches it after the tier, or alone for a tier
already there, and says if it did not arrive. `Store::fetch` itself
does not, so a failure is always said by the caller that can say it. A
tier fetched before keeps working on its fp32 file, and the editor
fetches the fp16 one in the background the first time the tier loads
with Core ML on offer, with no sheet, and again at later develops
until one fetch of its own has failed: it comes from the same
repository under the same license the tier's sheet showed, and the
license note beside the tier is written again to name it. One thread
fetches a file at a time, the others leaving it to that one, and a
download stalled on one file holds up no other. A test build never
fetches in the background, whichever constructor it takes; a debug
`greycard-ui` built by `cargo test --workspace` or `--all-targets` is
one, since greycard-ui's tests turn the engine's `testing` feature on,
so a Mac checking the background fetch runs a plain or release build.

## Said, and refused

Where a develop says where the denoiser ran, it says "CoreML (fp16)"
when the fp16 file is what runs, in the editor's develop line and the
CLI's log. `GREYCARD_DENOISE_FP32=1` keeps Core ML on the tier's own
file, for comparing the two on one Mac or a frame the fp16 file is
suspected on, and the editor then fetches no fp16 file in the
background. There is no setting: the difference is under what a
person could choose by (§489), the tiers already hold the choice of
quality against speed, and the providers already differ from each
other a little without asking.

`tools/denoise/compare.py` prints the figures §489 printed for any two
develops, for measuring the fp16 files on another provider: WebGPU on
Windows and Linux, and CUDA, are not measured, and keep fp32 until
they are.

## Checked

- On the M5, on the user store: Best already there without its fp16
  file opened its fp32 one on Core ML; `greycard models --fetch best`
  fetched the fp16 file alone, in 0.9 s, and named it in the license
  note; the next develop opened it on Core ML, raced once, and kept
  Core ML with a warm tile of 0.080 s against WebGPU's 0.607 s, where
  the fp32 file's was 0.202 s.
- Unit tests hold each tier's fp16 file beside its own, by name and
  host, and only the denoisers having one; the store having a tier
  without it, having it by its size, fetching it alone once, writing
  the note again, and leaving the tier's file as it was; which file
  Core ML is handed, under which hash, refused, or failed before.

## Left

- A tier retired later must list its fp16 file in `RETIRED` as well
  as its own, or the fp16 file stays in the store.
- An M1 or M2: the race guards the speed, but the first compile there
  is not measured (§489).
- The download sheet's size for a tier, and the CLI's, leave the fp16
  file out, about half as much again on a Mac; its download is not in
  the sheet's progress.
- ureq is given no timeout in the store, so a download stalled on a
  dead connection waits as long as the system lets it; one stalled
  fp16 file now holds up only itself.

## Review

A fresh review found four faults in the first version: Core ML
failing the fp16 file left the tier off Core ML for good, about three
times slower than before on the M5's figures; one lock over every fp16
download let a stalled background fetch hold up a fetch the user
asked for; the CLI said a tier fetched when its fp16 file had not
arrived, its warning landing on the progress line; and the background
fetch left the license note not naming the file. The fallback to the
fp32 file, a lock a file, the callers fetching and saying, and the
note written again are the fixes; with them, tests of the plan and of
the note, a test build kept from fetching, and fp16.py's checks made
exits rather than asserts, with the graph's input and output names.

A second review found the sheet's fetch saying the tier was here
before its fp16 file came, so the tier's first develop loaded the fp32
file on Core ML and kept it for the session, and the background fetch,
finding the file under way elsewhere, said it had arrived and gave up
on the tier for the process; and the store's list of files under way
not cleared by a panic. The tier loaded again when its fp16 file
comes, the background fetch giving up only on a failure of its own,
the sheet's fp16 download off the thread its waiters wait on, and a
guard that clears the list however a fetch ends are the fixes; with
them the CLI says a tier in place whose fp16 file did not come, and a
note that cannot be written no longer fails a fetch that worked.
A third review found the file still missed when it landed while the
tier was loading, which the sheet's fetch makes likely (a download of
under a second against a load of about one): whether the tier awaits
it is now asked before the load. A panic in the background fetch now
counts as its failure.
