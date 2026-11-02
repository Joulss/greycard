# §528. Highlights reaches what a merge keeps

*2026-11-01*

§525 left a merge's Highlights at -100 barely touching the range kept
over the reference's clip (a rise of 0.0005 on the test ramp where
the exposure's -2 gives over 0.03). The note said the span hangs from
the frame's top, and it does: on a merge the guide plane reads the
kept range (the test merge's top region 5.9 stops over grey, display
white 3.3), and the span sits there. The limit is the ramp's depth.
It takes about 0.6 stops off a frame's top at -100, enough for a
frame whose top is its clip at display white and not for one whose
top is three stops over it.

**The pull.** On a frame with headroom (`Baseline::headroom`, the
merge's tag), the negative travel first pulls the region readings'
top toward display white, `share` of the way, where `share` is the
slider's negative travel over its limit. The ramp then acts as it
does on a frame whose top is where the pull put it: its span is taken
from the pulled median and top. At -100 the frame's top lands on
display white and the ramp takes its usual 0.6 stops, so the brightest
region the merge keeps lands where a clip at display white lands at
-100 on a frame with no headroom.

The pull is a compression in stops, from a knee to the top:
`g - share * excess * ramp(g - knee) / ramp(top - knee)`, with
`ramp(u) = u - 0.5 * (1 - e^(-u / 0.5))`, a soft knee whose slope
rises from nothing to one over about half a stop. The excess is how
far the frame's top reads over display white, at most the headroom at
the picture's contrast, so an exposure that has already brought the
top under white leaves nothing to pull. The knee sits 1.6 stops under
display white, the span's least. What reads further under white than
that never moves, the median with it unless it sits within 1.6 stops
of white; a high-key merge's median comes down with the rest. At -100
the ramp's span is the one a frame with its top at white has.

We first hung the knee from the median, at the foot of the span the
frame would have with its top at white. On a merge whose median sits
3.5 stops or more under white that put the knee just over mid grey,
and -100 halved the contrast of everything from there up. A knee at
the span's least leaves the reference's midtones and lower highlights
to the ramp alone. Two stops under white was the other candidate; it
spreads the excess wider and keeps less of the reference's
highlights (below).

Why this curve. The pulled reading's slope runs from one at the knee
down to `1 - share * excess / ramp(top - knee)`, which with the top
`excess` over white is never under `1.1 / (excess + 1.1)`: over a
fifth for a merge of four stops, 0.30 on the test merge (an excess of
2.6); an excess held to the headroom, its top further over, only
raises it. It never reaches zero, so the pull never turns the curve
back, and the ramp after it on a span of at least 1.6 stops cannot
either. Sweeps hold both, on the pixel's own luminance and on the
region reading, for merges of half a stop to six, medians from four
stops under grey to 2.5 over, contrasts from 0.6 to 2, and the corners
of shadows and whites; the least slope found is the bound's, 0.083 at
contrast 2 and six stops. It is in region readings, as the
ramp is, so texture inside a region keeps its contrast and only the
regions are brought together. And the share is linear in the slider,
so the top comes down a little at every step: no step of a fortieth
of the travel moves it by more than 0.02 encoded. The positive travel
is the ramp's alone.

The cost is the reference's brightest highlights. To fit 2.6 stops
over white into the 1.6 between the knee and white, the readings over
the knee are compressed to 0.3 at the top. On the test merge at -100,
four bands under the reference's clip (three, 1.8, one, and 0.2 stops
under) render 0.553, 0.701, 0.784, and 0.847, where the reference
renders 0.606, 0.752, 0.836, and 0.932 (and the knee hung from the
median gave 0.536, 0.622, 0.678, and 0.757). A knee two stops under
gives 0.553, 0.695, 0.758, and 0.821: worse near white, no better
under.

What is left under the knee is the ramp's, not the pull's. The pull
lands the merge's top on display white, 3.3 stops over grey, and the
ramp hangs from there; the reference's top, its clip on the test's
synthetic body, reads a stop over display white, so its ramp sits a
stop higher and reaches less deep. On a body whose clip lands at
display white the two agree.

**What does not change.** A frame without headroom takes no pull:
every raw, every picture, every DNG without our tag. The render's 114
tests pass unchanged, and two sample raws (a RAF and a CR3) exported
under exposure +0.3, highlights -100, shadows +50, and whites -50,
before and after: 0 pixels differ. Whites keeps its pivot
on the span as the frame has it, so it needs no change. On a merge it
still aims at display white, so its -100 puts the white point two
stops out, short of a three-stop headroom; the reach into the kept
range is Highlights' job and the exposure's.

The viewport's shader takes the merge's headroom in `frame_range.z`,
free until now, and does the same arithmetic; on lavapipe its worst
channel is half an 8-bit level from the CPU's over three exposures and
five highlights.

The dropper places the highlights by the frame's range as the render
does (`Guide::frame_range`, a typical frame's when there is no plane),
so on a merge it reads the pull the picture shows. It took a typical
frame's range before, on every frame, which on a merge meant no pull.
On an ordinary frame the change shows only once Highlights or Whites
is set, and brings the dropper closer to the render: on a dark RAF
under exposure +0.3, highlights -100, shadows +50, and whites -50 its
mean gap to the render falls from 14 to 9 of 255.

**Measured.** The test merge (§525's ramp, -12 to +4 stops over the
reference's clip, a headroom of 2.92), exported at the edit a merge
starts from with Highlights -100: five bands from the reference's
clip to four stops over it read 0.875, 0.914, 0.954, 0.975, and 0.986,
where before the top four read 0.99 to 1.0, and the reference reads
1.0 over its clip. A flat region at the frame's top lands at 0.961, a clip at
white at -100 at 0.963; the brightest band reads over that by its
texture (30% fine and patches of 0.7 to 1.3 over the region's
geometric mean, lifted in the shoulder). The test holds the band to
0.03 of the clip's landing and under white, and the climb to 0.01 a
band.
