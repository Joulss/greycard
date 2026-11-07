# §590. The display curve goes with a copy

*2026-11-07*

A tester found (issue 82) that Copy and Paste settings carried every section but the display
curve, so a look copied from an AgX frame onto per-channel frames came
out different. The curve was left out on purpose: carried inside Light,
it would have turned AgX off on every picture a Light preset made
before the switch was laid on, since those files hold the default.

We make it a section of its own, `Section::DisplayCurve`
(`display-curve` in a file, "Display curve" on screen, the panel's word
for the switch), placed after Light in `Section::ALL` because the switch
sits at Light's foot. It has no parts, so the sheet shows it as one
check, as it does White balance and Tint. Light still leaves the curve
alone. The sheet's rows, the paste, the sync, Ctrl+C and Ctrl+V, and
the preset sheet all read `Section::ALL`, so the one variant is the
whole change; history records a paste as before, one step a frame
under the paste's label.

The curve goes only between raws. A picture (a JPEG, a TIFF) shows no
switch, yet a fitted look is looked up by the edit's curve, so AgX laid
on one could turn its look off with nothing on screen to set it back;
and a picture's own curve is the default, no choice anybody made, which
carried onto AgX raws would set them back to per channel.
`preset_fit::curve_fitted` takes the section out of what reaches or
leaves a picture, as a named camera profile is taken out of what
reaches another body. A sync, a paste, a preset click, a preset saved,
and a preset at import or at start all go through it. The sheets say
so: from a picture, the Display curve row opens unchecked, and the
status line counts the sections that went, not the rows checked.

It is on by default for a sync and a paste, and among a preset's
default sections. The preset sheet already unchecks a section at its
default, so a preset saved from a per-channel picture still leaves the
curve out. No schema moves: a preset names its sections, and a file
written before this one does not name the curve, so it reads as it
always did and leaves each picture's curve as it is. A build older than
this one refuses a preset that names the new section, as it refuses any
section it does not know; parts are passed over by name, sections are
not.
