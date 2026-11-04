# §546. The wheels' held drags and the pick from the picture

*2026-11-04*

Asked in discussion 59. In every wheel (Color Grading's three and
Tint's ring, both on `CurveEditor`), Shift at the press keeps the hue
for the drag and moves only the strength; Ctrl keeps the strength and
moves only the hue. `CurveEditor` reports the keys of its last press
(`press-modifiers`), the wheel passes a hold of 0, 1, or 2 with its
press, and `color::held_pick` reads the pointer through it from the
wheel's values at the press. A plain drag is unchanged.

The wheel last pressed is in hand (`wheel-hand`: 0 to 2 the grading
wheels, 3 the tint), its accent ring at double weight, grown inward
within the wheel's bounds so nothing clips it and the layout does not
move; the tint's ring, which has no chosen mark, takes the same. A
Ctrl-click on the picture then sets its hue to the color under the
pointer, Ctrl+Shift to the complement, at its own strength, and
nothing else: the press is held as a dropper's, so no zoom or pan. It
samples as the Color shape's dropper does (`sample_view_at`, its
Sample size) and reads the hue through `wheel_pick_hue`: the droppers'
stages with the grading and the tint out, the rest of the look in, and
`finish::Picked` gained `shown_hue`, the Oklab hue after the color
curves, the wheels' own hue circle. One change, one step, named as a
wheel's.

The wheel is put down by Esc (which goes on to mean what else it
does), by an edit put on the panel (`show_edit`: another frame,
target, undo), and by a control of another section. For the last we
keep the panel's edit at the press with the wheel's section left out
(`without_wheel`: the grading, or the tint, global and every mask's)
and compare at each `schedule_save`, where every view and develop
change lands; only while a wheel is in hand.

The wheels are out of the reading because a pick is for a cast, to
push it or its complement: once the wheel in hand pushes, a second
pick on the same spot must still answer the cast's hue, not the pushed
one, or the pick chases itself.
