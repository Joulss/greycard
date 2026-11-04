# §552. A mask or a retouch lit while its row is hovered

*2026-11-05*

Discussion 59: with many masks or repairs, the one to fix is hard
to find by name. Now the pointer on a mask's row paints that mask
in Show mask's red, whatever Show mask is set to, and leaving the
row puts back what was shown; on a repair's row, its outline is
drawn as the chosen one's is, beside it.

The rows say which is under them in `mask-lit` and `patch-lit`,
set by each row's hover and cleared by the row it was on, so a move
from one row to the next never leaves none lit whatever the order
of the two events; a list built again (another photo, a delete, the
tab shown again) clears them. The view's `show_mask` is the lit mask
before the chosen one (`mask::shown_mask`), only on the Masks tab.
The weight is the one the shader already computes for every mask,
switched off or not, so a hover asks for no develop. Each row's switch
and bin sit inside its hover area, so the row stays lit over them,
and its hover background with it. The lit patch's rings are kept
beside the chosen one's (`lit_shape`), so the two never contour
each other's out. No history step, no setting; the chosen mask,
the chosen patch, and Show mask are untouched.

Tests: a mask row's hover paints its disc red and the other's not,
on a GPU, and leaving restores; with Show mask on, another row
lights its mask, the chosen one's row its own, and leaving the
chosen; a repair row's hover adds exactly that patch's outline to
the chosen one's, and leaving takes it off.
