# §542. Focus peaking

*2026-11-04*

Shift+J paints the sharpest parts of the Loupe's picture over it, as
darktable's and Capture One's focus masks do (discussion 62). The map
is the focus stacker's, moved out of `stack` into
`greycard_core::sharpness` so both call one copy: the energy of the
Laplacian of the log luminance, box blurred. It is taken on the
viewport's own pixels after the draw, the way the clipping paint is
the draw's last step, so it follows the zoom: at Fit the downsampling
hides a missed focus that 1:1 shows. Each compare tile is its own
plane with its own threshold, so a noisy frame cannot set its
neighbor's, and the canvas outside the frame never enters the
measure. The canvas corners a turned or straightened frame shows in
crop mode lie inside the frame's rectangle, so their edge with the
picture measures as detail and paints.

**The overlays.** With peaking on the picture is drawn bare (no
clipping paint, no mask or sharpen red, no gamut grey), peaking
measures and paints on that, and the overlays alone are drawn over
the paint. Measured with the overlays in, the clipping paint's
speckle read as detail, painted over the clipping marks, and took
the threshold from the real focus. A mask on show is drawn from the
picture wherever it acts, so the green shows between its pixels and
not under the red.

**The measure.** The log is of the displayed luminance plus 2^-5:
a hard floor (the stacker's, or a fixed one) put a knee into a dark
ground and painted its grain, and the eight-bit steps of the deepest
shadows are a stop each. Smoothing is a 5 by 5 window.

**The threshold** is the frame's own, since the Laplacian grows with
the ISO and the sharpening: a pixel paints when its energy is within
3 octaves of the map's 99.5th percentile over the picture, both
counted in bins of the log of the energy (16 an octave). A fixed
quantile alone would paint the same share of every frame; this paints
the eyes and the coat of a portrait at f/1.2 and not the background
or the hair's blur, and most of the detail of a landscape at f/8
(3 to 27% of the nine samples tuned on at Fit). 4 octaves began to
take a dark ground's grain, and 2.5 dropped the near foliage. The
window measures the picture as edited, so heavy Texture or Clarity
raises the share: 40% at Fit and 53% at 1:1 on an f/10 landscape with
Texture +100 and Clarity +60.

**The color** is the theme's `peaking`, #3cff28 in every theme: a
lime green, apart from the clipping paint's blue and red, light
enough to read on a dark picture, saturated enough to read on a pale
one, and greener than foliage.

**On the device**, since the clipping paint is a shader pass: five
compute passes (the energy, the blur's rows and columns, the bins,
one a rectangle for the threshold) and a paint pass that discards what
does not peak, in `peaking.wgsl`, held to `greycard_viewport::peaking`,
which they match pixel for pixel on the RTX 5070 Ti and on lavapipe,
one view and two compare tiles alike. The paint pass reads its two
buffers as read-only storage, which every backend takes in a fragment
shader; the window never draws on GL.
Nothing is made until peaking is first on and nothing runs while it is
off; the develop is untouched. A 2560 by 1440 view of a 24 MP frame
(medians of 30 frames drawn and waited for, release):

| | plain | clipping paint | peaking | both |
|---|---|---|---|---|
| Fit | 0.29 ms | 0.28 ms | 0.74 ms | 1.00 ms |
| 1:1 | 0.34 ms | 0.35 ms | 1.03 ms | 1.33 ms |

The clipping paint is a branch in the draw and costs nothing; peaking
adds 0.45 to 0.7 ms a frame drawn, the map made again on each pan or
change, and an overlay on show adds the overlays' second draw,
0.3 ms. The CPU reference takes 90 ms at 2400 by 1600 on four
threads, which is why it is the reference and not the path.

The lightbox's tiles are Slint images, not the viewport's draw, so
peaking does not reach them.
