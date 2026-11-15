# §680. A profile's own tone curve, behind a switch (2026-11-14)

Issue 135. A DCP may carry a `ProfileToneCurve`; §639 read it and
applied nothing. Now a switch under the CAMERA PROFILE list, **Profile
tone curve**, shown only when the chosen profile carries one and off by
default, renders the picture with that curve in the display curve's
place. Off, every pixel written is what it was (the three test frames'
exports are pixel-identical to master's).

## The method, from the specification

The DNG specification (1.7.1) defines `ProfileToneCurve` as points over
0..1 that readers should interpolate with a cubic spline, and places it
in its processing model in linear ProPhoto: after the hue map, after an
exposure of `BaselineExposure + BaselineExposureOffset` and the user's
own (each channel clipped at one), after the profile's look table.
`BaselineExposure` is the file's (a DNG's IFD0 tag), never the
profile's: a DCP carries only `BaselineExposureOffset`, which adds to
it. The curve goes not per channel and not on luminance but on the
largest and the smallest channel, the middle one placed between the two
results as it sat between them: a neutral is the curve, and a color
keeps its hexcone hue and gains the saturation a per-channel curve
gives.

## No SDK code

An earlier draft of this ported Adobe's DNG SDK. The SDK's license
carries an indemnity clause for commercial distribution, which sits
badly beside the GPL's rule against further restrictions, so we carry
none of its code. Everything here is ours, written from the
specification and textbook math, and credits Adobe for the
specification and the method:

- **The spline** (`greycard_core::tone_curve`): the natural cubic
  spline through the points (C2, zero second derivative at both ends),
  its tridiagonal system solved by elimination and back substitution as
  Burden and Faires set it out, the end point's value held outside the
  points. A test pins it to two systems solved by hand. Over eight
  curves (the five Olympus DCPs' 64-point curves, a 128-point film
  curve, a 20-point S, three points), the tables match the earlier
  port's to the last bit at 65,537 inputs each, and the three test
  frames' exports with the switch on differ from the port's by one
  16-bit level at 6 to 9 values of 23 million (the hue step's
  arithmetic order).
- **The table**: 4096 steps over 0..1, a sample at each end, read by
  linear interpolation on the CPU and in the shader alike. Linear
  reading is off a smooth curve by `h²/8` times its second derivative,
  under a 16-bit level at this step for any curve a profile draws; a
  power of two keeps grid inputs exact; 16 KB for the shader.
- **The hue step** (`profile_tone::hue_preserving_tone`, `tone_profile` in
  `viewport.wgsl`): sort-free; the largest and the smallest through the
  curve, each channel placed by where it sits between them. A test
  covers all six orderings and the ties.
- **The order** follows the specification's processing model.

Departures for hostile or careless files: a point that is not a finite
number, more than 65,536 points, or a table not all finite numbers is
no curve; the table is held to 0..1 (a spline swings past it where
points bunch).

In the finish, where the display curve was (after the Light section's
shape): working space to linear ProPhoto (Bradford, neutral stays
neutral), times `2^(lift + BaselineExposureOffset)`, each channel
clipped to 0..1, the hue step, back to the working space. Then the
point curves, the look, and the output as before. Same in
`viewport.wgsl` (curve mode 3, the table in a storage binding).

- **Exposure.** Our baseline is fitted to our curve's brightness, so
  under Adobe's curve the picture read 4.6 to 7.1 L* brighter than the
  camera's JPEG. The curve now reads the picture at Adobe's
  `BaselineExposure` rather than ours: the `lift` is Adobe's less ours
  (`baseline::adobe_lift`). A DNG's own tag is Adobe's
  (`baseline::makers`); not one we wrote, a merge or a linear DNG of a
  develop, whose tag is our headroom: those are told by their
  `Software` tag, which our writer always starts with "greycard"
  (`DngTags::ours`). Every other raw carries none, Adobe's per-body
  numbers being inside its converter, so it stands in with ours less
  0.37 stops (`baseline::ADOBE_GREY_LIFT`, the one number). Matching
  mid grey through Adobe Standard's curve to ours gives 0.53 (0.49 to
  0.58 over baselines 0 to 1.2), but that left the frames below 2 to
  3 L* darker than their cameras' JPEGs; 0.37 centers them. Measured
  below. With the switch off nothing reads either.
- **The profile's look table** is read at the same lifted exposure
  while the curve renders: in the processing model both follow the one
  exposure, so a table read at our baseline under a curve read at
  Adobe's would see another picture than its author's. Off, it is read
  at our baseline as before. The switch now changes the develop key.
- **Over one.** Clipped at one. What the scene holds over the curve's
  white is the Highlights' and Whites' to bring under.
- **`DefaultBlackRender` Auto's black subtraction** is left out: the
  develop never subtracts a black.
- **Masks' bases** keep the switch off, so a base and its key do not
  move on a toggle. The print under the switch reads the look table at
  Adobe's exposure, so it no longer shares the base's first stages and
  develops them itself.

## What depends on our curve, with the switch on

- **The display curve choice** (per channel, AgX) is inert: dimmed,
  and it takes no click and no key. The Segmented's own `enabled`, as
  the mixer's controls are under black and white: the choice is a
  control inside LIGHT, not a section, so `superseded` (a section's
  switch) is not the mechanism. It holds no focus and has no keys of
  its own. What it says is kept and comes back when the switch goes
  off. AgX's output rail is off with it.
- **Fitted looks (§235)** are skipped at the render while the switch
  is on: a table fitted under a display curve of ours, or declaring
  one, corrects that curve's rendering, which the profile's curve
  replaces (`look::over_profile_curve`). A general table (a film
  preset, anyone's `.cube`) is a preference and still applies. The
  edit's choice and strength are untouched. The export and the viewport
  (its preview of a hovered look too) apply the one test. In the Look
  list a fitted look's row is dimmed, as a look with no table for the
  picture's curve already is (`look::fitted`, by the one table the
  render takes for the edit's curve); the mismatch line and the Refit
  button are not shown, so no sentence is new. A look set aside this
  way is not missing: the export does not report it as written without
  it, and the reduced develop keeps its pictures
  (`reduced::look_missing`). Under the switch only a look not in the
  directory at all is missing.
- **The scopes, the clipping lamps, and the readout** are taken again
  when the switch, the display curve, or the source changes (all three
  were missing from the scopes' key, the last two on master too).
- **Whites and a merge's pull** aim at display white: now where the
  profile's curve first reaches its top (`ToneCurve::white`), in stops
  over mid grey at the curve's own gain (`White::under`). Contrast,
  Highlights, Midtones, Shadows, and Blacks are scene shifts before the
  curve, unchanged.
- **Auto** is unchanged: it models our curve, so its targets are
  approximate under the profile's.
- **The dropper** (`pick`) reads the profile's curve as the render does.
- **Pictures** (JPEG, TIFF) never take it.

## The edit

`camera.tone_curve`, a boolean written only when on and a DCP is named,
so old sidecars read and write byte for byte. It enters the develop key
through the edit; a mask's or a fill's base clears it. `Camera::take`
carries it, but only where the DCP named carries a curve: a preset,
a paste, or a sync is settled against the profile list before it is
laid (`Preset::settled_by`), the command line's `--apply` against the
profile directory (`Preset::settled`), and the panel reads the switch
only while the chosen profile has a curve. On a profile with none, or
one not in the directory, the switch would be written, move the
develop key and the history for nothing, and come back unasked with
the next profile that has one. Choosing Embedded clears it too, and a
sidecar that says otherwise reads as off. A hand-edited sidecar that
sets it on a profile without a curve keeps it until the frame is next
edited in the window: it draws nothing. History: "Profile tone curve
on/off". The viewport and the dropper resolve the curve once per
profile and switch (`State::develop.profile_tone`).

## Notices

No ported-code entry: nothing is ported. `packaging/notices/bundled.txt`
now carries the sentence Adobe's DNG patent license asks for, word
for word: "This product includes DNG technology under license by
Adobe." It stands too on its own line in the headers of `dng.rs`,
`opcodes.rs`, and `tone_curve.rs`, since the license asks for it in
the source as well as the documentation. `README.md` carries it for
the documentation.

The `MapPolynomial` degree cap (8) and the `MapTable` cap (65,536) are
our limits, not the specification's: a few terms serve any lens or
phone correction, and a 16-bit sample needs no more entries.

`gain_map.rs`, `warp.rs`, `remap.rs`, and `opcodes.rs` cite the DNG
specification (its opcodes by name) and call our own choices ours,
where the specification is quiet; none names SDK files any more. The
rule between `WarpRectilinear2` and `WarpRectilinear` is our reading of
the 1.7 specification, which defines the newer opcode.

## Measured

Three Olympus frames (an E-M10 Mark IV frame under its own Camera
Natural profile, and two of another Olympus body under the same DCP),
3200-pixel TIFF exports, default edit, against each camera's embedded
JPEG. Mean ΔE00 and the mean L* difference:

| | off (= master) | on, our baseline | on, Adobe's | and the look table there |
|---|---|---|---|---|
| E-M10 IV frame | 10.13, 0.0 | 11.70, +4.6 | 10.28, -2.6 | 10.05, -2.5 |
| second frame | 4.44, -0.6 | 6.54, +5.2 | 4.42, -3.0 | 4.45, -3.0 |
| third frame | 2.93, -0.3 | 6.75, +7.1 | 3.47, -1.9 | 3.35, -1.9 |

The "Adobe's" columns are at the first stand-in, 0.53. The excess
closes and overshoots: 2 to 3 L* dark, about 0.15 to 0.2 stops, so we
took 0.37, which centers these three. It is fitted to three frames of
one maker against the camera's JPEG, which is not Adobe's render, and
two of them are under another body's profile; the derivation from
Adobe's curve would be the better number if Lightroom's renders bore
it out, and they are what a per-body table would need. Reading the look table at the lifted exposure moves
little (ΔE00 within 0.25).

The viewport at 1:1 against the export with the switch on: 0.067% RMSE
(GPU export; 0.067% CPU), as with it off (0.066%). GPU against CPU on
lavapipe and the default adapter: within half a level.

## Left

- The stand-in lift for raws without a `BaselineExposure`: 0.37 from
  three frames; a per-body table would need Lightroom renders.
- `DefaultBlackRender` Auto's black, and the negative-exposure bend.
- Auto under the profile's curve.
- The clamp's space.
