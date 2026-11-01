# §489. The fp16 denoisers at high ISO, and a first compile

*2026-10-31*

§481 left two things before the fp16 denoisers ship: frames at the
ISOs a denoiser is for, its CR3 having been ISO 1600, and a first Core
ML compile on a model the system had never compiled, its 0.3 to 0.8 s
first sessions perhaps taken from a cache the system kept. Both were
run on the M5 (macOS 27.2), the files converted as §481 converted
them and run through the CLI's `--ai-denoise` on the model's path.

## High ISO

Six frames at ISO 6400 to 12800 from the archive, four from a Canon
EOS R5 Mark II (45 MP) and two from an R6 Mark II (24 MP), each
developed under the fp32 and the fp16 Best, and two of them under
every tier. All twenty develops went through `open`'s race, and Core
ML won every one. Compared as §481 compared, the fp16 develop against
the fp32 in stops of luminance:

| | PSNR | mean \|Δ\|, mids | 99.9th pct, deep shadows | shadows | mids | highlights |
|---|---|---|---|---|---|---|
| Best, R5 II, four frames | 78.4 to 82.9 dB | 0.0010 | 0.0125 to 0.0134 | 0.0072 to 0.0098 | 0.0028 to 0.0030 | 0.0026 to 0.0027 |
| Best, R6 II, two frames | 74.0 to 79.6 dB | 0.0012 to 0.0014 | 0.0114 | 0.0054 to 0.0070 | 0.0032 to 0.0033 | 0.0031 to 0.0034 |
| Fast, both | 80.3 to 82.4 dB | 0.0010 to 0.0014 | 0.0140 | 0.0059 to 0.0078 | 0.0041 to 0.0053 | 0.0029 to 0.0032 |
| Balanced, both | 79.5 to 82.6 dB | 0.0008 to 0.0014 | 0.0085 to 0.0148 | 0.0056 to 0.0090 | 0.0036 to 0.0041 | 0.0025 to 0.0031 |

The bands are §481's: deep shadows under 0.002 linear, shadows to
0.01, mids to 0.1, highlights above. The deep shadows are under 1% of
every frame's pixels. The figures are §481's at ISO 1600 within a few
thousandths of a stop: the fp16 networks do not part from fp32 as the
noise rises. The difference is again a lift, every band brighter by
about 0.001 stops on average, too small to see.

Best's denoise of a 45 MP frame took about 10 s with the fp32 file and
4.1 to 4.6 s with the fp16, 2.2 to 2.4 times quicker, which is §481's
ratio at Best.

## A first compile

A Core ML compile is kept by the system under the model's contents,
so the fp16 Best was given a new input, a Mul by one ahead of the
network, to make a file no Core ML compile had seen. Opened as the
editor opens a denoiser on Core ML, every compute unit and its fixed
tile, its first session took 0.68 s and a second 0.25 s, and the two
develops are the same to the bit. So the first compile is short at
every compute unit; §477's 112 s was most likely the probe's CPU and
Neural Engine session, which the editor does not ask for.

## What it leaves

Nothing measured stands against publishing the fp16 files. What
shipping takes is §481's: an fp16 file a tier beside the fp32 one,
made by the exporter (`tools/denoise/export.py`), and Core ML handed
it where the other providers keep the fp32. Not measured: an M1 or
an M2, whose Neural Engines compile for themselves, and WebGPU on
other cards, which keeps fp32 until it is.
