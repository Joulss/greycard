# §602. White balance on a picture (2026-11-07)

A JPEG, a PNG, or a rendered TIFF opens with WHITE BALANCE greyed
"raw only" (§50). The reason was sound: the raw path applies white
balance as gains in camera space before the matrix, and a picture has
no camera space and no as-shot white, so a kelvin number on it would
be a lie, 6500 K standing for "whatever rendered it did". Testers ask
for it anyway, and the ask is fair: a warm JPEG is as common as a warm
raw.

## The decision

A picture's white balance is an offset from its own white, in mired
and a tint difference, the form a mask's white already takes (§400).
The picture's white is its color space's: D65 for sRGB, Adobe RGB,
and Display P3, and the profile's white point for a tagged TIFF. The
offset is applied in the working space as a chromatic adaptation
(Bradford, `color::CAT`) from that white to the light the offset
names, after the picture is linear, turned, and reduced, and before
everything else: a per-pixel matrix commutes with the turn and the
reduce, so the reduced pictures pay it at their own size.

Mired, not kelvin and not a unitless pair: "so much warmer" is a
mired distance whatever the starting light (§400 gives the numbers),
and the repo already means mired by "relative", in the mask's white
and the sync sheet's. Lightroom's -100 to +100 on a JPEG is a scale
nobody can carry to a raw; ours carries, so a look with a white
balance offset applies to a picture as it does to a mask.

The section's sliders for a picture read as the mask's offset
sliders do, "+25 mired" and the tint as Duv, 0 at rest; As shot is
the offset of zero. The dropper stores the offset that makes the
pick neutral, as the mask's dropper does. Auto white balance (§194)
reads the picture and gives an offset. A mask's relative white adds
its offset to the global's; a mask's absolute white names no light a
picture can resolve, so on a picture it is read as the offset it
would be from the picture's white, and the Follow global toggle is
not offered.

## The schema

The edit's `WhiteBalance` is tagged by mode, `as_shot` or `custom`,
and its reader has no fallback: every released build refuses a
sidecar with a mode it does not know, and the frame opens with the
default edit (the mask's white has the fallback, §400; the global
never got one). So a new mode would cost a picture's sidecar every
older build, and a flag beside `custom` would have an old build read
"-150" as a light and clamp it to 2000 K.

The offset is written beside `as_shot` instead: `{"mode": "as_shot",
"offset": {"mired": 10, "tint": 0}}`. An older build reads the mode
it knows and ignores the field, so it opens the sidecar with the
picture's white balance off and renders nothing wrong. The schema
version stays. A raw's edit never writes an offset, and a picture's
never writes `custom`; a preset or the copy sheet carries whichever
it has, and the receiving frame's kind decides what applies: an
offset applies to either, a kelvin light to a raw alone. From this
build on the reader also takes a mode it does not know as As shot,
so the next new mode costs nothing; the price is that a sidecar with
a later mode, saved again by this build, loses that mode where before
the sidecar was refused whole.

The decode already adapts every picture's white to the working
white: sRGB, Adobe RGB, and Display P3 through their matrices, an
ICC profile's colorants from D50. The profile's own white point
(`wtpt`) is not read today, so the picture's white, which the offset
is from, comes with it: D65 for the named spaces, the profile's for
a tagged TIFF (ProPhoto's is D50).

## The mechanism

`WhiteShift` (greycard-edit, white.rs) today is made from a develop's
resolved white balance and goes back through the camera profile to
make a mask's matrix. A picture has no profile, so `WhiteShift` gets
a second constructor from a white point alone, and its matrix to a
target light is the adaptation in XYZ between the two whites, carried
into the working space. The same constructor serves the picture's
global offset and its masks' offsets, so there is still one meaning
of a white shift. `color::white_balance_at` stays the raw's.

Export, the command line, the reduced pictures, and the edited
thumbnails take the stage as the viewport does, since it is one step
in the walk's list; the CPU reference is the matrix multiply, and the
GPU needs nothing new, the stage running where the picture's decode
to linear already runs. The identity test between the editor's
export and the command line's covers it on a picture.

## Not in this

The NOISE section stays "raw only" for now. Its chroma denoiser works
on RGB and could run on a picture; the learned tier wants a mosaic
and never can. That is a line of its own on the roadmap. DEMOSAIC
stays raw only, there being nothing to demosaic.

## What was built

**The picture's white.** `Picture` carries `white`, the white of the
space its samples were in: D65 for sRGB and for a file without a
profile, and for an embedded matrix profile its media white, which is
`chad` undone on D50 when the profile has one (a version 4 profile's
`wtpt` is D50 by rule) and its `wtpt` otherwise; D50 for a profile
naming neither. So ProPhoto comes back D50 and a version 4 Display P3
D65. `picture::white_temp_tint` gives it as a temperature and tint
(D65 is about 6504 K, +0.0032).

**The schema.** `WhiteBalance::Offset { mired, tint }`, written
`{"mode":"as_shot","offset":{...}}`; As shot and a light are written to
the byte as before. A mode the reader does not know is As shot; a
`custom` without its numbers is still refused. On a raw an offset
develops as As shot, as an older build reads it. The schema version
stays.

**The matrix.** `color::adaptation(from, to)`: Bradford from `to` to
`from` in XYZ, between Rec.2020's matrices. The stage is
`adaptation(own, light)`; a mask's matrix on the developed picture is
`adaptation(light, target)`, and since Bradford is a diagonal in the
cone space the two compose to `adaptation(own, target)`.
`WhiteShift::picture(light)` is the constructor from a white point
alone; `WhiteShift` holds a camera base or a picture base, and
`local()` serves both, an absolute mask light on a picture coming to the
offset it is from the picture's white.

**The stage.** `Stage::White { own, light }`, in a picture's list
after the turn and the reduce (a per-pixel matrix commutes with both,
so a reduced or thumbnail develop pays it on the reduced pixels) and
before everything else, only when the light differs from the picture's
own, so As shot
and an offset of none are no step at all and develop to the bit as
before. CPU only (`walk::white_matrix`, the reference and the only
version), where the picture's linear decode runs; the viewport,
prefetch, export, command line, reduced pictures, and edited
thumbnails all take it from the list. `Meta::picture` carries the own
white and the light, so the masks' white shifts in an export and the
window's `WhiteBase::picture` come from the develop that ran.

**The panel.** WHITE BALANCE is live on a picture with the sliders in
the mask's offset form ("+25 mired", Duv). The kind the sliders are
read as is the open file's, by its name (`picture-open`, set when the
frame's edit is shown), never what the last decode said, which holds
the last frame's kind until this one's develop lands. The span is what
reaches the panel's lights (2000 to 12000 K, ±0.05 Duv) from the
picture's white, `white::picture_offset_range`: about -346 to +70 mired
from D65, -300 to +117 from D50, set when the decode says the white;
the dropper and Auto clamp to the same. A mask keeps ±450 as §400 has
it. A white of the other kind, a kelvin light on a picture (master's
paste or sync wrote them) or an offset on a raw (a hand edit), shows as
As shot, is logged once a file, and is left in the edit untouched until
a slider moves, so opening the frame writes nothing. As shot is the
offset of zero, the double-click puts a slider back to none. The
preview while a slider drags is `adaptation(shown, panel light)`. The
dropper stores the offset that makes the pick grey
(`color::picture_neutral`); Auto takes the grid back to the picture's
own white, runs grey-world with no gains and no matrix against a
neutral prior, and brings the answer forward, so a second press finds
what the first did. A mask's white on a picture starts as an offset
with Follow global hidden. `W` works on a picture.

**Presets and the copy sheet.** `preset_fit::white_fitted` at each
frame a preset, a paste, or a sync lands on. The raws' As shot an
offset lands at is read on the pool before the lay, with the sync's
own reads (`read_whites`); the open frame's comes from the panel, read
when it was decoded; `--preset` at start reads its one file: an offset onto a
picture as it is and onto a raw as the light it names at the raw's As
shot (`white::landing`, the header read only then); a kelvin light
onto a picture is left out and logged, as is an offset onto a raw
whose As shot will not read.

**Small rules.** A stored offset of none reads back as As shot, so a
hand edit or a preset writing one is no step. A profile whose white is
no light (zeros, Y of nothing, a negative) is taken as D50 with a line
in the log and the picture opens, as before the white was read. A
first move of the tint alone reads "Tint +0.004" in the history, the
picture's row and a mask's offset row alike.

**Measured.** A raw's offset through the profile path and through the
XYZ path, on an ordinary sensor's two calibrations: the same grey for
a tint alone (the profile's matrix is the same at the same kelvin),
0.0017 uv at +25 mired and 0.0047 uv at worst over ±60 mired. The
difference is the profile's matrix read at another light, not quite
Bradford's rendering of the old white.

### Verified
- greycard-edit: the wire form both ways, an unknown mode as As shot, a
  sidecar with an offset read through the old reader's enum as As shot
  with the rest kept; the offset's matrix against Bradford written out
  by hand at four lights (1e-4); a warmer offset warms a grey; the
  dropper's light makes the pick grey; a mask's offset composes; the
  profile and XYZ paths agree to the number above; what lands on a
  picture and on a raw.
- greycard-core: the white of sRGB, a version 2 `wtpt` (D50, D65), a
  version 4 `chad`, and none.
- greycard-engine: the stage's place in the list; zero offset equals As
  shot to the bit through the walk; +25 mired warms a grey by the
  adaptation exactly; the window's export and the command line's of a
  PNG with an offset identical but for the dates (lavapipe), the grey
  patch warm against As shot; `white_fitted`.
- greycard-ui: the picture's section live, a drag an offset and one
  step, the "+N mired" readout, the preview matrix, the double-click,
  the dropper, Auto stable over two presses, a mask's offset adding to
  the global with Follow global absent and the dropper under it; the
  raw's white balance tests unchanged.

- After review: a raw chosen after a picture, moved before its
  develop lands, takes a light and a picture after a raw an offset
  (the repro as a UI test); a picture holding a kelvin light shows As
  shot and reads back unchanged until moved; the slider span from D65
  and the dropper's clamp to it; a broken `wtpt` opens as D50; an
  offset of none reads as As shot; the tint-only history rows.

### Not verified
- Not looked at on screen in the running editor; the UI tests drive the
  panel headless.
- The live preview while dragging on a real picture's GPU texture: the
  matrix is tested, the drawing is not.
- A real ProPhoto or v4 Display P3 file: the profile reading is tested
  on hand-built profiles only.
- The pool read of the raws' As shot for an offset preset is not timed;
  a preset onto the open raw before its decode lands leaves the white
  out (logged), there being no As shot yet.
- `--temperature` on the command line still writes a kelvin light, which
  a picture ignores; unchanged from before.

### Recorded
- The reader's fallback for a mode it does not know means a later
  build's new mode, read here as As shot, is gone when this build
  saves the sidecar, where before the whole sidecar was refused.
- `W` and Neutral are enabled with no frame open; the dropper then
  answers "wait for the develop", which is harmless.
