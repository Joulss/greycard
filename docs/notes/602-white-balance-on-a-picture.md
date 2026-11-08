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
names, after the picture is linear and before everything else, where
a raw's white balance sits in the order of the stages.

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
so the next new mode costs nothing.

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
