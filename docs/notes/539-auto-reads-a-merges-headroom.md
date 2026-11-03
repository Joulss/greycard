# §539. Auto reads a merge's headroom

*2026-11-03*

A merge (§525) is stored under its reference's scale by its headroom and
the develop brightens it by as much. Auto read its histogram before
that, against a display white placed for a frame without one, so it saw
the test merge about three stops dark: median -4.23 stops where the
reference reads -1.31, Highlights -0.39 and Whites -0.08 where the
reference gets -0.9 and -1.02. The kept range stayed at white (the
export's five bands from the reference's clip to four stops over it read
0.992 to 1.000).

**The read.** Auto now takes the merge's headroom
(`Source::auto_headroom`: `Source::headroom` and the guide's
`frame_range`, what the finish places the pull by) and reads the frame
with it on, as the develop shows it. That alone puts the middle where
the reference's is, but the kept range then reads up to three stops over
white, and Whites chases it to its limit and takes the rest off the
exposure (-0.94, Whites -2 on the test merge).

**The kept range is Highlights'.** §528 made bringing it down
Highlights' job, by its pull. So where Auto reads the frame's top for
Whites, it reads it through the pull at the Highlights it has just set;
where the cut asks what the top wants past Highlights' limit, at that
limit. The finish pulls a region and moves its pixels with it, so Auto
pulls the quantile's region (its reading, held to the frame's top) and
keeps what the pixel reads over it. Highlights itself reads the frame as
the default shows it, as on any frame. The pull moves into greycard-edit
(`kept`), one copy the finish's `Kept` and Auto both call, the shader
its only twin; Auto reads it at contrast one, as it reads no contrast
anywhere, and a render test holds the two to 1e-4 over merges of half a
stop to six.

**Measured,** the test merge against its reference, each read as the
window reads it: exposure -0.13 against 0, Highlights -1.0 against -0.9,
Whites -1.56 against -1.02. Whites goes further because the merge's top
reads two stops over the reference's reconstructed clip; it holds that
top 0.1 stops under white, as it does the reference's. Exported at the
merge's Auto, the five bands read 0.879, 0.920, 0.959, 0.980, and 0.989:
the kept range climbs under white. The test holds the exposure to a
quarter stop of the reference's, Highlights to 0.15, Whites to three
quarters and off its limit, the climb, and the top band under 0.999.

**What does not change.** A frame with no headroom takes no shift and no
pull. Two sample raws (a CR3 and a RAF) read through the same path
before and after: the five sliders equal to the bit.


