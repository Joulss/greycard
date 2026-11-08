# §614. Eight notes from a few minutes with 0.10.0

*2026-11-08*

A tester's second list on discussion 59, the day after 0.10.0. Two
are bugs, five are asks we take, and one waits on its context.

**The brush ring under Shift.** With a brush in hand, Shift held
turns a drag into a pan, and the viewport hides the brush's ring
whenever that hand is on, drag or no drag. Shift and a bracket set
the feather, so the ring hides exactly when the tester wants to
watch it. The ring hides only while a drag pans (`hand-down`), and
shows under Shift at rest.

**The wheel past its center.** Shift on a color wheel holds the hue
and reads the amount off the pointer's distance from the center, so
a drag in across the center climbs again on the far side. Under a
held hue the amount is the pointer's projection onto the hue's ray,
clamped at zero: in to the center and on past it is zero.

**Brackets size the brush.** `[` and `]` turn the frame a quarter.
Photoshop, Affinity, and Lightroom size the brush with them, and the
tester's hands know it. With a brush or a retouch patch in hand the
brackets size it by the wheel's step (1.1 a press); with nothing in
hand they turn the frame as before. Shift and Ctrl forms keep
feather and flow.

**Finer feather and flow steps.** Shift or Ctrl and a bracket step
the feather or the flow by a tenth. Half that: 0.05 a press.

**Keys that can be taken away.** Settings > Keys rebinds the `act`
lines; the turn pair and frame stepping are `shown` lines, fixed.
Both become actions, one a direction (`turn-left`, `turn-right`,
`previous-frame`, `next-frame`, with their Shift forms following as
the arrows' do today), shown on one line as before. A binding may
also be none: while an action listens, Backspace or Delete clears it
and the row reads "None". Frame stepping by arrow in the grid and the
lightbox is the selection's and stays; the rebinding is the Loupe's
step, which is what the tester's habit hits.

**A pick onto an empty wheel.** Ctrl and a click on the picture turn
the wheel in hand to the pixel's hue at the wheel's own amount, so on
a wheel at zero nothing visible happens. When the amount is zero the
pick also sets it from the pixel: its Oklab chroma over a reference
of 0.1, clamped to one. A wheel with an amount keeps it.

**Shift on a shape's handles.** Reported as holding only at
placement. The handle reads Shift at the press and holds a Linear to
angle steps and a Radial to a circle, as placing does. A test of the
report's shape, Shift on a corner handle of a placed Radial and on
an end of a Linear, decides whether there is a bug; a found one is
fixed here.

**Ctrl for the constraint.** Kept as Shift: the three editors above
constrain on Shift, and Ctrl is the wheel's dropper and the Mac's
menu click. With the keys rebindable it can follow later.

**Not changed.** Space and a drag not panning, intermittently: Space
pans only with a brush or a retouch tool in hand; elsewhere a plain
drag pans and Space is Fit or 1:1. Waits on which tool was in hand.

## What was built

**The rings, the wheel, the pick, and Shift.** The brush's ring and
the repair tool's drop `!root.hand` from their conditions: each hides
while a drag pans or the middle button pans, and shows under Shift or
Space at rest. `wheel::pick_along(x, y, hue)` is the pointer's
projection onto the hue's unit ray on `pick`'s scale, clamped to
0..1; `held_pick` under the hue hold reads it, so a drag in to the
center falls to zero and stays there past it. `finish::Picked`
carries `shown_chroma` beside `shown_hue`, the Oklab chroma of the
color as shown with the wheels and the tint out, as the hue already
is; `set_hand_hue` sets a wheel's zero amount to chroma over 0.1,
clamped, and keeps a nonzero one. Still one history step. We read the
shown color rather than the working-space pixel: the hue comes from
there, and scene-linear chroma grows with exposure.

Shift on a placed shape's handles had no bug: the handle's
press-time read reaches `MaskDrag.held`, and `dragged_held` makes the
circle or the step. What the tester met, we believe, is Shift pressed
mid-drag, which did nothing, since placing and handles alike read it
at the press only. Both now read it live: the viewport's
`pointer-event` sets `place-shift` from each move's modifiers while a
press places, and `CropHandle.shift` is set on every pointer event as
`alt` is and carried by `mask-dragged`. The press's read stays the
start; a Shift taken or let go acts on the next move.

*Verified.* UI tests with real key and pointer events: the brush's
ring and the repair's stay under Shift at rest, go during a Shift
drag that pans, and return after; Shift before the press on a placed
Radial's axis ends makes a circle and on a Linear's end a step, and
without Shift nothing is held; Shift pressed mid-drag on a handle and
while placing makes the circle on the next move and releasing lets it
go. Unit tests of `pick_along` at the center, past it, and off-axis.
A UI test of the pick: a saturated chroma onto a zero tint and a zero
grading wheel sets an amount, a grey leaves zero, 0.4 is kept, the
other wheels stay, one history row a pick; the GPU Ctrl-click test on
lavapipe checks the amount is the chroma over 0.1.

*Not verified.* None of it by hand in the running editor; whether
the tester pressed Shift mid-drag.
