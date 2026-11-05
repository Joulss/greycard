# §568. A mask's switch, without its overlay

*2026-11-04*

Hovering an adjustment's row lights its mask on the picture, the
whole row, its switch and bin included (§552). So there was no way
to flip a mask off and on and watch what it does: the pointer on the
switch always had the red overlay up over the very pixels the toggle
changes.

The switch now takes the overlay down while the pointer is on it and
the row puts it back when the pointer moves off the switch, whichever
hover changes first. The bin keeps the row's lighting; it is the
chosen row's and deletes rather than compares.
