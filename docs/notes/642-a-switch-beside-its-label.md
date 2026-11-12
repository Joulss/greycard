# §642. A switch beside its label (2026-11-11)

**Asked.** On a wide panel a `Toggle`'s label sat at the row's left and
its switch at the right edge, far apart ("As shot" the complaint). The
switch goes right beside the label instead.

**Decided.**

- `Toggle` lays out the label at the left and the switch a
  `Theme.space-2` gap after it. A new `switch-at-end` property (off by
  default) keeps the switch at the row's right edge with the label right
  up against it on its left.
- A label too long for the row is cut with an ellipsis (`overflow:
  elide`, as the panel's other labels do) at the row less the switch and
  the gap, so the switch never leaves the row, in either placement. A row
  only as wide as the switch is the switch alone.
- No label: the switch stays at the row's right edge, no gap, as before.
- A click flips the switch only on the label, the gap, or the switch;
  the empty run of the row does nothing, and the pointer cursor shows
  only over that target. Focus, the ring, Space, and Return are as they
  were.

| Use | File | Placement |
|---|---|---|
| As shot | panel/color.slint | at end (our call: the White balance rows read better so) |
| Follow global (a mask's white balance) | panel/color.slint | at end (same) |
| Gamut warning | panel/display.slint | beside |
| Replace existing looks | panel/match-sheet.slint | beside |
| Show mask, Invert (one row, half each) | panel/mask.slint | beside |
| Denoise, Auto radius, Auto threshold, Show mask (sharpen) | panel/edit.slint | beside |
| Embed profile | panel/deliver.slint | beside |
| Include the rejects folder | panel/archive-sheet.slint | beside |
| With subfolders | panel/folders.slint | beside |
| Portrait | panel/crop.slint | beside |
| Books, prints...; Write over the {n} frames... | panel/lightroom-sheet.slint | beside |
| Write over the {n} frames... | panel/rawtherapee-sheet.slint | beside |
| Move on after a rating, flag or label | panel/cull.slint | beside |
| Verify against the card | panel/import-sheet.slint | beside |
| Profile, Distortion, Chromatic aberration, Vignetting, Auto scale, Defringe | panel/lens.slint | beside |
| One section open at a time, Show the filter bar, Resize with the wheel, Write XMP sidecars, Treat a JPEG..., Lens corrections on new edits, Check for a new version at launch | panel/settings-sheet.slint | beside |

Every one of the 32 uses has a label; none sits in a section header
(those are `Switch`, a separate control this leaves alone), and none is
given a width as narrow as the switch.

**Limits.** Right-to-left is not handled (the app has none). `Toggle`
gains five `out` lengths (`label-x`, `label-width`, `switch-x`,
`target-x`, `target-width`) that only the tests read: a `slint!` bench in
the UI crate has no element debug info for `ElementHandle`.

At the default panel width three French labels are now cut: "Passer à
la suivante après une note, un marqueur ou une étiquette" (Cull),
"Afficher le masque" in its half row (Masks), and "Traiter un JPEG placé
à côté de son RAW comme une photo distincte" (Settings). No tooltip
gives the whole label, as none does for `EditSlider`'s cut label; before
this change they ran on under the switch.

A column of switches no longer lines up, each switch following its own
label: Lens's four stacked switches, Sharpen's three, Settings' Library
pair, and the Lightroom sheet's two.

The switch's `FocusScope` covers the click's target only, as the
`TouchArea` does: a click on the row's empty run neither flips the
switch nor takes the keys, so Space and Return there still reach the
window.
