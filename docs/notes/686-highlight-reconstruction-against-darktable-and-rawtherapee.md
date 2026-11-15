# §686. Highlight reconstruction against darktable and RawTherapee (2026-11-15)

§677's bench now scores darktable 5.6.2 and RawTherapee 5.13 the way it scores
the engine: the clipped mosaic as a DNG, each tool's float TIFF read back, fitted
to the engine's color by the frame's own unclipped round trip, and scored where
the input clipped. The question: is any of their methods, guided laplacians in
particular, worth porting, or does the effort belong to a learned model with the
develop's ceiling raised?

## Making the adapters true

They had never run. What was wrong, and what now holds them to account:

- A float TIFF of a 24 MP frame is over the TIFF decoder's default limit of
  256 MiB; every read failed. The limit is lifted.
- darktable's exposure entry is version 7 in 5.6 (a highlight-preservation
  flag), not 6; 5.6 upgraded ours, and we now write 7. The highlights blob
  (v4, 48 bytes) and its mode codes were right; we checked both against 5.6.2's
  source and against the history darktable writes into an export.
- darktable's OpenCL path errored on this driver; the adapter runs it on the
  CPU.
- The history check read only the mode. It now holds every entry the sidecar
  writes (demosaic pinned, RCD on Bayer and Markesteijn 3-pass on X-Trans,
  whose default is 1-pass; the whole highlights blob; the exposure) and fails
  on any enabled module beyond the plumbing (rawprepare, temperature, colorin,
  colorout, flip, gamma).
- RawTherapee is run with `-O`, which writes the profile it merged; we check
  its method, recovery on or off, rcd, compensation, and that nothing but the
  white balance and the crop guide is on. Its configuration and cache go to
  the scratch folder. `LcMode=lfnone` was not a value; `none` is.
- "No two methods may agree" failed honest cases (darktable's segmentation is
  its opposed where no segment has a candidate). The history and the profile
  prove what ran, so an agreement is now logged.
- The fit must find the output linear: the slope of its log luminance against
  the engine's, 1.0000 on every frame for both tools.
- New rows: each tool's develop of the unclipped frame (`dt-unclipped`,
  `rt-unclipped`), the floor, and a third truth, `own`, that tool's unclipped
  develop, against which its methods are also scored; guided laplacians at
  128 iterations and 512 px; the segmentation's rebuild (generic, full
  strength); RawTherapee's CIELab blending; and RawTherapee with recovery off.

## The floor

Each tool's unclipped develop, scored where the input clipped: dE 0.0002 to
0.0003 (darktable), 0.0005 to 0.0006 (RawTherapee), under 0.001 stop. Both use
RCD and the DNG's matrix as we do; after the per-frame fit they are the engine's
picture. Scored against their own unclipped develop instead, no number moves by
more than 0.001. Three clips (ours, darktable's, RawTherapee's with recovery
off) agree to 0.001 in every regime. The cross-tool comparison is as tight as
the comparison among our own methods.

## Results

38 cases from 14 Bayer frames of §677's list (+1 to +3 stops), med dE against
the raw truth by regime 1 / 2 / 3, med abs stops in brackets:

| method | regime 1 | regime 2 | regime 3 |
|---|---|---|---|
| opposed (ours) | 0.007 (0.02) | 0.032 (0.08) | 0.092 (0.35) |
| dt segmentation + rebuild | 0.007 | 0.032 | 0.077 (0.28) |
| rt Color propagation | 0.007 | 0.030 (0.08) | 0.102 (0.42) |
| dt opposed, dt segmentation | 0.007 | 0.032 | 0.121 (0.52) |
| rt Coloropp | 0.012 | 0.033 | 0.118 |
| dt LCh, rt Luminance, rt Blend | 0.022 to 0.033 | 0.071 to 0.074 | 0.172 |
| dt guided laplacians, default | 0.033 (0.11) | 0.151 (0.56) | 0.268 (1.16) |
| dt guided laplacians, 128 it 512 px | 0.030 (0.10) | 0.119 (0.47) | 0.229 (1.07) |
| clip | 0.052 | 0.153 | 0.278 |

1. Regimes 1 and 2: nothing beats opposed by more than noise. RawTherapee's
   Color propagation is 0.002 better in regime 2 pooled and wins 18 of 36
   cases, by a median 0.0009. darktable's opposed is ours (we ported it).
2. Guided laplacians is the weakest reconstruction here, near clip in regime 2
   (0.56 stop under) and contrast 0.28 against opposed's 0.64. It carries a
   surviving channel's texture into a clipped one but not its level, so a
   gradient at the clip comes out flat and dark; a large fully clipped area
   stays the clip's color (magenta) at the default 128 px. The high setting
   helps a little (regime 2 0.119) at 3 to 4 times the run time: 9 s a 24 MP
   frame at default, 24 s high, 30 s and 61 s at 100 MP. Not worth porting.
3. Regime 3: the only method better than ours is darktable's segmentation
   rebuild, which invents light above the clip: 0.077 against 0.092, bias
   −0.19 against −0.35 stop, better in 19 of 29 cases. It goes past our
   ceiling, so against the capped truth it is worse (0.072, +0.19), and it
   beats the capped truth itself (0.084). It is a fraction of what §677's
   oracle-raised shows is there (0.000 in regime 3 against the raw truth).
   darktable's own opposed and segmentation leave a fully clipped area at the
   white-balanced clip, tinted (0.121); our cut at the ceiling, neutral, is
   better against the raw truth.

## Decision

We do not port guided laplacians: it loses to what we have in every regime.
Regimes 1 and 2 are done to within 0.002 of the best of three developers; the
headroom left there is §677's per-region offset, not another tool's method.
The headroom is regime 3, where only inventing light above the ceiling helps,
and darktable's rebuild, a heuristic, recovers a sixth of it; the effort goes
to a learned model with the ceiling raised (§677's harness is its generator),
and the rebuild is the bar it must clear.

## Caveats

- One rebuild setting was tried: generic, strength 1, candidating 0.4,
  combine 2. Another may do better in regime 3, so the bar a learned model
  must clear may be higher than 0.077.
- Guided laplacians was tried at two settings only: not at 1024 or 2048 px,
  with a noise level, or with a lowered clip. It won none of 103
  case-regimes against opposed (38, 36, and 29), so a setting would have to
  move it a long way.
- The pooled numbers are medians over pixels (each case's bucket capped at
  50 000), not over cases; the per-case spread is in the capture's tables.
- Scored in linear scene-referred light: darktable relies on its tone mapper
  to desaturate a tinted clip, which this does not measure.
- Bayer only: darktable runs guided laplacians as Bayer on any sensor.
- 38 cases from 14 of §677's 58 frames.
