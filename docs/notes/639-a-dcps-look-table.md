# §639. A DCP's look table (2026-11-11)

Issue 129, and before it issue 49 and discussion 59: Adobe's Camera
profiles for a body all rendered alike, and far from the camera's
colors.

**The cause, confirmed.** One body's five Adobe Camera profiles
(Natural, Vivid, Muted, Portrait, Monotone) carry no
HueSatMap at all. Their color matrices, forward matrices, and
illuminants (17 and 21) are identical, byte for byte. Each carries a
90x16x16 sRGB-encoded LookTable, a 128-point ProfileToneCurve (the five
differ slightly), BaselineExposureOffset -0.64, and DefaultBlackRender 1
(None). Everything that tells the five apart is the look table, and
§122 applied only the matrices and the map. So greycard rendered all
five identically (ΔE00 0.000 between every pair), as the bare forward
matrix: a matrix Adobe pairs with a table, not one meant to stand on
its own. Greens went olive, and reds went pink.

**Placement.** The table is applied in `develop::finish`, straight after
the hue map, through the same stage (`MapStage::look`): linear ProPhoto,
HSV, a trilinear lookup, the encoding on the value axis alone, as §122
reads it. That is DNG's order (map, exposure, look table, tone curve)
less the exposure and the curve, which are the Look's. The viewport
and the export already meet there, so they cannot disagree, and no GPU
path is needed (the hue map has none either). It runs once a develop,
not on every Exposure drag: putting it after the edit's exposure would
mean a 3D table in `viewport.wgsl` and the export finish, with masks'
local exposure under it, or rerunning the into-working stage at full
size on every slider move. RawTherapee reads it after its exposure
compensation (rgbProc: highlight and shadow curves, then
`step2ApplyTile`, then the user curves).

**Where the table is read.** The table's value axis carries a lot:
Natural's value scale runs from 1.04 at the bottom to 0.84 at the top,
and its saturation scale from 1.0 to 0.76. DNG reads it after the
exposure: in the DNG SDK's render (`dng_render`) the hue/sat map, then
the exposure ramp, then the look table, then the tone curve. And the
specification makes that exposure `BaselineExposure` plus the
profile's `BaselineExposureOffset`, plus the user's. Our
`BaselineExposure` is the body's baseline (`greycard_edit::baseline`,
or an old edit's stored `light.baseline`): the stops our default
develop brightens a raw by, fitted to the brightness of Lightroom's
renders and of the camera's JPEG, which is the job Adobe's tag does. So
a default edit reads the table at baseline plus offset (0.8 - 0.64 =
+0.16 stops on this body). The pixel is scaled by that for the lookup
and back after, so the brightness does not move: that stays §122's
call, and the Look's. A merge of ours stores its samples its
headroom (1.5 to 3 stops) under the reference frame and the render
adds that back, so the reading point adds it too. The engine passes
the sum in (`DevelopSettings::look_baseline`, from
`Edit::look_baseline` with the frame's make, model, and headroom; the
command line uses the same function), only for an edit whose table is
on, so every other edit's stage list is as it was. It is fixed per
frame and edit, so an Exposure drag does not rerun the stage. A mask's
or a fill's base (`mask_base::base_edit`) carries an older edit's own
`light.baseline` only when its table is on, so it reads the table where
the picture does and no base made before the table moves.

**A hostile profile.** The offset is now an exponent, so it is
checked where it is read: a value that is not a finite number is
dropped as if absent, and one past ±10 stops is held there
(`dcp::sane_offset`), on the file's way in and on the way into the
engine's profile. And the look stage holds whatever it is given,
the consumer's baseline included (a sidecar's `light.baseline` is
read unchecked): stops that are not a number read the table at the
develop, and anything past ±20 is held there (`MAX_LOOK_STOPS`), so
the scale is finite and not zero. A hue map with an entry that is not
a finite number is refused when the file is read, and the develop
falls back to the frame's own calibrations with a warning, as for any
profile that will not read; before, such a map turned every pixel it
touched into NaN. A look table with one is dropped alone, with a
warning, and the profile keeps its matrices and its map: such a file
read and rendered before the table was applied, and an edit naming it
should not lose its profile now. A table with an axis of zero nodes (or a
saturation axis of one) was already refused (§122). Scales that are
finite but absurd (a saturation scale of -5, a value scale of 1e6) are
read as given: finite pixels, an ugly picture, the file's fault.

The first build read it at the offset alone (-0.64), 0.8 stops lower.
Between the two readings, same profile, one frame: mean ΔE00 1.35
(Natural), 1.66 (Vivid), 1.23 (Muted), 1.56 (Portrait), 0.19 (Monotone).
The maxima are 11 to 17, and 20 to 29% of the pixels move by more than
2. Against the camera's JPEG the two readings score the same (Natural
10.09 and 10.12): that comparison could not tell them apart, and does
not choose between them; the specification does.

**The user's Exposure is not followed.** An edit two stops up reads
the table where the default develop would. What that costs, measured
in review on Natural, Vivid, and Portrait for saturated colors: a
reading point 1 EV off moves hue by 3.4° on average (22° at most) and
the saturation scale by 5%; 2 EV off, 6.8° (30°) and 12%. Following it
needs the table after the exposure, on the GPU in the viewport.

**Clipping.** DNG clips S and V to 1 after a look table, and RT also
clips its input and leaves out-of-gamut pixels untouched
(`setUnlessOOG`). We keep §122's scene-referred stance for the value
(unclipped, so highlights above 1 survive; above 1 the table's top
slice is held). For the saturation we take a middle course: a raise
stops at ProPhoto's edge, `min(s·scale, max(s, 1))`. Adobe's tables
raise saturation by up to 4.2 (Portrait), which would throw near-neutral
colors far out of any gamut. A color already outside ProPhoto is
neither pushed further nor pulled in, and a lowering always lowers.

**The tone curve stays out.** The table was designed to sit under
Adobe's ProfileToneCurve, which adds contrast and, being applied per
RGB channel, saturation. Under our display curve the result has the
table's hues and saturations but our contrast. On this frame the
lightness already matches (mean L* 49.9 against the JPEG's 49.8). The
mean chroma lands between the JPEG and the embedded route (Natural C*
17.2, JPEG 15.5, embedded 17.4), so the missing curve is not showing
as a saturation deficit here. We did not render with Adobe's curve,
since that would mean replacing the display curve.

**Old edits, with no new schema version.** An edit says
`camera.look_table: true` only when the table is on and a DCP is
named; absent means off. So every edit written before this build reads
with the table off, keeps its picture, and writes back byte for byte.
Its develop key, thumbnail, mask rasters, and fill base prints
(`mask_base::base_edit` clones `camera`) stay valid, and 0.11.0 still
reads everything 0.12 writes. A version bump was the other way, and
was turned down: every sidecar 0.12 writes would be refused by 0.11.0,
and every old DCP edit's JSON would change, staling its caches though
its picture had not moved. It is how a mask's local white and its
Detail came in.

Every door that lays a Camera section naming a DCP on a frame turns
the table on (`Camera::take`): a preset, a paste, a sync, or an
import's preset, all through `Section::Camera`'s copy, whatever the
frame had and whatever the source's switch says. A first rule spared a
frame already on the same DCP, and one old preset over a batch then
came out two ways; now it comes out one. Choosing a different profile
on the panel does the same; choosing the one already chosen changes
nothing. A preset reads as applied when laying it again would change
nothing (`Camera::has`), so a just-applied preset reads as applied. So
the only DCP edit without its table is an old one nothing has laid a
Camera section on. An undo and a snapshot restore the edit whole,
switch and all. A brand-new edit is on the embedded profile, which has
no table.

**A mask's white balance.** A mask's own white goes back through the
develop's camera matrix to the sensor's balanced values and forward
through the matrix its white asks for (`greycard_edit::white`), which
is exact for a matrix-only develop. A map was already not undone there;
a strong look table makes the local white less exact still, since the
table's hue shifts sit between the matrix and the mask.

**Measured** on one frame (3200x2400 export, default edit,
against the camera's embedded 3200x2400 JPEG, picture mode Natural;
"hue+chroma" is ΔE00 with our L* replaced by the JPEG's):

| | ΔE00 vs JPEG | hue+chroma |
|---|---|---|
| Embedded | 9.88 | 5.75 |
| Natural, before | 11.24 | 7.50 |
| Natural, after | 10.12 | 6.08 |
| Vivid, after | 10.58 | 6.66 |
| Muted, after | 10.29 | 6.32 |
| Portrait, after | 10.74 | 6.75 |

Natural against each, after (before, all 0.00): Vivid 1.52, Muted 1.77,
Portrait 1.80, Monotone 11.71. Natural against Embedded went from 4.87
to 2.07. The stage costs about 90 ms at
24 MP (a 90x16x16 sRGB table, on a busy machine), against about 56 ms
for §122's 90x30x1 map.

**Limits.** Natural is closer, not the JPEG: the camera's own
processing (its tone curve, local contrast, and sharpening) is in the
remaining ΔE, and Natural's hue+chroma error is still a little above
the embedded route's on this one frame. One frame, one body. The
user's exposure does not move the reading point (figures above). An
old edit gets the table when a preset, a paste, or a sync lays a
Camera section on it, or by choosing another profile and back; there
is no switch of its own.
DefaultBlackRender is read by nothing (we do no automatic black
subtraction, so None is what we do anyway). The ProfileToneCurve is
still read and applied by nothing. A DNG's own embedded map and look
table are not applied on the embedded route. An older build reads a
sidecar with `look_table: true`, ignores the switch, and drops it when
it writes the file back, so the file reverts to what that build showed.
And the library index keeps the develop key the newer build computed,
so an older build sharing the index may show the newer build's
thumbnail until the sidecar changes (reasoned, not tested).
