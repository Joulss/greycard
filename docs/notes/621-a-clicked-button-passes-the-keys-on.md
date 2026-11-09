# §621. A clicked button passes the keys on (2026-11-09)

**Undo should cross a tool's reset** (discussion 59, 0.11.0). The history
recorded the reset; the keys never reached it. A `PanelButton` takes
the focus on a click and keeps only Space and Return. The window's
keys sit beside the panel, not over it, so every key the button turned
away went nowhere: after a click on Reset, Ctrl+Z, Escape, and every
other shortcut did nothing until the pointer crossed the picture. §550
mended four crop buttons one at a time. `Switch`, `Toggle`, and a
mask shape's invert had the same fault.

**We mend our own controls together, as the sliders already do.** A new global,
`ButtonKeys`, takes a key a button, switch, or toggle turns away,
except Tab and Backtab, and the window hands it to its own keys, as it
does a slider's `SliderKeys.passed`. Space and Return still press the
control, and Tab still moves on.

**Not under a sheet whose card has keys of its own** (Escape, Return):
there the control turns the key away as before, and the card takes it.
The window keeps `ButtonKeys.sheet-up` for that: any sheet up but the
import, Lightroom, RawTherapee, match, and model sheets, which have no
key handler on their card. The window's keys serve those five, so a
click on one of their buttons, switches, or toggles now passes its keys
to the window, and Escape closes the sheet. Named by the five, a sheet
added later keeps its keys.

That opened a way to Ctrl+E and Ctrl+Shift+E under those sheets,
which only checked that the export's own sheet was down: the export's
sheet over the match sheet, or a set sent behind it. Both now do
nothing under any sheet, as Ctrl+Z has since §156. The window's keys
already reached them under the five with no control clicked. Ctrl+O
stays a key the window answers whatever is open.

**A model's offer at an export's end** could come up over Settings, and
`sheet-up` would then read false; `offer_model` now drops its offer
under any sheet, as the lens profiles' offer does, and the next need
asks again.

**The std `ComboBox`es** (the crop's Aspect, the lens panel's Maker and
Lens) keep the same fault, the keys they turn away going nowhere; they
are left for later.

A passed key's release stays with the control, which ignores it. The
window's release handling is Space's hand, and Space is the control's
own. A held key's repeats go on with their `repeat` flag, as a
slider's do.

## Tests

The crop's Reset all clicked with the pointer on the panel, then
Ctrl+Z: the angle is back (it failed before). Flip clicked: Return and
Space press it, Tab is not passed, and Escape leaves the crop. Under
the Settings sheet, a clicked button keeps G from the window and
Escape is the sheet's. On the import sheet, Escape after a click on
Choose closes it. On the match sheet, after a click on its toggle,
neither Ctrl+E nor Ctrl+Shift+E opens the export's sheet; each did
before the guard.
