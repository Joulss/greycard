# §555. The color mean at a fixed scale

*2026-11-05*

The pool line from §158: a Color shape's window and the mixer's hue
read a 5x5 mean of a and b at the picture's own pixels, so an export
made smaller took it over more of the scene than the viewport, and a
window's edge and a hue near a band's border moved between the two.

**The span.** The mean now spans a fixed part of the scene: 5x5
pixels of the full-size develop. `local_ab_at(image, reach)` takes the
picture's pixels to a full-size pixel; its box is `MEAN_RADIUS` and a
half full-size pixels each way, at least the pixel itself, and a tap
the box covers in part counts for that part. At a reach of one it is
the plain 5x5, to the bit, so the viewport (whose texture is always
the full-size develop, at Fit and at 1:1 alike) and the shader are
untouched. `finish_reaching` is `finish_with` with the reach.
`render_framed` takes the frame's width at the full-size develop
beside the width its image stands for, and passes the image's width
over it: for an export these are one, and for the pictures kept of an
edit made from the reduced develop (`edited_pictures::make`) they are
not, since that develop's source is half the full one, a third for
X-Trans. Measured against the reduced develop's own pixels the reach
there came out twice the right one (0.68 for 0.34 on a frame 6000
wide made 2048), the mean spanning about 10 full-size pixels, so
`make` passes the frame at `Finish::full`.

**Measured.** A color window with hard edges (195 degrees, 30 wide,
chroma from 0.03, no fades) on a 24 MP frame, the full-size weight
binned 2x2 against the weight on the picture fitted to half size: the
mean of the difference 0.0162 at the half-size pixels to 0.0068 at the
fixed scale, the pixels more than half out 1.16% to 0.03%. The maximum
is 1 both ways, a hard edge binned against one, which the resize
allows. The ignored `a_real_frames_color_window_at_half_size` runs it.
The reviewer's run on the same frame: at half size the mean 0.0171 to
0.0081, more than half out 1.53% to 0.33%; at 0.37 (1480x2220) 0.0234
to 0.0109 and 2.11% to 0.42%. The mean costs the same at either reach on
the full-size picture (0.278 s and 0.255 s over 24 MP), and a
half-size export's is no slower (0.065 s against 0.070 s).

**Tested.** A stripe six pixels wide on grey and the same picture
binned: at the fixed scale every half-size pixel's weight is the full
size's, and a 5x5 at the half-size pixels reads grey into the stripe
and drops its middle. The same stripe through `make` with the full
develop twice the reduced picture is brightened by the window, and
with the two the same size is not.

**Left.** No test of the editor's export against its viewport for
this; none exists for the §18 recipe in the ui bin.
