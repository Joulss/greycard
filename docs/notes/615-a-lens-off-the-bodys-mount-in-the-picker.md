# §615. A lens off the body's mount, in the picker

*2026-11-08*

A reader with a Sigma Art on a mirrorless body could not find its
profile. The database has the lens on the mounts its calibrators had
(Canon EF, Nikon F, Sony A, Pentax K, Sigma SA), not on Sony E or
L-Mount, and the lens on an adapter is the same glass.

**What already works.** The automatic match (`Database::lens`) looks
among the body's mounts first and, finding nothing of the name, tries
the name across every mount. A file whose EXIF names the lens is
corrected today, on any body.

**What does not.** The picker (`lenses_for`, `lens_named`, and
`options_in` in the engine) offers the body's mounts alone, so an
adapter that passes no lens name leaves the user with a list the lens
is not on, and a name picked off the mount is refused when the
correction is built.

**The change.** The body's own lenses stay first and alone until the
user types in the words box. With words typed, lenses on other mounts
that match them join the list after the body's, each labeled with its
mounts after the name, as "Sigma 35mm f/1.4 DG HSM | A (Canon EF,
Nikon F)", so a reader knows it was calibrated elsewhere. One entry a
name, the calibration nearest the body's sensor size, as the body's
list already does. The Maker row lists the makers of the lenses shown.
`lens_named` accepts any straight-line lens of the name when the body
has none on its mounts, nearest sensor size first, so a pick made off
the mount holds when the correction is built and when the sidecar is
read back. A lens with a profile made at full frame read on a crop or
medium-format body is already how an off-mount pick on the automatic
path behaves (`Profile::correction` scales by the sensor), and the
same holds here. No new UI wording: the mounts in the label are the
database's names.

**Not changed.** Lenses the database lacks, such as Sigma's DG DN
line, stay absent; Adobe's profiles through the profile folder are
the route for those, as the guide says.
