# §557. Constrained mask drags

*2026-11-05*

Shift at the press holds a Linear's angle to 15-degree steps and a
Radial to a circle, while placing it and while dragging a handle; a
center moves as plainly. The step is measured on the view: the anchor
and the pointer go to the screen, snap there, and come back, so a
horizon leveled by Straighten or a keystone takes a level gradient. A
placed radial's circle takes the larger of the two radii the drag
spans; an axis handle gives both radii its reach. The rule lives in
`Shape::dragged_held` and `mask::on_angle_step` in greycard-edit, the
step handed in by the caller.

Shift is read once, at the press, as the wheels read theirs: a
handle's `CropHandle.press-modifiers` goes with `mask-grabbed`, and a
placing press sets `place-shift` before `place-pressed`. Shift was the
hand while any tool was in hand; with a Linear or Radial being placed
it now holds the shape instead (the viewport's `shift-holds`, which
the app's hand reads), and Space still pans. The viewport test that
pinned Shift's pan there now checks that Space pans and Shift places
on a step. The history records the same steps as a plain drag.

Alt+Shift switches the keyboard layout on Windows and some Linux
setups, so a free-end drag held to steps may change the layout too.
