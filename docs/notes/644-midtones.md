# §644. Midtones (2026-11-11)

**Asked.** A Midtones slider in Light between Highlights and Shadows, so
the middle of a picture lifts or sinks without the curve. Testers asked
(discussion 59); §250 and §315 weighed one and left it out. Built to be
judged in use.

**Why §315's Brightness was not this.** Brightness was a Gaussian in
stops about mid grey (sigma 1.5, a stop at ±100), in fixed stops. It kept
a quarter of its strength at scene white, so on high-key frames it moved
the darkest tenth 0.3 to 0.4 and the top twentieth 0.1 to 0.3, and on
portraits it overlapped the adaptive Highlights. Midtones is placed by
the frame's own range and has compact support: it is nothing at the
frame's top and nothing two stops under its middle.

**The shape.** `midtones_weight` in `finish.rs`, a bell on the region
reading (the guide plane, as Highlights and Shadows read it), over the
highlights span (`highlights_span`, the frame's median to its top, held
to 1.6 to 3.5 stops and hung from the top):

- full at the span's foot, the frame's median where the span is its own;
- falling by a smoothstep to nothing 2 stops under the foot
  (`MIDTONES_BELOW`);
- falling by a smoothstep to nothing at the frame's top, the span's
  width over the foot, at least 2 stops (`MIDTONES_ABOVE`);
- peak 0.5 of the slider (`MIDTONES_PEAK`): ±2 moves the middle ±1 stop.

In `shape`, shadows first, then midtones on where shadows put the region,
then highlights (and a merge's pull) on where midtones put it; whites and
blacks after, as before. The summed slider (picture plus masks) is held
to ±2 (`MIDTONES_LIMIT`). A gain on all three channels, so hue holds.
Global tone by region: no halo beyond the tone equalizer's own.

**Figures at +2** (stops of shift on a region; "foot" is the span's):

| frame | foot | median | grey | foot −1 | foot +1 | frame top |
|---|---|---|---|---|---|---|
| typical (median −0.9, top +1.9 at the default develop) | −0.90 | +1.00 | +0.76 | +0.50 | +0.71 | 0 |
| low-key interior (median −4.67, top +0.97) | −2.53 | 0 | +0.19 | +0.50 | +0.80 | 0 |
| high-key (median +0.4, top +1.2) | −0.40 | +0.65 | +0.90 | +0.50 | +0.50 | +0.10 |

On the typical frame, where Highlights peaks (1.2 stops over grey)
Midtones takes a sixth of its peak (+0.16 at +2); at the foot, where
Midtones is full, Highlights takes a fifth of its own. A slider of one
stop moves the middle half a stop and the ends nothing.

**How it differs.** Exposure moves every tone alike. Contrast is a power
about mid grey: it moves the two ends apart and holds grey. The
parametric curve's Lights and Darks act per pixel on the encoded,
display-referred picture at fixed quarters of the output, so they flatten
or steepen the texture inside the band they move and do not follow the
frame. Midtones moves the frame's middle, holds its ends, reads regions
(texture keeps its contrast), and is placed in scene stops by the frame's
range.

**Monotonic.** Each of the three region shifts on its own has a least
slope over zero at the slider's limit: shadows 0.14 (peak 0.86),
midtones 0.25 (peak 0.75: `1.5 * 2 * 0.5 / 2`), highlights 0.125 (peak
0.875). Composed one after another, the map's slope is their product,
never under 0.0044. The sweep over the eight corners of shadows,
midtones, and highlights at ±2, on spans from six stops under grey to six
over at four widths, finds a least slope of 0.0052 (all three at +2, the
least span, foot at −2, where each falls at once); without midtones it is
0.020. The existing sweeps now take Midtones at both limits on every
corner: 32 corners through the whole finish under both curves at two
display whites, 16 over every span, and 8 along a merge's pull. The
summed hold at ±2 keeps the proof true under masks.

**Schema.** No new version. `Tone::midtones` is `#[serde(default)]` and
skipped when zero, so an edit without it serializes byte for byte as
before (a test pins the tone's JSON), every existing sidecar, key, and
cache is untouched, and an older build reads a newer file with the slider
ignored. `DEVELOP_RECIPE` is unchanged: a zero changes nothing rendered.

**Where it reaches.** The panel row (between Highlights and Shadows,
its value in EV read as its neighbors'), its history step ("Midtones +0.50"), the
double-click reset, fine steps, presets and Sync (a part of its own,
`light.midtones`, and with the whole of Light), the copy sheet, the
dropper (`pick` through `shape_at`), the viewport shader (a `tone` vec4
on the uniforms and on each local), `reads_guide` (a set midtones reads
the plane), and a mask's look: the panel is the same LightSection, so a
mask's Light gets the row and the finish sums it by the mask's weight
as it does Highlights and Shadows. That is a mirror of the global work,
nothing more.

**Auto** leaves Midtones as it is, as it leaves Contrast: a fresh frame
has it at zero, Auto's exposure places the frame and Midtones is a
choice on top of that, and nothing Auto reads (the top for Whites and
Highlights, the bottom for Blacks) is moved by it. The Lightroom and
RawTherapee importers leave it at zero (nothing maps to it).

**Checked.** CPU against the shader on lavapipe: 0.5 of an 8-bit level
(the rounding) over three exposures, two contrasts, four midtones, two
mixed cases, and a mask summing past the limit; the random parity run
draws Midtones too and passes seeds 0 to 10.

**Limits and open questions.**
- On a low-key frame wider than 3.5 stops the bell sits 3.5 under the top,
  over the median, which it leaves to Shadows; on a high-key frame
  narrower than 1.6 stops the bell's foot is under the median, which
  takes about two thirds of the peak, and the frame's top a tenth.
  Whether the middle should be the median itself on such frames is a
  feel question.
- ±1 stop at the slider's limit. More would need a wider bell or a
  smaller margin.
- Measured against no Lightroom export; Lightroom has no Midtones slider.

**Also changed.** `panel::merge`'s test of the row's place compared two
window positions taken at two different panel scrolls; one more Light
row moved one of them. It compares in the panel's own terms now.

**A reset in culling.** A field left out of the file at its default
was invisible to the culling overlay when set back to that default:
`overlay_changes` walked only the keys the touched edit wrote, so a
double-click on Midtones over a frame with its own value left the
frame's value. Structure and Sharpen had the same hole, as did
Noise's chroma strength, the lens choice, and Tint's switch. A key
the edit before had and the touched one lacks is now taken out of the
frame's too. Every such field outside the arrays is one value, never
a section whose other fields the frame might hold apart; the two
sections skipped whole (`Look.white_balance`, `Look.detail`) live
only inside `adjustments`, an array, which the walk treats as a leaf.
