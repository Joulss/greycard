# §553. The viewport's half floats round to nearest

*2026-11-05*

§269 found that a store to an `rgba16float` texture rounds toward
zero on NVIDIA and lavapipe, where the CPU path's `Halves` round to
nearest, ties to even (the `half` crate): about a third of the
viewport's samples a half-float ulp low, a bias on every GPU op that
writes the viewport. WGSL leaves the store's rounding to the device,
so the ops now round first and store a value the half holds exactly,
which any rounding keeps.

`half.wgsl` holds `half_nearest` and its four-channel `halves_nearest`:
integer arithmetic on the f32's bits (add the rounding bias from the
dropped mantissa bits, truncate; for a subnormal half, the mantissa
shifted to units of 2^-24 and rounded the same way), so no device's
float rounding, fused or fast, comes into it. Past 65504 by half its
ulp it is an infinity, as the `half` crate makes it. The
`with_halves!` macro prepends it to the three shaders that store
halves: the sharpen's apply (and its unsharpened keep), the local
contrast's, and the dehaze's. `pack2x16float` would not do: WGSL leaves
its rounding open too.

### Measured

`greycard-gpu/tests/halves.rs`, a 600x400 picture, each op's viewport
texture against its own full floats on the device rounded by the
`half` crate (only the store differs), and against the CPU
reference's halves. Share of samples off, the worst in half-float
ulps, the mean signed distance:

| Op | Before, NVIDIA | After, NVIDIA | Before, lavapipe | After, lavapipe |
|---|---|---|---|---|
| Sharpen | 38.25%, 1, -0.381 | 0.03%, 1, +0.0000 | 36.80%, 1, -0.367 | 0%, 0, 0 |
| Unsharpened | 36.72%, 1, -0.367 | 0%, 0, 0 | 36.72%, 1, -0.367 | 0%, 0, 0 |
| Local contrast | 36.61%, 1, -0.366 | 0.06%, 1, -0.0000 | 36.59%, 1, -0.366 | 0.06%, 1, +0.0002 |
| Dehaze | 36.40%, 1, -0.364 | 0.45%, 1, +0.0016 | 36.20%, 1, -0.362 | 0%, 0, 0 |

Against the device's own full floats every op is now at the nearest
half on every sample on both adapters (before: 36 to 38% off, the same
as against the CPU). What is left against the CPU is the ops' own f32
differences landing across a rounding boundary: on NVIDIA the
dehaze's full floats differ from the CPU's on 39% of samples, by up
to eight f32 ulps (its fused multiply-adds and its divisions), and
0.45% of those cross a half's boundary; lavapipe's floats are the
CPU's to the bit, as §269 found, so its dehaze is at zero. These are
the new test's figures, not the parity sweeps'; the sweeps, at two
seeds on both adapters, pass unchanged.

The rounding itself is checked against `half::f16::from_f32` on
453,972 values on the device: every half, each tie between two halves
and the f32 either side of it, both signs, the f32 subnormals, the
zeros, the infinities, 65504, 65520, and the subnormal edges, and
200,000 values from 2^-30 to 2^17. The rounded f32 and the stored half
are the crate's to the bit on both adapters; the bare store misses
191,454 of them on both.

§269 counted the samples a level off after the 8-bit quantization,
and the extra half point the truncation added should go with it: by
§269's own numbers, the sharpen off, the share returns to the CPU
halves' own 0.66 to 1.15%, and the sharpen on stays near 2%, which is
the sharpen's own difference. Not remeasured on a frame; one level on
that share is not visible, so this has no changelog line.

### Tolerances

Tightened, none loosened: the dehaze's viewport test from "half an ulp
either way" to the nearest half to the bit; the local contrast's
viewport test from 2e-3 to 1e-3 relative (measured 4.87e-4 on both);
the engine's Detail section on the GPU against the CPU from two half
ulps to one (2e-3 to 1e-3, measured 4.87e-4 to 4.89e-4). The new test
holds every op to its full floats at the bit, to the CPU's halves
within one ulp, and the mean within 0.01 ulps.

### Cost

None measured. Release, the 5070 Ti, 6000x4000, the median of five,
three rounds alternating the two builds (one-minute load 11): sharpen
48.8 / 48.9 / 48.8 ms before, 48.9 / 48.9 / 48.7 after; local
contrast 7.5 before, 7.5 to 7.6 after; the dehaze's apply 1.4 to 1.5
before, 1.4 after. These are the three ops that write the viewport;
the rest of the develop's GPU work does not change.

### Open

Metal and DX12 are not run. The rounding is integer arithmetic, so
fast math cannot touch it, but a device that flushes half subnormals
at the store would still flush them.
