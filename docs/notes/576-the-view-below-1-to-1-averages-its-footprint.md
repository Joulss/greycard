# §576. The view below 1:1 averages its footprint

*2026-11-05*

The viewport aliased foliage and branches at Fit and at any zoom under
1:1, where the export of the same frame was clean. The shader took
each target pixel to one source position and read one bilinear tap
there (or the cubic, for a turn between pixels). At Fit on a 24 MP
frame a target pixel covers about four by four source pixels and saw
the four nearest its center. The range masks' picture, the sharpen's
delta, and the Detail planes were read the same way, all at the
source's size.

## The taps

Below 1:1 the shader now averages `n` by `n` bilinear taps, one at the
middle of each of as many equal cells of the target pixel, each taken
to the source through the view's own map (perspective, turn, and
texture turn), so a turned or leveled frame averages the right
footprint. `n` is ceil(1 / zoom), at most four; the window computes it
(`minify::taps_at_most`) and hands it over in `Params::minify`, so the
cap lives in one place. At 1:1 and above it is one, and the shader
takes the path it always took. Measured once: frames at 1, 1.37, 2,
and 3, straight, turned, and cubic, read back on lavapipe byte for
byte as before the change.

The picture, the range masks' picture, the sharpen's delta, and the
four Detail bands are averaged alike. The planes are read through the
sampler when averaged, not `planes_at`: its exact weights are for one
tap seen magnified, where a stop's step from one pixel to the next
would band, and are lost in a mean of sixteen; at four loads a tap
they cost 25 ms of a lavapipe frame. The masks are not averaged: each
is read once, at the pixel's center, on the averaged picture. That is
what the export does, a mask once a pixel of its resized picture, so a
hard luminosity window at Fit takes the edge the export takes; a mask
averaged over the footprint would be a softer edge than the export's.
The local mean the mixer and the color windows read is still a 5x5 of
source pixels about the center, where the export's is 5x5 of its own,
larger pixels; that was so before, and it moves a hue window's edge,
not detail.

`minify::footprint_mean` is the reference, test-only: the CPU's
bilinear, the same taps, the shader's map with its perspective divisor
and texture turn. Against it, drawn at 1:1 with a per-pixel look, the
shader at 0.3 is within a level, 0.01 on average, turned, and in
perspective with the texture a quarter and three quarters behind; one
tap is 13 levels off on average. At 1:1 and 2 the shader is the
reference's single tap at the center, the same bytes straight at 1:1
and, turned, within a level on lavapipe; on the GeForce a turned tap
lands up to 11 levels off where two contrasting texels meet, 0.07 on
average, the sampler's 8-bit sub-texel weights against the CPU's
float, and the test's bound there is the sampler's. A one-pixel checkerboard at a quarter, the
view a quarter pixel off its corners so no tap reads a 2x2 mean by
itself, draws as its flat mean to the level, and so does a
checkerboard of the sharpen's delta, which averages to none.

## The cap

A perfect resize shows a grating past the view's limit flat, so the
swing left of one is aliasing, with the box's own leak just past the
limit, which no count of taps removes. Gratings from the view's limit
to the source's, four phases each, worst swing left as a share of the
grating's (`what_the_cap_costs`):

| zoom | taps | to twice the limit | past it |
|---|---|---|---|
| 0.242 (45 MP, 2000 wide) | 1 | 0.996 | 0.995 |
| | 3 | 0.636 | 0.389 |
| | 4 (the cap) | 0.639 | 0.249 |
| | 5 (uncapped) | 0.615 | 0.211 |
| | 8 | 0.608 | 0.166 |
| 0.125 (a compare tile) | 4 (the cap) | 0.631 | 0.229 |
| | 8 (uncapped) | 0.631 | 0.229 |

At the 45 MP fit four taps fall just over a source pixel apart and
leave 0.04 of a grating's swing more than five do, at the worst
frequency; real foliage spreads over many and shows less. At an eighth
four taps land between source pixels two apart and each reads a 2x2,
so the cap is the uncapped box. Between whole ratios the cap leaves
gaps between taps and some aliasing back, never more than one tap's.

## The cost

A 2000 by 1333 view, lavapipe on the Ryzen 9 9950X3D, best of five,
read back included (`a_masks_detail_timed_on_the_gpu`), on a quiet
machine:

| frame, zoom | before | after | a mask's Detail, before | after |
|---|---|---|---|---|
| 24 MP, 0.333 (3 taps) | 13.3 ms | 24.0 ms | 18.3 ms | 50.0 ms |
| 45 MP, 0.244 (4 taps) | 13.4 ms | 24.5 ms | 18.8 ms | 49.7 ms |

Paired again later on a busier machine, one tap against four for the
45 MP frame: 15.3 against 27.6 ms, and 21.5 against 56.2 with the
Detail. The planes read through `planes_at` had cost 74.5 ms there.
Five taps a side cost the 45 MP frame 30 ms against four's 24.5. Not
measured on a discrete card, where sixteen taps a pixel of a 2.7 MP
view should be small.

## Against the export

The export resizes the linear picture by Lanczos3 before the finish,
and the sharpen's delta and the Detail planes with it, then runs the
finish once a pixel. The view now does the same in that order: linear
mean, then exposure, tone, look, and curves on it. The filter differs,
a box of bilinear taps against Lanczos3, measured on the CPU's
reference against the export's `fit_exact`
(`the_taps_against_the_exports_resize`):

| zoom | grating at half the view's limit kept, taps / Lanczos3 | fine noise, levels apart | detail, taps / Lanczos3 |
|---|---|---|---|
| 0.25 | 75% / 84% | 2.63 | 7.12 / 5.73 |
| 0.33 | 77% / 87% | 2.32 | 7.70 / 7.56 |

Detail is the RMS of each pixel about its four neighbors' mean, in
encoded levels. So the view is a little softer than the export below
its limit, a tenth less of a grating there, and on noise past its
limit it keeps more pixel-to-pixel variation: the box leaks what
Lanczos3 cuts, at a quarter a fifth more. Lanczos3's ringing on a hard
edge does not show in the view.

## What else changes at Fit

- The clipping warnings and the scopes read the averaged picture, so an
  isolated clipped speck no longer shows at Fit, as it would not in an
  export of that size. The scopes' analysis draw is itself a view under
  1:1, so its histogram is of the averaged picture.
- The camera's JPEG in the culling loupe is averaged too, in its
  encoded values.

## Limits

- Past the cap's reach (under 0.25 and between whole ratios) the taps
  leave gaps; a mip chain of the source would close them, at a
  pyramid's memory and a build per develop.
- Under a strong perspective the stretched side's footprint is wider
  than 1 / zoom, and the taps, spread by the zoom, leave gaps there as
  past the cap.
- Taps near the crop's edge read the source past it, by (n − 1) / 2n /
  zoom and the bilinear's pixel: a few source pixels, about 2.5 at a
  quarter.
