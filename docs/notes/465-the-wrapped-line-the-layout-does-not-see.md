# §465. The wrapped line the layout does not see

*2026-10-31*

§461 sized the Import sheet from its wrapped notes and said the
rule: a layout's preferred height is read with no width to wrap at.
Applied to the other eight sheets with a guard test each, the rule
turned out narrower. A wrapped Text set directly in the measured
layout measures right, as does a nested one with a width of its
own: the fork's height-for-width pass hands one cross-axis width
into the layouts it solves, and a Text with its own width or height
never asks. What measures short is a wrapped Text nested one level
down, in a row or a column of its own, inside a layout with no
width, with no width or height of its own: it reports the height of
one line.

Three sheets had that shape. The Keys sheet's wrapped rows left its
last row past the pane in every window height, in 0.9.0 and 0.9.1.
The match sheet's group list, with six or more groups wrapped to
two lines, cut the last group where the wheel could not bring it in;
its room of forty pixels a group, put there for the list measured
short, goes with the fix, so a short list no longer pads itself. The
Lightroom sheet's dry-run step was short only with longer notes, a
narrower window, or a larger scale, so it gets no changelog line.
The other five measured right under the guard tests, which are not
kept: Settings gives its categories a width, RawTherapee's totals
have theirs, and the rest wrap nothing nested.

The tests share three helpers in the window's testing module: settle
the window, bring a line in by the wheel, and keep the last line in
a tall window then a short one. An element clipped wholly out of
sight is not found at all, which the helpers read as out of sight.
The counts are set past the smaller line heights of Windows and
Mac, where a short window that fit every line would leave the wheel
nothing to prove, as the Import test's did once.
