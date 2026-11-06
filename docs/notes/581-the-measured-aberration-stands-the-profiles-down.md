# §581. The measured aberration stands the profile's down (2026-11-06)

A tester (issue 77) turned on the LENS section's Chromatic aberration
switch on a Nikon D90 frame and got more fringing, not less: red and
cyan edges a pixel wide where there had been none.

Two corrections were running on one aberration. Every mosaic raw gets
the automatic lateral correction on the mosaic (§39's port of
RawTherapee's), which measures where red and blue landed on this
frame and brings them onto the green; there is no switch for it. The
profile's `tca` model, on by default since the LENS section came,
then moved them again by the calibration's amount, the other way.
Measured on the tester's frame at the long edges, red against green:

| develop | red |
|---|---|
| the automatic correction alone | within 0.1 px |
| both (the report) | 1.3 px, inward |
| the profile alone | 0.7 px, inward |
| neither | 1.3 px, outward |

The profile alone overshoots this copy of the lens too: it was made on
another body and another copy. The measurement is of the frame at
hand, and darktable and RawTherapee both tell their users to run one
of the two, never both.

**The rule.** The aberration measured on the mosaic stands the
profile's down, as a DNG's warps that move each color already do
(§513): `Lens::over_measured_ca` beside `over_file`, applied in the
walk, the panorama's reference, and the CLI's develop, whoever chose
the lens, since the measurement runs either way. The aberration set
by hand (§45) is kept: it is the user's word over both. An HDR merge
of ours measured its frames' aberration on their mosaics before it
wrote its linear DNG, and its tag's recipe says so (`ca` other than
`off`; a file from before the recipe ran the fit once): the review
caught that such a file got both too, and `Tag::ca_measured` carries
the measurement to its open. On any other linear raw, a JPEG, or a
TIFF nothing is measured, and the profile's runs as before. Old
sidecars with the switch on are corrected by the rule alone.

Left as it is: a panorama merged again from its saved geometry takes
the lens recorded at the first merge, the profile's aberration
included if it was merged before this; a fresh merge measures.

**The switch.** It is greyed on a frame whose aberration is measured,
as Distortion is greyed without a profile: `lens-ca-measured`, set
on each open from the frame's facts. No wording changes. The CLI's
`--lens` on a mosaic raw now corrects the aberration once; `--no-ca`
hands it back to the profile.

Tests: the edit crate's rule drops the profile's aberration alone, for
a lens picked by hand too, and keeps the manual one; the walk's lens
stage has no aberration where the CA stage runs, and has it on a
linear raw.
