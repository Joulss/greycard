# §529. The merge's CA fitted once

*2026-11-01*

§525 left the CA per frame: each frame of a bracket fits its own
correction, from its own noise and clipping, the fits disagree, and
the merge paid for it (0.0071 to 0.0090 in the middle band). Now the
reference fits once and every frame takes its fit.

**The fit.** Core's `fit_ca` is `correct_ca` handing back what it
used: each pass's polynomial (`Fit`, a pass's made on the last pass's
output) and the color-shift avoidance's blurred factors, red's and
blue's on the avoidance's own grid (2x2 cells on Bayer, 3x3 on
X-Trans). `apply_ca` replays them on another mosaic of the same size
and pattern, and refuses any other: each pass's resample in turn
(X-Trans takes the green plane of the mosaic it corrects, as its pass
does), then the factors multiplied in. The factors are the fit's, not
remade from the frame: remade, they would read each frame's own clip
and noise, the very difference being taken out. `correct_ca` is now
`fit_ca` with the fit let go, so the develop runs the same code; on
four Bayer raws and two X-Trans ones the correction hashes the same
before and after, and a fit applied to its own frame is that frame's
correction to the bit (an ignored test on `GREYCARD_SAMPLES`, and
unit tests on the synthetic aberrations, Bayer and X-Trans). The GPU's
CA tests against the reference pass on the default adapter and on
lavapipe. A fit holds its factors: 90 MB at 45 MP.

**The stage.** `MergeInput` takes `ca: MergeCa`: `Make` lists
`Stage::CaFit`, which corrects and hands its fit back on the walk's
result; `Take` lists `Stage::CaApply` with the fit; `Own` is the plain
`Stage::Ca`; `Off` lists none. The reference walks with `Make`, every
other frame with `Take`. Both stages are the CPU's, as the merge's
whole list is. `Options::ca` is `CaFits::Once` in the command line
and the window; `PerFrame` and `Off` exist for the ignored
measurement only. A frame of another size than the reference's is
refused before its walk, so the merge says "a merge wants frames of
one size", not the CA fit's refusal of a mosaic it was not made on.

**The camera match refuses a merge.** Its brightness is no camera's
and its preview is our rendering. A DNG's style is always bare, so a
merge never reached a group and the index counts it with the raws of
no style. The files' survey now refuses it by name before the style
read, its tag read from IFD0 with no decode
(`decode::merge_tag_path`, through the style reader's bounded TIFF
reader), says so in the log, and counts it with the raws of no style
too, so a folder counts the same indexed or not. Only a `.dng` is
read for it.

## Measured

§525's synthetic bracket (one 45 MP CR3 made into x1, x3.8 and x15.5
frames, the middle the reference), release build, 4 threads, the
pairs' noise. Relative RMS error against the source before the noise,
by band in stops under the reference's clip. The truth is the source
walked with its CA by its own fit (none with the CA off), as §525 had
it; "by the reference's fit" walks it with the reference's fit
instead, which takes out how far one frame's fit is from another's
and leaves the merge's own consistency.

Bilinear demosaic:

| band | CA off | per frame | once | per frame, by the reference's fit | once, by the reference's fit |
|---|---|---|---|---|---|
| -14 to -10 | 0.115 | 0.186 | 0.147 | 0.155 | 0.131 |
| -10 to -6 | 0.020 | 0.023 | 0.022 | 0.021 | 0.020 |
| -6 to -2 | 0.0071 | 0.0091 | 0.0079 | 0.0085 | 0.0070 |
| -2 to 0 | 0.0052 | 0.0052 | 0.0052 | 0.0051 | 0.0051 |
| 0 to +1 | 0.0069 | 0.0081 | 0.0085 | 0.0083 | 0.0068 |
| +1 to +4 | 0.0052 | 0.0073 | 0.0086 | 0.0078 | 0.0051 |

The editor's demosaic (AMaZE and VNG4):

| band | CA off | per frame | once | per frame, by the reference's fit | once, by the reference's fit |
|---|---|---|---|---|---|
| -14 to -10 | 0.284 | 0.325 | 0.304 | 0.300 | 0.286 |
| -10 to -6 | 0.024 | 0.026 | 0.025 | 0.025 | 0.023 |
| -6 to -2 | 0.0095 | 0.0111 | 0.0102 | 0.0105 | 0.0093 |
| -2 to 0 | 0.0057 | 0.0057 | 0.0057 | 0.0056 | 0.0055 |
| 0 to +1 | 0.0110 | 0.0121 | 0.0131 | 0.0121 | 0.0112 |
| +1 to +4 | 0.0107 | 0.0137 | 0.0139 | 0.0143 | 0.0104 |

The per-frame column reproduces §525's CA-on figures to the printed
digit but the deepest band's (0.186 against 0.179), and the CA-off
column its CA-off ones. Fitted once, against the truth by the
same fit, the merge sits at CA off in every band, both demosaics: the
frames are corrected alike and the merge keeps its whole gain.
Against the truth by its own fit it sits between under the
reference's clip, and over it is worse than per frame: bilinear,
0.00848 against 0.00814 from 0 to +1 stops and 0.00860 against
0.00726 from +1 to +4, 18% worse; the editor's demosaic, 0.0131
against 0.0121 from 0 to +1. The truth is the x1 source, clipped less
than the reference, so its fit has votes over areas where the
reference's tiles were clipped and did not vote, and the reference's
polynomial is extrapolated there. That gap is one
fit's distance from another, which any develop of the reference
alone has too, not the merge's; the -14 to -10 band holds 100 to
2000 pixels and moves with any of it.

Time, three frames, editor's demosaic: 15.0 s fitted once, 17.3 s per
frame, 14.1 s off; a frame that takes the fit skips the measuring
passes (3.4 and 3.7 s a frame walked, against 3.7 and 4.0). Peak
resident 3.12 GB against 3.08.

## Left

- Fit on the frame with the most voting tiles, not the reference
  (`stats.blocks` counts them; about half a day, a pre-walk of each
  candidate at about 3.5 s a frame), measured on a real bracket with
  a clipped sky. Votes pooled across frames (a day or more) only if
  that falls short.
