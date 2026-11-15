# §677. A benchmark for highlight reconstruction (2026-11-14)

We want the best highlight reconstruction of any raw developer, perhaps
a learned one later. "Best" needs a number, and so does the headroom over
today's methods. `greycard-highlight-bench` (in `greycard-bench`) is that
number: ground truth by synthetic clipping, scored only where the clip
fell, split by what was lost.

## The method

A raw whose highlights the sensor did not clip is the truth. We overexpose
it by N stops by reading it with its white level lowered by N stops: each
black-subtracted photosite over the new range is 2^N times what it was,
and the normalization clips it at 1, in raw space, before the white
balance, so with the frame's gains the channels clip at different scene
levels as in a real overexposure. The truth is the same frame scaled by
2^N, nothing clipped.

We were asked to choose between this and "scale the mosaic by 2^N, clip
at the white". After normalization they are the same mosaic to the last
bit of float; only the integers in a file differ, and lowering the white
leaves them untouched. Neither has the noise of a real overexposure: a
real +N stop frame gathers 2^N the light and has √(2^N) the shot-noise
SNR, while both variants keep the original's noise and scale it with the
signal, so the simulated input's unclipped neighborhood (which every
method reads) is noisier than a real overexposed frame's would be. The
"lowered white" variant has no less noise scaling than the other; it is
the same thing. We keep it because it touches no integer.

Every develop is the engine's own `prepare`, `demosaic_prepared` and
`finish`, with one setting changed per method: the highlight mode. The
rest is held: one demosaic (RCD on Bayer, scale-free enough that a truth
several stops above the clip takes the path a reconstruction at the clip
takes; Markesteijn 3-pass on X-Trans), no CA correction, no hot pixel
repair, no denoise, sensor orientation. The truth goes through the same
stages with nothing clipped or reconstructed.

Frames: photosites the original read at 95% of its white or above are
not truth; their cells and two cells round them are left out, and a frame
with more than 0.2% of them is skipped. Each frame runs at every N of 1,
2 and 3 that clips between 0.05% and 40% of its photosites.

What it does not model: a sensor's nonlinearity near saturation (Canon
declares a white below its ADC ceiling for exactly this), blooming into
neighbors, the lower noise of a real overexposure, and a camera's own
processing of clipped data (Nikon's and Sony's raws are not raw at the
clip in every mode). The bracket set exists to catch these.

## The metrics, and where each is taken

Only where the input was clipped: a photosite at or above 0.987 of its
white, the engine's own threshold (`CLIP_MAGIC`), the same in the
synthetic and the bracket cases. The unit is the cell, a 2x2 Bayer quad
or a 3x3 X-Trans block, which holds every color; its regime is how many
colors have a clipped photosite: 1 (usually green, or red under warm
light), 2, or 3 (nothing of the scene left). A region is a 4-connected
run of clipped cells; small up to 256 px (speculars), medium up to 16384
(128x128), large above.

Two truths, every score against both. The raw truth is the frame scaled,
nothing cut: what is lost. The capped truth is the same mosaic cut per
photosite at the develop's ceiling, the largest gain, before the
demosaic: what a method can do within today's engine, which cuts every
reconstruction there (`clip_highlights` after the pass). Every method
enters through one function with its ceiling stated, and each row
records it, in units of the develop's: 1 for the engine's methods,
below for clip, raised for the rows that lift it.

In the working space (linear Rec.2020), after the shared demosaic and
matrix:

- stops: log2 of luminance over the truth's; median of its absolute
  value, 90th percentile, and the signed median as the bias.
- dab: Oklab ab distance with both pixels scaled to unit luminance, a hue
  and chroma error alone; its signed means of a and b show a cast's
  direction.
- dE: Oklab distance with both scaled by the truth's luminance: lightness
  and color in one perceptual number, which the oracles split.
- contrast: the ratio of the energies of log-luminance gradients
  (smoothed 5x5 so the truth's noise is not structure), taken only where
  no smoothing window reaches past the region's edge, the stated
  structure number. The first draft let the edge in, where a flat fill
  has a step as large as the truth's, and its correlation read 0.78 for
  clip in regime 3; the correlation stays in the CSV. A pixel's stencil
  can still span cells of another regime inside one region.
- ring: unclipped pixels within 3 px of a clipped cell, their error; and
  the step, median signed stops just inside less just outside.

In the mosaic (white balanced, before the demosaic), for the methods that
return one: stops per clipped photosite.

Pooling: pixels of every case pooled, each case's regime-and-size bucket
capped at 50 000 pixels so no one large frame decides a row.

## Oracles and ceilings

- truth-at-ceiling: the capped truth itself, scored against the raw one:
  the floor under every method the engine can run today.
- opposed-raised: inpaint opposed with the ceiling lifted to the truth's
  brightest: what raising the ceiling alone wins.
- oracle-raised: opposed where anything is left to reconstruct from, the
  truth's photosites where every color of the cell clipped, the ceiling
  raised: the headroom of a method that invents a fully clipped region's
  light, which is a learned model's job.
- oracle-luma: the truth's luminance with opposed's chroma.
- oracle-chroma: opposed's luminance with the truth's chroma.
- oracle-offset: inpaint opposed with each clipped region's ideal
  chrominance offset per color, from the truth: the best opposed's model
  (an opposed average plus one offset) can do with a perfect per-region
  estimate, which is what the segment pass tries to estimate. Under the
  develop's ceiling, as opposed is.

## The first results

We ran 58 frames of our own test set, 139 synthetic cases (28 at +1, 53
at +2, 58 at +3): 55 Bayer (Canon 37, Fujifilm GFX 14, Nikon, OM System,
Panasonic and Sony one each) and 3 X-Trans; skies, snow, skin, clouds,
windows, water, sunsets, and a few each of foliage, blossom, indoor,
neon, the moon, speculars on glass, a white garment, a building. Three
Nikon HE raws need the `nikon-he` build and were left out. Run time 74 minutes on four threads,
shared with other builds.

Which method users get: the editor runs inpaint opposed alone.
`Edit::settings` starts from `DevelopSettings::default()`, whose mode is
opposed, and no edit field changes it. The segment pass runs only from
`greycard-cli develop --highlights segments`; merges use opposed or clip,
stacking and registration clip. We score it as a candidate.

Working space, pooled; each pair is against the raw truth / the capped
truth (med abs stops; med dE), then contrast against the raw truth:

| | regime 1 | regime 2 | regime 3 |
|---|---|---|---|
| clip | 0.10 / 0.10; 0.057 / 0.057; 0.20 | 0.63 / 0.64; 0.151 / 0.147; 0.08 | 1.30 / 0.96; 0.265 / 0.200; 0.05 |
| opposed | 0.02 / 0.02; 0.007 / 0.007; 0.85 | 0.09 / 0.09; 0.037 / 0.033; 0.65 | 0.33 / 0.00; 0.093 / 0.000; 0.47 |
| segments | 0.02 / 0.02; 0.007 / 0.007; 0.84 | 0.09 / 0.09; 0.036 / 0.033; 0.64 | 0.39 / 0.01; 0.105 / 0.004; 0.45 |
| truth-at-ceiling | 0.00; 0.000; 0.90 | 0.00; 0.001; 0.76 | 0.30; 0.085; 0.40 |
| opposed-raised | as opposed | as opposed | as opposed |
| oracle-luma | dE 0.005 / 0.005 | dE 0.027 / 0.026 | dE 0.035 / 0.076 |
| oracle-chroma | dE 0.005 / 0.005 | dE 0.021 / 0.025 | dE 0.074 / 0.035 |
| oracle-offset | 0.02 / 0.02; 0.006 / 0.006 | 0.06 / 0.05; 0.025 / 0.017 | 0.31 / 0.00; 0.090 / 0.000 |
| oracle-raised | as opposed | as opposed | 0.00 / 0.30; 0.000 / 0.090 |

(pixels: 364 M, 363 M and 237 M before the per-case cap. Contrast is
not stated against the capped truth in regime 3: that truth is nearly
flat there, and a ratio over a flat denominator reads 1.19 for opposed.)

1. Regime 3, every color clipped in the cell: under today's ceiling it
   is measured, not improvable; the loss is the ceiling's. Against the
   capped truth opposed is exact (0.00 stop, dE 0.000). Against the raw
   truth it is 0.33 stop under, dE 0.093, contrast 0.47, and the capped
   truth itself scores 0.30 stop under, dE 0.085, contrast 0.40: a flat
   grey field where the truth had a gradient and clouds. Raising the
   ceiling alone wins nothing (opposed-raised equals opposed: its fill in
   a fully clipped cell is built from channels clipped at their own
   gains and never reaches the ceiling). The light has to be invented:
   oracle-raised, the truth in fully clipped cells with the ceiling
   lifted, halves the overall error against the raw truth (dE 0.020 to
   0.010). That is the job of a gradient rebuild (darktable's unported
   rebuild modes and guided laplacians) or a learned model, and either
   needs the ceiling lifted with it.
2. Regime 2 is where a better method pays within today's engine. Opposed
   sits 0.08 stop under; a perfect per-region chrominance offset
   (oracle-offset) halves its capped dE (0.033 to 0.017) and takes the
   bias to −0.02. Overall, against the capped truth, that is 0.011 to
   0.007. Regime 1 is nearly done (0.007, the oracle 0.006). In regime 2
   the luminance and chroma halves weigh about the same (oracle-luma
   0.027, oracle-chroma 0.021, against 0.037).
3. The segment pass is the engine's estimate of that offset, and it
   seldom finds one. It changed a photosite in 83 of 139 cases; in the
   other 56 no segment had a candidate spot. Of 118 497 segments, 1403
   (1.2%) had one. A candidate needs a smoothness weight above 0.6 (the
   spread of cube-root 3x3 block means over a 21-block cross under about
   0.0016, some 0.5% linear over 15x15 photosites, with a 0.75 penalty
   away from the segment's border) and a 5x5 mean above an eighth of the
   clip. Near a highlight's edge the light is still rising and noisy, so
   the test rarely passes. Against the capped truth, the one that
   measures a method, it is within 0.004 dE of opposed in every regime.
4. By kind, opposed raw / capped dE: the moon 0.081 / 0.032 (a blue
   cast), a building 0.062 / 0.019, foliage 0.048 / 0.030, neon 0.047 /
   0.025 worst; white fabric 0.004, clouds 0.008, skin 0.010 best.
   X-Trans behaves as Bayer (0.019 / 0.012 against 0.020 / 0.011).

Mosaic domain (white balanced, clipped photosites), opposed, raw /
capped: 0.03 / 0.03, 0.14 / 0.11 and 0.37 / 0.00 stop median by regime;
oracle-offset in regime 2 0.07 / 0.04. Ring at the clip boundary:
opposed leaves none (median 0.000 stop, p90 0.006, step −0.002 against
clip's −0.027).

Real brackets: one bracket of ours from an R6 Mark II (5 frames 1 stop
apart, plus 4 later frames). Both frames of a pair develop at the bright
frame's as-shot white, so a white balance the camera changed between
them is not scored as error. A shift moves by whole pattern periods so
the mosaic's phase holds (2 on Bayer, 6 on X-Trans). Of 12 mosaic pairs,
2 were static enough (under 10% of 64x64 blocks off by more than 0.1
stop, those blocks and their neighbors left out), one after a (12, 6) px
shift; the other 10 disagree over 25 to 60% of the frame (a sunset with
moving light). Opposed, raw / capped: regime 1 0.02 / 0.02 stop, dE
0.012; regime 2 −0.13 / −0.10, dE 0.042 / 0.043; regime 3 −0.29 /
−0.09, dE 0.120 / 0.088, the capped truth −0.19 against the raw. Unlike
the simulation, regime 3 leaves 0.09 stop under the ceiling: real room,
or a real clip below the declared white (the nonlinearity the
simulation does not model); 60 700 pixels from two pairs cannot tell
which. The segment pass is better here in regimes 1 and 2 (dE 0.010 and
0.036 against 0.012 and 0.042). Two pairs are a check, not a set; a
tripod bracket of a static bright scene would make one.

## External developers

The adapters are built and dry-run tested: the CFA DNG writer
(`greycard_core::dng::write_cfa_dng`) round-trips a real 45 MP Bayer
frame and a 40 MP X-Trans frame through the engine at 0.00006 and
0.00008 stop, dab 0.00002 and 0.00003. darktable-cli and rawtherapee-cli
are not installed; nothing of theirs is scored yet.

A run fails rather than score a sidecar a tool ignored: the unclipped
round trip must fit (its failure is an error now, not a skip); a
darktable export must carry, in the history it embeds, the highlights
entry with the mode asked for; and no two methods of one tool may give
the same picture where the input clipped (clip and opposed must
differ). Both tools get the same exposure pull, 4 stops, acting after
their highlight pass (darktable's exposure module, RawTherapee's
`Compensation`), so a float TIFF clamped at 1 cannot cut what they
reconstruct; the per-frame fit takes it back out.

To verify once the tools are installed, before trusting a row:

- darktable's `dt_iop_highlights_params_t`: the version (we write
  `modversion` 4), the size (48 bytes) and field order, and the mode
  codes (clip 0, LCh 1, laplacians 3, segments 4, opposed 5).
- `scales`: we write index 6 for the laplacians' diameter; check the
  enum and the default.
- `dt_iop_exposure_params_t` version 6, 24 bytes (manual mode, black,
  exposure, the two deflicker values, the bias compensation flag).
- The missing `blendop_params` on both history entries: darktable must
  take its defaults rather than drop the entry; the history check
  catches a dropped highlights entry, nothing yet catches a dropped
  exposure entry except the fit's gain.
- `iop_order_version` 4 and `auto_presets_applied` 1: that no workflow
  module (exposure +0.7, filmic or sigmoid, color calibration) runs.
- The demosaic is darktable's default and not pinned to ours: no setting
  pins it, and a demosaic history entry would be one more unverified
  blob. Check what the default is for Bayer and X-Trans; if it is not
  RCD and Markesteijn 3-pass, add the entry.
- That darktable embeds its history in a TIFF export from the CLI at all
  (the check fails loudly if not; the export's metadata settings are
  the place to look).
- The float TIFF of each tool: whether it clamps at 1 (the pull assumes
  it may) and whether the pull acts where we think it does, after the
  highlight pass.
- RawTherapee: `Coloropp` (inpaint opposed) arrives in 5.10, not 5.9;
  the profile says `AppVersion=5.10`. `Hlth` and `Hlbl` are left out so
  each method runs with RawTherapee's own defaults; check those defaults
  and the method names (`Coloropp`, `Color`, `Blend`, `Luminance`).
  `InputProfile=(camera)` should read the DNG's matrices, and the
  embedded output profile is undone through lcms.

## What a learned model needs from this harness

The harness is already the generator; a training run wants it as a
library, not a report.

- Pairs from the same code: `sim::lowered` and `sim::prepare_scaled` give
  the clipped input and its truth as white-balanced mosaics of one frame,
  in the units the engine's pass sees (each channel clipping at its gain),
  and `ClipMap` the loss mask, the regime and the region of every pixel.
  Training on anything else would train a model for a different input.
- Domain: the mosaic, before the demosaic, where the engine calls the
  pass, so a model drops in beside `inpaint_opposed` and the demosaic, the
  denoiser and everything after stay as they are. Input: the packed
  mosaic (Bayer 2x2 to four planes, as the denoiser's contract packs it;
  X-Trans waits on the 6x6 packing the X-Trans denoiser also waits on),
  the clip mask per plane, and the gains or the clip level per channel as
  constant planes, since the same reading means different things under
  different white balances. Output: the clipped photosites only, the rest
  passed through. The loss in log space on clipped photosites, plus this
  benchmark's working-space dE on the demosaiced result as validation.
- Overexposure as augmentation: 0.5 to 4 stops continuously, not three
  values, and white balance gains drawn from the camera's range (the same
  scene clips in a different order under tungsten), so regime 1, 2 and 3
  all occur at every scale.
- Tiling: 256 to 512 photosite tiles sampled toward clipped regions (most
  of a frame has nothing to learn), with a margin of 64 so a large area's
  border is in view; at inference, tiles with overlap, as the denoiser
  tiles. Regime 3 regions larger than a tile are where a model needs
  context it does not have; the results above say how much they weigh.
- The ceiling: under today's cut at the largest gain a model wins
  nothing in regime 3, so its output enters `sim::develop_mosaic` with
  the ceiling raised, and the engine must raise it with the model.
- Noise: the inputs carry the original's noise scaled with the signal
  (above). A model should see the frame's own noise model (the engine
  measures one) or be trained on frames denoised first, or it will learn
  to put noise back into reconstructed highlights that a real
  overexposure would not have.
- Frames: the 58 used here are a test set, not a training set; keep them
  out of training. The training set needs thousands of unclipped frames,
  and the bracket check (static, same body and settings) as a filter for
  any real pairs.
