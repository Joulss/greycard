# §579. A double-click inside the crop is Done (2026-11-06)

A tester (issue 74) asked for Lightroom Classic's gesture: with the
crop adjusted, a double-click inside it confirms the crop, so the
pointer need not go back to the CROP section's button. The button
stays.

The crop's body is its own `CropHandle` over the picture, so the
double-click lands there and never on the picture area under it,
whose click would zoom. The handle raises `crop-done`, and the window
answers it as it answers the button, through `crop-in-or-out`, only
while the crop mode is on; the two presses before it are a grab and a
drop with no drag, which is what a single click on the body has
always been. With Draw in hand the body is disabled, so the gesture
does nothing there, as a drag there draws a fresh crop.
