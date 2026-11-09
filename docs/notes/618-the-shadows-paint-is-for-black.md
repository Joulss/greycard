# §618. The shadows' paint is for black (2026-11-09)

Reported from use: with the shadows' warning on, blue over parts of
the picture that are plainly not clipped.

**The cause.** §41 painted a pixel blue when any channel of the
encoded output was at level 0. A channel reaches zero two ways. One
is the shadow the warning is for. The other is a color outside the
output space: the output matrix leaves it with a negative channel,
and the clamp, or under AgX the output rail (`agx_compensate`, which
takes the lowest channel off all three), leaves that channel at
exactly zero, whatever the pixel's brightness. Dim warm light is the
common case, a church's lit stone or a lamp-lit wall, where the blue
channel sits under zero across the frame; under AgX's rail more so
than under the clamp.

**The rule.** Blue where a channel is at zero and the brightest is no
higher than level 2: the pixel is black, and a channel of it has run
out. We tried all three at zero first, and the review showed what it
loses: a black wall is levels like (0, 1, 2) and (1, 0, 0), so the
noise chose which of its pixels were painted, a seventh of them on
one frame. Two levels is under anything seen as other than black,
and above it every false positive we measured was gone.

**The highlights** keep §41's rule, any channel at 255. The same
thing happens there on a smaller scale: sRGB's matrix takes a
saturated color of the working space past one in a channel, so a
candle's orange halo is painted where the tone curve alone would not
have clipped it (an eighth of the red on two frames, measured against
a Rec.2020 output). We leave it: that channel is flat in the file
whichever of the two put it there, it sits beside what is blown, and
one channel blown in a petal or a sky is what the red is asked for.

**The lamps** are as they were, per channel, since they say what the
exported file holds. The left one can now be lit with no blue to
paint, and the guide says so.

**Checked** by `the_shadows_paint_is_for_black_and_not_a_color_out_of_gamut`:
ramps of a working-space green, a grey, and a red from nothing up,
under both curves, painted where the bare picture says black and
nowhere else, on lavapipe, RADV, and an RTX 5070 Ti; it fails on
§41's rule, on all three at zero, and on a level either side of two.
And on fifteen frames at defaults, the paint's share of the fit
view, §41's rule and this one: the three worst under AgX went from
35.6, 20.3, and 5.8 percent to 9.2, 11.6, and 5.0, what is left
being black to the eye; under the per-channel curve from 17.5, 9.7,
and 2.8 percent to 2.9, 0, and 1.2.
