# §683. A picture past the card's textures (2026-11-15)

Issue 152.

## The cause

A picture wider or taller than the device's `max_texture_dimension_2d`
(16384 on every Apple GPU and on lavapipe; 32768 on a recent discrete
card) went up whole as the viewport's `developed` texture. wgpu
refused the texture, its validation error reached the window's
uncaptured handler (`gpu_errors.rs`), which takes anything but the
surface's configure as a fault, and the process ended. The develop
landing had already written `last_file` (`remember_last_file` runs as
the develop lands, before its picture goes up on the next frame), so
every later launch restored the file and died the same way.

The engine was already safe: every op in greycard-gpu asks `fits`
before it makes anything of the picture's size, and a picture past
the device runs on the CPU for that develop with the context kept
(the existing tests at 256 and 512, and a new one for the export).

## Part 1: nothing made from a picture's size passes the device

Every texture made of a picture's size, and how it is bounded now:

| Texture | Where | Bound |
|---|---|---|
| `developed` (the viewport's picture) | `render/draw.rs` `upload` | reduced by the least whole factor that fits `device.limits()` (`render/reduce.rs`) |
| an engine op's result set as the source | `set_source` | made by greycard-gpu only after `fits` |
| the Original's picture put up again | `original.rs` `put_up` | the reduced texture, with its full size carried (`Landed::full`, `set_source_sized`) |
| detail planes, sharpen delta | `render/inputs.rs` `set_detail` | reduced by the same factor (same function of the same size) |
| range masks' sample | `render/inputs.rs` `sync_sample` | reduced likewise |
| camera preview / full JPEG in the loupe | `encoded_texture` | reduced like the picture; the loupe's tile names the size it stands for (`Tile::full`), so the view stays in the JPEG's pixels |
| brush and learned-mask rasters | `render/locals.rs` | within the device's side: a raster (2048 x h) on a frame tall enough asks past it; one that does is spread edge to edge to fit (`reduce::bytes_spread`), the shape told its size; one that fit goes up as it always did |
| scopes' analysis and navigator targets | `render/scopes.rs` `analysis_size` | 512 wide and as tall as before where that fits the device; past it, that tall and narrower, in 64s for the copy's rows |
| tone guide plane | `finish::guide_plane` | `GUIDE_EDGE` (1024) on its long edge already |
| viewport target, output-sharpen targets, peaking | window-sized | the window's size |
| thumbnails, lightbox, grid, previews | engine/library | a long edge (170 up to 2048) already |
| look preview | through the viewport | no picture texture of its own |
| engine ops (sharpen, CA, lens, local contrast, dehaze, demosaics) | greycard-gpu | `Context::fits` before any texture; a refused size is the CPU's |
| the window's own Slint images (navigator, texture handed to Slint) | startup | the analysis size and the window's size |

Not audited to the end: greycard-ai's models on ONNX Runtime's WebGPU
(Dawn, outside wgpu's validation; a failure there is an error, not
the window's handler) and generative fills at full resolution.

### The reduced viewport

The renderer keeps the source's full size beside its texture
(`Renderer::full`, `developed_size`). `Params::image` is that full
size, so every position the shader works in (the frame, the turn, the
masks' uv, the guide plane, the vignette, the grain) is the picture's
own. Only where a texture is read is the position mapped into it,
by `to_texture`, and only for a reduced source: the whole factor k
comes from Rust (`Params::minify.y`), and at k = 1 the position goes to
the texel read untouched, as before. No ratio is taken on that path: a
first version scaled by the texture's size over the full size, and on
the NVIDIA card a number over itself is not always exactly 1, which
moved a quarter of 1120 probe views by up to 2 levels.

A reduced picture is boxes of k: texel i holds pixels [k i, k i + k),
the far edge's partial box padded with the edge pixel as the sampler
clamps (`reduce::plane`), so every texel center is at its box's
center, and the shader divides by k and nothing else. The first
version mapped extent to extent (`dims / full`) over a box grid, which
slid the picture up to (k - 1) pixels toward the far edge where a side
was not a multiple of k. The planes of the source's size and the
range masks' picture are the same boxes; the pick reads texel x / k;
the Original's texture is handed back with its full size; the loupe's
tile carries its own. A raster is the one thing read edge to edge
(`uv * size`), so a reduced one is spread edge to edge too. The view the
UI hands over is unchanged: fit, 1:1, zoom, crop, masks, brushes, the
droppers, the readout, the navigator, and the scopes are all in the
full picture's pixels. 1:1 shows the reduced picture magnified.

The log says it once per picture (size and factor), and again only
when another size or factor comes or a picture that fits went up in
between. No UI text.

## Part 2: a launch never reopens a frame the last died opening

`crates/greycard-ui/src/open_note.rs`, a sibling of `gpu_marker`. The
note is `<config>/greycard/opening` (beside `settings.json`), with a
lock file beside it held for the session.

- At the start, before the window: the lock is taken (a second
  greycard beside a live one cannot, and leaves the note alone). A
  note naming the file this launch would restore (`Select::Last`,
  compared by canonical path) means the last launch died opening it:
  the file is not opened, its folder opens in the Library grid with
  no frame open (`open_loaded`, `Launch::not_reopened`, once), the log
  says why, and the note is taken off so the file opens by hand. A
  note naming another file is let go; with nothing to restore (a file
  named on the command line, a `last_file` that is gone) it is left
  for the launch that would. The restored file is noted before anything
  opens it, right after the GPU select. The note's path is the
  system's own bytes (UTF-16 on Windows), not trimmed or decoded.
- A run the command line owns (`--snapshot`, `--screenshot`, `--keys`,
  `--press`, `--menu`, `--turn`, `--cull-key`, the timing runs; the
  headless `--export` and imports return before) neither reads, takes
  off, nor writes the note: a capture never uses up or hangs on a note
  meant for a launch by hand, and never leaves one naming the user's
  file.
- As a develop lands for a file other than the one noted or last
  drawn (`remember_last_file`, the same moment `last_file` is
  written): noted. A slider's develop of the same frame writes nothing.
- Taken off as soon as the view on screen is past what the note
  guards, by exactly these:
  - Develop: the develop's picture put up since the note
    (`land_pending` → `picture_up` → `put_up`, after the upload came
    back) and then a frame of the develop view drawn (`drawn`, right
    after its texture is handed to Slint in the window's draw).
  - Cull: the loupe's draw of the open frame's picture (`cull_frame`,
    after its texture is handed to Slint, when the open frame's tile
    was among those drawn → `shown`), whatever of a develop went up.
  - Library (grid or lightbox): at once, as a list opens there
    (`open_loaded` → `shown`): its pictures are thumbnails, never a
    frame's full picture.
  - Another frame becoming current (`current_moved` → `moved_to`), and
    the event loop ending as it ends (`quit`).

  A note taken off by `drawn` or `shown` leaves the frame settled: a
  develop of it landing later (a slider, or the grid's frame opened in
  the Loupe) writes none.

Cost on a normal launch: one small write at the start and one remove
after the first frame; one write and remove per new frame that lands.
Not synced: what the note detects is the process ending, and a process
that ends leaves its writes in the system's cache; a sync was 5.4 ms
median per frame change on the window's thread here. A settings folder
the lock cannot be made in is said as that, and the launch goes on with
no note. No UI text.

False trips, by design: anything that ends the process between a new
frame's develop landing and its first draw counts as dying opening it,
so a SIGTERM, a logout, an out-of-memory kill, or an unrelated crash in
that window skips the frame once at the next launch (it opens by hand,
and the note is gone). A crash after the draw is never counted. The GPU
start marker and this note cannot both count one death: the marker
covers only Slint's select, and the note is written after it, so a
death in the select is the marker's alone and a death after it the
note's alone. Both can be found at one launch only from two earlier
deaths of different kinds, and each then does its own thing.

Ruled out: making texture-creation validation survivable in
`gpu_errors.rs`. Its header keeps every uncaptured error but the
surface's configure a fault; a texture refused there leaves the
renderer without its source mid-frame, and catching it would hide the
next texture made too big. Part 1 bounds by construction; part 2 is
the net.

## Measured

- Lavapipe (16384): a 18432x10368 JPEG opens, shown 2x smaller
  (9216x5184), fit and 1:1 captured; 1:1 centered on the picture's
  center matches the NVIDIA card's (32768, whole) to MAE 0.5% of the
  viewport, the same circle in the same place. A 17000x400 and a
  400x17000 open; 16384 wide goes up whole, 16385 halved.
- A probe (1120 views of four fitting pictures over
  zooms, turns, an angled perspective, the mixer, the output sharpen,
  three masks drawn and alone, the planes, the guide, and the range
  sample; plus the pick, the scopes' bins, and the navigator), this
  branch against c7b2a5e4: byte for byte on NVIDIA and on lavapipe. The
  test `a_picture_that_fits_draws_byte_for_byte_as_before` holds the
  shader to its mapping-free form on the same views; with the first
  version's ratio back it fails on NVIDIA (24 of 1120).
- A one-box column and row near each edge and in the middle, sides not
  multiples of k, k = 2, 3, 4, at 1:1 and 3x: the reduced picture's
  centroid on the whole picture's to 0.000 px (the first version: up
  to 2 px off).
- Viewport tests at a 256 limit against the same picture whole: 0.03
  levels at fit, 0.2 at 1:1, 0.4 to 0.65 at 3x; drawn 4 to 6 pixels
  off, 2.8 to 11 levels. Scopes' shares 0.017 apart (of 2). Same on
  lavapipe.
- Exports of the oversize pictures: full size, on lavapipe and on the
  NVIDIA card (whose own export device asks 16384, so both are the
  CPU's), identical. Engine test: an export past a 256 device is the
  CPU's to the bit, the context kept.
- Crash loop on a build with part 1 switched off: launch 1 dies, 2
  opens the Library without the frame, 3 opens it again and dies, 4
  skips again; with part 1 back, 5 opens it and the note is gone.

## Left

- Metal itself is untested. On a Mac: open a picture over 16384 a side
  (the log's "viewport: the WxH picture is past the device's 16384 a
  side" line), check fit, 1:1 and a brush on it, and quit and reopen.
- The export's own device asks 16384 (`WANTED_TEXTURE_DIMENSION`), so
  a 32768 card exports such a picture on the CPU; asking the adapter's
  own limit would put it on the GPU.
- A frame past 128:1 tall on a device that cannot take its scopes'
  picture leaves the bottom of it out.
- The skipped launch's capture (`--snapshot`) waits for a develop that
  never comes, as any Library launch with no frame open does.
