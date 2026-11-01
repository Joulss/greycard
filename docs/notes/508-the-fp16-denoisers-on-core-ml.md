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
with the fp16 file races once. The denoise cache's key is left as it
was: the provider was never in it, and the fp16 answer is the same
network's within about 0.001 stops, so the file arriving does not
throw away a Mac's answers.

## How a Mac gets it

On a Mac, `Store::fetch` fetches a tier's fp16 file after the tier's
own, so a tier fetched from now on comes whole; the tier stands if the
fp16 file fails. A tier fetched before keeps working on its fp32 file,
and the editor fetches the fp16 one in the background the first time
the tier loads on Core ML, once a process, with no sheet: it comes
from the same repository under the same license the tier's sheet
showed. Core ML opens it from the tier's next load. The CLI's
`models --fetch` fetches it for a tier already there. Tests do not
fetch (`Ai::for_test` turns it off).

## Checked

- On the M5, on the user store: Best already there without its fp16
  file opened its fp32 one on Core ML; `greycard models --fetch best`
  fetched the fp16 file alone, in 0.9 s, and named it in the license
  note; the next develop opened it on Core ML, raced once, and kept
  Core ML with a warm tile of 0.080 s against WebGPU's 0.607 s, where
  the fp32 file's was 0.202 s.
- Unit tests hold each tier's fp16 file beside its own, by name and
  host, and only the denoisers having one; the store having a tier
  without it, having it by its size, fetching it alone once and
  leaving the tier's file as it was.

## Left

- A tier retired later must list its fp16 file in `RETIRED` as well
  as its own, or the fp16 file stays in the store.
- An M1 or M2: the race guards the speed, but the first compile there
  is not measured (§489).
- The Assets panel's size for a tier leaves the fp16 file out.
