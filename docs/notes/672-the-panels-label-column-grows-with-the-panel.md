# §672. The panel's label column grows with the panel (2026-11-14)

Widening the develop panel widened the sliders' tracks but not their
labels: the label column was a fixed 68 px, so a label the default cut
("Roundness", "Temperature"; in French "Température", "Luminance",
"Hautes lumières", "Pondération", "Chrominance") stayed cut however
wide the panel was dragged.

## The rule

One column for the whole panel, `LabelColumn.width` in theme.slint,
set from the panel's width as it changes (`changed right-w` in
app.slint, and at `init`):

    column = clamp(68 px + (panel - 320 px), 68 px, 116 px)

At the default 320 px and under (the drag's floor is 280) it is 68 px,
as before, to the pixel. Dragged wider, the labels take the extra
first: from 320 to 368 px the tracks keep their default length and
only the column grows; past 368 the column holds at 116 px and the
tracks grow. Every row of the panel reads the same column, so the
tracks and the rows' controls still line up down the panel.

## The cap, from the labels

Measured at the panel's type (12 px) in Noto Sans, the Linux default
sans-serif, through parley 0.11.1 as Slint lays text out. A label is
whole when it fits the column less the 8 px gap, so a label `w` wide
is whole from a panel of `w + 260` px. Segoe UI and San Francisco set
these narrower.

| Row | English | px | whole from | French | px | whole from |
|---|---|---|---|---|---|---|
| Temperature (WB) | Temperature | 72.7 | 333 | Température | 72.7 | 333 |
| Blue / yellow (Lens) | Blue / yellow | 71.3 | 332 | Bleu / jaune | 67.2 | 328 |
| Roundness (Vignette) | Roundness | 62.6 | 323 | Arrondi | 42.5 | default |
| Luminance (Mixer) | Luminance | 62.1 | 323 | Luminance | 62.1 | 323 |
| Luma (Noise) | Luma | 31.7 | default | Luminance | 62.1 | 323 |
| Darks (Curves) | Darks | 32.6 | default | Tons foncés | 66.5 | 327 |
| Weight (B&W) | Weight | 39.9 | default | Pondération | 69.6 | 330 |
| Custom (Crop) | Custom | 43.6 | default | Personnalisé | 72.3 | 333 |
| Red / cyan (Lens) | Red / cyan | 58.3 | default | Rouge / cyan | 73.0 | 334 |
| Midtones (Light) | Midtones | 52.9 | default | Tons moyens | 73.7 | 334 |
| Chroma (Noise) | Chroma | 44.9 | default | Chrominance | 75.4 | 336 |
| Canvas (Monitor) | Canvas | 40.3 | default | Zone de travail | 83.5 | 344 |
| Highlights (Light, Curves) | Highlights | 57.9 | default | Hautes lumières | 91.8 | 352 |
| Feather (Vignette, masks, Retouch) | Feather | 43.2 | default | Contour progressif | 106.3 | 367 |
| Dehaze (Detail) | Dehaze | 42.1 | default | Correction du voile | 106.5 | 367 |
| High fade (luminance mask) | High fade | 54.9 | default | Dégradé des tons clairs | 131.9 | cut (needs 140) |
| Low fade (luminance mask) | Low fade | 50.9 | default | Dégradé des tons foncés | 139.6 | cut (needs 148) |

The 84 px rows (the mask's Refine edge and Opacity, the color range's
five):

| Row | English | px | whole from | French | px | whole from |
|---|---|---|---|---|---|---|
| Chroma fade | Chroma fade | 73.1 | default | Dégradé de chrominance | 142.4 | cut (needs 151) |
| Hue fade | Hue fade | 51.2 | default | Dégradé de teinte | 101.6 | 362 |
| Refine edge | Refine edge | 67.1 | default | Affiner le contour | 98.7 | 359 |
| Chroma | Chroma | 44.9 | default | Chrominance | 75.4 | default |

Every other label in the panel is whole at the default in both.

So every English label is whole from 333 px, and every French one but
the three fades from 367 px. 116 px (68 + 48) is the least column that
holds Feather's and Dehaze's French, two of the panel's commonest
sliders. A section's slider row is about 288 px at the
default panel, so at 116 the column is 35 % of the row where it stops
growing (368 px panel, 336 px row). The fades would need a 140 to
152 px column: at 152 it would be 41 % of the row there, and in
English most of it empty beside "Exposure", for three compound phrases
in two mask panels. They stay cut; a shorter French word is the better
fix there.

## What follows the column, and what does not

- `EditSlider`'s default `label-width` is the column, so every slider
  in the panel follows: Develop, Crop (Angle, Vertical, Horizontal),
  Masks (each mask's sections, the luminance window, the brush),
  Retouch, and the Look section's Strength.
- The rows with a label and a control: Crop's Aspect and Custom,
  Lens's Maker and Lens, Monitor's Canvas, and Cull's Picture, Turn,
  and Compare; Cull's test for putting Picture over its buttons
  counts the column too.
- The 84 px mask rows take `max(84 px, column)`: 84 at the default,
  as before, and the shared column once it passes 84 px, so at the
  cap they line up with the rest rather than keeping a 16 px step.
- Not the export sheet (its sliders now name their 68 px), not the
  Settings sheet's rows, not the left pane. The Scopes head has no
  label column.
- The value field, the double-click reset, the hit area, and the focus
  ring all hang off the track's own `track-left`/`track-right`, so
  they move with it. A drag in progress moves the value by the
  pointer's travel over the track's width, not by its position, so a
  change of column mid-drag does not jump it, and between 320 and
  368 px the track's width does not change at all.
- A width kept in the settings sets the column before the first frame
  (Slint runs change handlers before it renders). The UI scale is the
  window's scale factor, so the column, in logical pixels, scales
  with everything else.

## Tested

In `panel/sizes.rs`: the column is 68 px at the default and at 280,
and Roundness's and Exposure's tracks are the default's from 320 to
368 px while their labels grow (92 px at 344, 116 at 368); past it
the column holds at 116 and the track takes the rest, to the widest
the window allows; the Crop tab's Aspect row follows; the rule
gives Roundness's measured width its room from a 324 px panel (the
width is stated in the test, not measured there); a kept width sets the column on the first
pass of change handlers, and a hidden panel leaves it at 68; the
export sheet keeps 68 px under a wide panel.
