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

## What was built

`Database::lenses_off_mount(camera)` in greycard-lens: straight-line
lenses on none of the body's mounts and not already in its list, one
a name, nearest the body's crop, sorted by name. `lens_named` looks
in the body's list first and, on a miss, makes one pass over the
lenses for the entry of the name nearest the body's sensor size, so
a name on the mount costs no off-mount list (6 to 125 µs on the full
database, from 1.3 ms) and a pick made off the mount holds when the
correction is built and when the sidecar is read back. `options_in`
in the engine puts the off-mount options after the body's and before
the Adobe profiles, labeled "<name> (<mount>, <mount>)" in the
database's order, the choice the bare name. `lens_rows` lists an
off-mount option only with words typed that match its name and maker,
never the mounts its label adds: "sony" on a Sony body would otherwise
bring in every lens calibrated on Sony A. "The makers of the lenses
shown" reads as: the body's makers and the profiles' always, an
off-mount maker only while the words list one of its lenses, and the
chosen lens's. The filter's callback refreshes the Maker row, keeping
the maker shown while it is listed, All when not.

**Measured** on the full lensfun database (1568 lens entries): on
the mount 269 (A7 IV), 432 (R7), 8 (GFX 100S), 494 (Z 6), the rest
off it; `lenses_off_mount` 1.0 to 2.1 ms, once per open.

**Verified.** Unit tests in the lens crate (a Sony E body: the Sigma
35 HSM off the mount, one entry at the full-frame calibration, found
by name; a 1.5 crop body takes the 1.5 entry in the list and by name;
a name on the mount still the mount's), the engine (labels with
mounts, bare choice, the pick corrects, no export warning), and the
UI (hidden with no words, listed after the body's with "35", Sigma on
the Maker row then, an off-mount lens on "Sony Alpha" not found by
"sony", the pick read back as the bare name, still listed and
available after the words are cleared, the Maker row back to All when
the words drop the Sigma it shows).

**Not verified.** In the running editor by hand; with a real adapted
file. On a medium-format body one typed letter can list about 1400
rows.
