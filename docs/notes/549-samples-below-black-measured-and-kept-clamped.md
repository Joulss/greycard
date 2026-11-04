# §549. Samples below black: measured, and kept clamped

*2026-11-05*

The pool asked whether `normalize_levels`, which clamps every
photosite at zero once the black is off, cuts read noise's negative
half enough to lift and tint the deepest shadows, as darktable's
unclamped levels would not. We measured before changing anything,
with `examples/below_black.rs`: the levels both ways, each through
the noise estimate, the gains, the default demosaic, the profiled
denoise, and the matrix, read in the frame's darkest 1% of 24x24
blocks and in its 5% to 10% band. The test set's highest ISO is
3200 (X-Trans); there is no ISO 12800 frame. We added the 1600 and
1250 Bayer frames, the 1250 because it is the one with true black
in it.

**What the clamp touches.** Of the photosites, 0.0012% sit below
black at ISO 3200, 0.012% at 1600, and 0.57% at 1250. At 3200 and
1600 the darkest blocks sit two to five sigmas above black, and
nothing moves by more than 0.4%. At 1250, x1000 of white:

| darkest 1%, ISO 1250 | R | G | B |
|---|---|---|---|
| mosaic, clamped | 0.408 ± 0.376 | 0.692 ± 0.515 | 0.361 ± 0.368 |
| mosaic, kept | 0.370 ± 0.430 | 0.679 ± 0.536 | 0.310 ± 0.437 |
| demosaiced, clamped | 0.755 | 0.706 | 0.468 |
| demosaiced, kept | 0.703 | 0.687 | 0.423 |
| developed, clamped | 0.794 | 0.813 | 0.465 |
| developed, kept | 0.757 | 0.793 | 0.435 |

Developed, the clamp lifts those blocks' luminance by 3% (0.787 to
0.762 thousandths, about 10 stops under white), R/G by 0.022, and
B/G by 0.023. On the mosaic the largest lift is the blue's, 0.05
thousandths: under one raw unit of a 14-bit range. In the 5% to
10% band every figure moves by under 0.5%. The noise estimate barely
notices: `b` moves by 1.5% to 3% where it is fitted above zero, `a`
not at all (at 1600, `b` is 4.17e-8 clamped and 4.23e-8 kept; at
1250 the blue's is 3.02e-7 and 3.11e-7). The fit puts `b` at zero for
two of three channels on both Canon frames either way.

**What the change would have to reach.** Dropping the clamp in
`normalize_levels` alone changes nothing past the demosaic, because
every consumer before the denoise either clamps again or assumes
values at or above zero:

- the hot pixel repair's tests are partly ratios (`v > max * ratio`),
  and against a negative neighbor that half passes for any value;
- the CA correction clamps its mosaic at zero, on the CPU and in
  `ca.wgsl` alike;
- RCD's ratio estimates divide by `l0 + far`, which noise around zero
  takes through zero; RCD, AMaZE, VNG4, and Markesteijn all clamp
  their output at zero (the GPU has no demosaic to check);
- the profiled denoise cannot take a negative at all. With `b` at
  zero, `Vst::forward` of a value under `-3a/8` lands past the pole
  of the unbiased inverse, at `d = -2 s0 / a` (1.22 below zero): with
  negatives fed in, the 1250 frame came back with 240 values out to
  2.7e6 and the 1600 frame with 4 out to 4.1e3. Clamped, neither
  had one;
- the learned denoiser's packed input clamps at zero on its own,
  since its training data never went below black.

So the change the pool describes is a rework of four demosaics, two
CA passes, the hot pixel tests, and the denoise's transform, for a
shift under one raw unit in the darkest 1% of a frame. We keep the
clamp. A frame at ISO 12800 or above, with true black in it, is what
would reopen this; the example takes the table again.

The denoise's pole stands on its own: our develop never hands it a
negative, but a consumer that did would be hit, and nothing guards
the inverse against a fitted `b` of zero.
