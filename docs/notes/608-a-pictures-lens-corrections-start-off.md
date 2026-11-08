# §608. A picture's lens corrections start off (2026-11-08)

Discussion 90: a tester developed a NEF with distortion correction,
exported a TIFF, and opened the TIFF in greycard, where the lens
profile was applied a second time. §50 gave a picture the same lens
lookup as a raw, and the lookup finds the lens, so the correction is
on by default and the picture is corrected twice.

## The decision

A picture (JPEG, TIFF, PNG) opens with the Lens section's profile
corrections off: distortion, vignetting, and chromatic aberration.
The lens is still matched and the section still shows the match, so
one click turns a correction on, and a sidecar keeps whatever was
chosen. A raw's defaults do not change.

The reason is what a picture is. A camera's JPEG is corrected in the
camera on nearly every body of the last decade; an export from any
editor was corrected once already; and neither says so in metadata
anyone can rely on. Lightroom leaves profile corrections off for a
JPEG for the same reason. The exceptions, an old body's JPEG or a
scan, are the user's to switch on, and a preset carries the switch.

Our own exports already say what they applied (§51: the whole edit
in the XMP packet). The open does not read it yet; when a picture
carries a greycard edit, the Lens section could show the correction
as already applied rather than merely off. That is for later, since
with the default off the double correction no longer happens.

## The mechanism

`Edit::for_picture` (greycard-edit) sets the three corrections off
beside the capture sharpen it already turns off; the lens match and
the section's display are unchanged. The command line's `develop`
and `export` of a picture follow the same default, since they start
from the same fresh edit. Existing sidecars are not touched: a
picture edited before this build keeps its corrections on, as its
sidecar says.

## Verified
(To be filled from the build.)
