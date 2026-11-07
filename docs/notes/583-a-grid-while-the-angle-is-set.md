# §583. A grid while the angle is set (2026-11-06)

The crop shows a rule of thirds, which is for composing, not leveling:
a horizon a third of the way down is the one case its lines help with.
The Angle slider and the level drawn by hand both want a reference the
eye can hold a horizon or a doorframe against anywhere on the picture.

While the Angle slider is being set, by a drag, the wheel, or the
arrow keys, or the level's line is being drawn, the viewport lays a
grid of screen-aligned lines 32 px apart over the picture, every
fourth brighter, and takes it off when the pointer lets go or a second
or two after the last step. In crop mode it covers the crop, where the thirds
already are; otherwise the whole view. Lightroom shows its grid the
same way while its Angle slider moves.

The slider's `dragging` flag was never cleared, only reset at the next
press, so the slider says `moving`: dragging and still pressed, or
stepped, which a timer lets go only after a whole second with no step
(a restart on each step did not take while it ran, and the grid
blinked between a slow wheel's notches). The flag travels as `angle-dragging` from the ROTATE section through
the window to the viewport, Slint only; the leveler's own `dragging`
drives the second grid. No wording.
