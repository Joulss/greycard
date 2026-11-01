# §481. The fp16 denoisers on real raws

*2026-10-31*

§477 left the fp16 denoisers waiting on a comparison on real raws, the
probe's random input having put fp16 up to 2.6e-3 from fp32 on a range
of 0.28, and on their first Core ML compile, 112 s with the Neural
Engine and the CPU alone. Both are answered here, on the M5 (macOS
27.2), with each tier converted by `onnxconverter-common`'s
`convert_float_to_float16`, fp32 in and out, and run through the CLI's
`--ai-denoise` on the model's path, so no cached answer stood in. Each
pair of develops went through `open`'s race as the editor's does, and
Core ML won every one.

## The pictures

The 24 MP Canon CR3 at ISO 1600 under every tier, and the 102 MP GFX
RAF under Best, each written as the CLI's 16-bit linear Rec.2020 TIFF
and compared pixel for pixel, the fp16 develop against the fp32:

| | PSNR | mean \|Δ\| in stops | 99.9th pct, shadows (0.002 to 0.01) | 99.9th pct, deep shadows (under 0.002) |
|---|---|---|---|---|
| Fast, CR3 | 77.8 dB | 0.0015 | 0.0072 | 0.021 |
| Balanced, CR3 | 77.7 dB | 0.0011 | 0.0072 | 0.021 |
| Best, CR3 | 77.9 dB | 0.0012 | 0.0073 | 0.022 |
| Best, GFX | 76.2 dB | 0.0013 | 0.0082 | 0.015 |

In the midtones and highlights the 99.9th percentile is 0.0035 stops.
The deep shadows' figure is about one step of the 16-bit file at
those values, so it is the measure's floor, not the network's. The
difference is a lift more than a scatter: every channel of the fp16
develop is brighter by 0.0002 to 0.002 stops on average, green the
most, a shift no eye takes from a frame. The probe's random input was
the harsher test; on a picture the fp16 networks give what fp32 gives.

## The speed

| a 608 tile on Core ML | fp32 | fp16 | the CR3's denoise |
|---|---|---|---|
| Fast | 0.058 s | 0.035 s | 1.4 s, now 1.0 s |
| Balanced | 0.102 s | 0.051 s | 2.6 s, now 1.3 s |
| Best | 0.199 s | 0.080 s | 4.9 s, now 2.0 s |

Best on the GFX frame took 9.5 s against 22.5 s. On WebGPU the fp16
files were quicker too on this machine (Best's tile 0.46 s against
0.59 s), though Core ML beat both.

## The first compile

Opened as the editor asks for Core ML, every compute unit, the fp16
files' first sessions took 0.3 to 0.8 s, after a system update had
come between this and §477's runs. §477's 112 s was the probe's
CPU-and-Neural-Engine session; whether the system kept that compile
for these, or every compute unit compiles another way, could not be
told without clearing a cache that is the system's. So the first
compile is no longer known to be long, and is not known to be short
on a Mac that has never compiled these.

## What shipping takes

An fp16 file a tier, published beside the fp32 one, and Core ML
offered the fp16 file where the other providers keep theirs (WebGPU's
fp16 on other cards, NVIDIA's above all, is unmeasured). The race
would then time Core ML's fp16 against WebGPU's fp32 as it times the
two today. Publishing is a decision about the model store, not taken
here.
