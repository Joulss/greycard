# §608. A picture's lens corrections start off (2026-11-07)

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

With sidecars off nothing is read, and the
frame used to stand in with `Sidecar::default()`, a raw's edit, even
for a picture: sharpened and lens-corrected again. The stand-in is
now the frame's own default (`on_disk::fresh_sidecar`, which the
sidecar read already used for a picture with none), so the editor
and `greycard export` with sidecars off start a picture from
`for_picture` too. The schema migration's "develops nothing" test,
which spares an untouched edit the baseline, also takes the picture
default as it was before (lens corrections on), so an old untouched
picture sidecar still counts as untouched there.

a picture's sidecar an older build wrote when it was only
rated holds that old default. It develops as it says, corrections
on, but nobody edited it, so it must not show as edited. The rule is
one list, `Edit::picture_defaults()` (`for_picture` and the same
with the lens profile's corrections on), which the migration, the
library's `is_default_edit` (the index's `edited` column, the strip's
mark through `develop_marks`, the reduced picture's edited test in
`edited_pictures::is_edited`, and Reset to Original's earliest
state), and the history's Original row all use. A raw holding that
edit is still edited: the rule is a picture's. The schema migration's own check
takes only the old default, as before, since no sidecar from before
it can hold the new one untouched. A picture whose user turns all
three corrections on and changes nothing else equals the old default
and so counts as untouched (no edited mark, its thumbnail the plain
file); we accept that, since without a marker the two cannot be told
apart, and an old untouched sidecar had the same mismatch before.

## Verified
- `Edit::for_picture` has distortion, vignetting, and chromatic
  aberration off, the section and its profile switch on, the lens
  from the file, nothing else apart from the raw's default but the
  sharpen; one correction switched on reads back through the JSON
  (greycard-edit unit test).
- The develop's stage list, with a lens database that matches the
  picture's lens: a fresh picture has no lens step; distortion
  switched on brings the profile's distortion alone; a picture edit
  from before (corrections on) still corrects; a raw's default still
  corrects (greycard-engine stages test).
- With sidecars off a picture starts from `for_picture`, a raw from
  the raw's default (greycard-edit-io test).
- A picture opened in the window with its lens found: the Lens
  section names the lens, is available, section and profile on, the
  three corrections off; Distortion switched on goes into the edit
  with the other two still off, and the sidecar written beside the
  file holds it (greycard-ui test).
- A picture's sidecar holding the old default, rated, is not edited
  in the index row nor by `develop_marks`; one with an exposure moved
  over the old default is; a raw holding the old picture default is
  edited (greycard-library index test).
- An untouched picture edit written with the corrections on still
  counts as developing nothing for the baseline migration
  (greycard-edit migration test).
- Every fresh picture edit goes through `for_picture`: the sidecar
  read (`on_disk::load`), the index's stand-in (`held::from_row`),
  sidecars off (`held::from_nothing`, now path-aware), reset to the
  original (`panel::reset`), a version from the defaults, the
  Lightroom catalog import, and the CLI's preset sidecar. `greycard
  develop` is unaffected: its lens correction is opt-in (`--lens`)
  for raws and pictures alike.

## Not verified
- No real JPEG or TIFF opened in the running editor, and no export
  compared before and after on a sample; the checks are the stage
  list and the panel with a stubbed open.
- A rated picture from before is shown unedited, so its thumbnail is
  the file as it is (not a develop), while opening it develops with
  its lens corrections on, as its sidecar says. Reasoned, not
  measured.
- Culling has no picture stand-in to check (a picture's camera
  preview is the picture; `cull.rs` falls back to nothing for one).
