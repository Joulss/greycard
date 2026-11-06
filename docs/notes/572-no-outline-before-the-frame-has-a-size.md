# §572. No outline before the frame has a size

*2026-11-05*

A straightened frame opened with a Linear shape chosen panicked in
the renderer's tessellator. Until a frame's first develop lands its
source has no size, and the crop's fit on a source of no size divides
nothing by nothing: the aspect and the keystone both come out NaN. An
unstraightened frame got away with it because the fit's snap to the
source's edges turns NaN into zero; a straightened one skips the
snap, so every handle reached the outline's path as NaN. Arrowing to
such a frame with Heal or Clone in hand sent a sourced patch's
handles the same way.

The overlays that draw through the view's mapping now wait for the
frame's size: the chosen shape's handles and outline, the chosen
patch's handles and outlines, an object's and the people's boxes,
and the guide's kept stroke. The shape's overlay is extracted from
the rendering notifier, which still calls it every frame. Should a
point off the plane reach the chosen shape some other way, its
outline sits that frame out, said in the log at debug.
