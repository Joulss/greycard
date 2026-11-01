# §497. A lens picked by hand, and Adobe lens profiles

*2026-11-01*

Asked twice on discussion 59: a CR3 whose lens the file does not name
gets no correction, and a profile made with Adobe's generator had
nowhere to go.

**The picker.** LENS gains a Lens row, shown always: From the file
first, then the database's rectilinear lenses for the body's mounts,
then the Adobe profiles found at the top of the profile folder beside
the DCPs, named by the lens inside each. The edit keeps the choice and
writes it only when it is not From the file, so old sidecars and their
develop keys are unchanged; a chosen profile's contents join the key.
The choice travels with the profile's part in copy, paste, and
presets. A lens the body or the machine does not have shows greyed
and corrects nothing; when the body is known, a name is matched
strictly. The command line takes `--lens-pick NAME|FILE.lcp`, and
both it and the engine go through one `picked_correction`, so they
correct alike by construction. An export whose picked lens cannot be
had says so and exits as a missing look does. Two profiles for one
lens fold to one row; a database lens and a profile of the same name
show as two rows alike, since telling them apart would want words.

**Adobe's model, mapped.** The LCP's rectilinear model is read in
both XMP forms, each sample by focal length, focus distance, and
aperture, and put into the correction the lensfun path fills: the
distortion as the `acm` model with each sample's units folded into its
coefficients (k₁u², k₂u⁴, k₃u⁶, t₁u, t₂u, u the inverse of twice the
aspect times the focal length), the vignetting as the `pa` model over
the half diagonal, the chromatic models fitted least-squares to our
radial v + c·r + b·r², exact for k₁ alone and within a tenth of a
pixel with all three. Samples interpolate on log focal length,
diopters, and stops, the lensfun path's rule, since Adobe publishes
none. The center is read as fractions of width and height, as
RawTherapee reads it against the specification's text. The body's
sensor size comes from the database when it knows the body; a profile
made on another body is otherwise used only when its make and model
match, else no correction and one warning, since a full-frame profile
on an APS-C body would correct half again too much. On review the
mapping was exact for a centered profile to a thousandth of a pixel.

**Turned with the frame.** The lens stage runs after the orientation,
so the tangential terms acted on the wrong axes on a portrait frame,
28 pixels at worst with a real profile's terms; the correction is now
turned with the frame in core, in the stage list, the worker, the
reduced develop, and the command line alike, which also mends
lensfun's `acm` there. Not mapped: an off-center optical center,
warned once when more than a pixel off; a vertical focal length that
differs from the horizontal; the green channel's own distortion; the
chromatic models' tangential terms; fisheye entries; and a non-raw
picture's own orientation tag, unknown at the lens step.
