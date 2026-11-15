# §679. The curve editor's histogram is what the curve takes in (2026-11-14)

This supersedes §22's "the histogram's key includes it, so the bins
follow a drag".

## The decision

The histogram behind a curve is the histogram of what that curve takes in,
not of the picture after it. Lightroom, Capture One, darktable, and
RawTherapee all draw the input. Until now the editor drew the scopes' own
histogram: the export space's encoded values after every curve, the look
table, the output sharpen, the export matrix, and the grain. That paired the
curve's input axis with an output histogram, and it moved under the pointer
while a point was dragged. A first-time contributor's PR 150 proposed the
change, along with a sharper picture and round points. We finished it.

## What "what the curve takes in" is

The order is checked in the code (`Curves::bake_with`, and `curve()` in
`viewport.wgsl`):

- Each channel goes through the parametric curve first, then its own point
  curve (red, green, blue), then the master (RGB).
- The color curves (R/G, B/Y) read the lightness of the result.
- A mask's curves act first, on the value before every curve, blended in by
  its weight. The global look's curves act on the result.
- The curves act on the encoded working-space value (Rec.2020), after the
  display curve (per channel or AgX), and before the look table, the output
  sharpen, the export matrix, and the grain.

So:

- **The analysis stops at the curves' input.** It is a second analysis draw
  through the same viewport shader with a tap (`cubic.w` 2, 3, or 4, `Tap`
  in the viewport crate).
  - A mask's editor reads the value before every curve
    (`Tap::BeforeCurves`).
  - The global editor reads it after the masks' curves are blended in
    (`Tap::BeforeGlobalCurves`).
  - The color curves (R/G, B/Y), in either editor, read past every point
    curve, the masks' and the global look's blended together
    (`Tap::AfterCurves`). Its key levels only the color curves.
  - It is binned by the scopes' compute pass, weighed by the selection as the
    scopes are.
- **The CPU moves that histogram through the curves ahead of the one
  shown** (`Curves::taken_in`, `histogram_taken_in`):
  - the parametric: nothing;
  - a channel's curve: the parametric;
  - the master: the parametric, then the channel's own curve.
- **The color curves get no move.** The point curves ahead of them are
  every look's, blended per pixel by the masks' weights, so no move of one
  look's curves stands for them; the third tap reads them as they are.
- **The point curves' move is exact.** Each tone curve maps one channel to
  itself. A
  level's count is spread over the span its half-level edges land on, so a
  curve ahead that stretches a range leaves no comb of empty levels. A
  first cut moved each level whole, and the backdrop combed visibly under
  a strong parametric.
- **What still reads the scopes' bins:** the scopes and the clipping lamps,
  as before.

## What it costs

- **The second analysis** is a 512-wide draw, a compute pass, and 3 KB read
  back, in its own read back beside the scopes'. It runs only while the
  CURVES section shows: open, panel up, on Develop or on Masks with a mask
  chosen. Its key leaves out the curves past its tap, so a drag of the
  curve shown dispatches nothing.
- **Measured** on a 6000x4000 synthetic frame, median of 20, dispatch to
  landing:

  | Device | Scopes | Tap | Both together |
  |---|---|---|---|
  | RTX 5070 Ti | 1.52 ms | 1.15 ms | 1.91 ms |
  | lavapipe | 8.0 ms | 4.7 ms | 12.3 ms |

  GPU memory: one more 512-wide Rgba8 target (about 0.7 MB at 3:2) and 3 KB
  of buffers.
- **The two read backs share one struct and one dispatch helper.** Each has
  its own "image changed" flag. The contributor's version copied the
  scopes' flag into its own, so it re-sent every time it landed while the
  scopes' read back was out.
- **What follows the tap sends nothing.** The tap's key leaves out the
  grain and the output sharpen, and a new display LUT, export matrix, or
  look table marks only the scopes' read back. A tapped draw has the output
  sharpen off only because it stops before it, so it leaves the viewport's
  output-sharpen targets alone; letting them go made the next viewport draw
  allocate them again at full size.
- **The backdrop is never another frame's.** The renderer counts the
  frames opened and stamps each source with the count it came in under;
  the tapped read back carries its source's stamp. After a frame is opened
  and before its develop comes in, the source is still the last frame's:
  the tap sends nothing, and one on its way is let go when it lands. The UI
  takes no tapped histogram either until the picture on the GPU is the open
  frame's own develop (`own_develop`), since a develop of another frame can
  still go up after the open. The scopes keep master's way: they describe
  the picture on screen, the last frame's until the new develop lands, and
  show whatever lands. A stream of develops of one frame keeps showing each
  read back.
- **The backdrop is never another curve's input.** The editor's cached
  histogram carries the tap it was taken at, and every draw of the editor,
  and the frame loop, goes through one rule (`curve::backdrop`): bins of
  another tap are not drawn. Switching between the global editor and a
  mask's, or into or out of R/G and B/Y, shows the editor bare until its
  own histogram lands. The renderer drops bins taken at another tap or
  weighing as they land, and a hidden section lets its histogram go, so
  showing it again draws nothing taken before.
- **The splines are built once a draw,** not once a level
  (`Curves::taken_in_by`).
- **The bars' height leaves out each channel's black and white.** A curve
  ahead that clips piles everything past its end there; counted in the
  peak, it flattened the rest to about a tenth. Master left both end levels
  out too.
- **The curve dropper lands on its spike.** The picked value goes through
  the curves ahead as the histogram does: a channel's value past the
  parametric, the master's luminance of the values past each channel's own
  curve. Left as it was: `finish::pick` reads the picture without the
  masks, so under a mask that changes exposure or curves the tone picked in
  the global editor lands off its spike there.

## The picture

- **Size.** The editor's picture is drawn at its own laid-out width times
  the window's scale, one picture pixel to one screen pixel. The editor
  reports its width in physical pixels on its first layout and on every
  change, so a move to a monitor of another scale draws it again. The
  contributor's fixed 768 was downscaled 2.6:1 at the default 1x panel and
  upscaled on wide HiDPI panels.
- **Strokes and marks** are sized in logical pixels and anti-aliased: the
  grid 1, the curve 2, the split triangles 8 high.
- **Points:**
  - 8 px discs with a dark rim (the old squares were 10 px, the PR's dots
    about 6 px).
  - The selected point is a 10 px disc with a 2.5 px accent ring: two whole
    pixels of accent between its anti-aliased edges at 1x.
  - Each disc is drawn whole inside the picture, so the black and white
    points show their whole handle at the corners.
  - The hit radius (`curve::HIT`) is unchanged.
- **`draw_curve` per call** (release, the least of three under a busy
  machine):

  | | 1x | 2x |
  |---|---|---|
  | master (256 px) | 0.29 ms | 0.38 ms |
  | landed (288 px / 576 px) | 0.48 ms | 1.24 ms |
  | contributor's 768 px | 2.0 to 2.2 ms | 2.0 to 2.2 ms |

  The buffer is 249 KB at 1x and 995 KB at 2x, against 197 KB before and
  1.77 MB at 768.

## Tests

- **GPU, on the 5070 Ti and on lavapipe:**
  - Where nothing follows the curves, the tap's bins are the scopes' at
    either tap: 0 of 523,776 counts apart on the card, 8 on lavapipe.
  - Neither tap moves or dispatches for the global curve; the
    before-every-curve tap neither moves nor dispatches for a mask's curve,
    which the global tap takes in.
  - A lost tapped read back is made again.
  - Bins of another frame are let go by the tap, while the scopes show
    them as master does; between a frame's open and its develop the tap
    sends nothing and drops what lands; a stream of develops of one frame
    still shows the scopes.
  - A tapped histogram keeps the output sharpen's targets.
  - Bins of another tap, or taken before the histogram was let go, are not
    drawn.
  - With a look table, another export space, and grain on, each tap's bins
    are those with them off, while the scopes' move; none of these, nor
    the output sharpen, sends the tap again.
  - With nothing past them, the global curves' tap reads the scopes' bins
    under a local's strong curve, and the last tap reads them under a
    strong global curve: 8 and 4 of 523,776 counts apart on the card, 4
    and 16 on lavapipe.
  - An image change sends each read back once.
- **CPU:**
  - Each curve takes in what the curves before it make, checked against
    the baked table.
  - Curves switched off take in the value.
- **UI:**
  - Each curve is drawn over what it takes in.
  - A stretch ahead leaves no gaps.
  - The backdrop follows the curve shown.
  - The editor is drawn at its own size, and again at a scale change.
  - A curve ahead that clips leaves the bars their height.
  - The dropper lands on its tone in the backdrop.
  - Each curve's histogram comes from the right tap.
  - Bins of another tap are never drawn behind the curve: the global
    look's behind a mask's, a point curve's behind R/G or B/Y, and theirs
    behind a point curve.
  - The end points show whole, and the selected ring is at least two
    pixels.
  - The histogram is taken only while the section shows.
  - The clipping lamps read the scopes' bins, not the editor's.

## Left as they were

- The backdrop of a mask's curve is of the whole frame unless the
  Selection toggle weighs it by the mask, as before.
- The color curves' lightness histogram is still the channels' mean
  resampled to Oklab's L.
