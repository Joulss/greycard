# §634. As shot off gives back the white it replaced (2026-11-11)

Discussion 59 asked that switching As shot off restore the white balance
it had replaced, not leave the camera's. We agreed: the switch is how a
user compares the camera's white with their own, and before this the
comparison cost an undo.

The rule. As shot switched on by hand remembers the custom temperature
and tint on the sliders (a raw's kelvin and Duv, a picture's mired and
Duv offset) for the open frame. Switched off by hand, it gives them back,
one history step, as the switch on was. With nothing remembered (the
frame opened at As shot), off leaves the camera's white as before.

Anything else that sets the white forgets it: a slider, Auto, or the
Neutral dropper (each a new choice, turning As shot off as before),
any state made current (undo, redo, a history row, a snapshot, a reset,
a preset, a paste, or a sync), leaving Cull (where any of those may
have moved the edit unseen), and another frame opened. So an undo of
"on" shows the custom white with nothing held, and a redo of "on" gives
back nothing: every sequence ends on a state that existed.

The memory is window state (`Develop::before_as_shot`), keyed by the
frame's version key, one slot: a version never gives back another's,
and none outlives the frame's stay on screen or the session. It is
never in the edit: the sidecar gains no field, and copy, sync, and presets cannot carry it.
A mask's white balance has Follow global, not As shot, and is untouched.

Tested: off with nothing held and with a custom white, one step each
way; the undo and redo orders; a slider, Auto, Neutral, and another
frame each forgetting; a preset applied in Cull forgetting; a
picture's offset given back. Moving another section's slider while As
shot is on keeps the memory. A sidecar read that shows As shot without
making a state current is not forgotten by name; the version key and
the next custom change cover it, untested. Nothing was tried in a real
window.
