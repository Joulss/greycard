# §681. A frame opens at the camera's aspect (2026-11-14)

Issue 145.

Olympus and OM System save the whole 4:3 sensor and record the aspect
chosen in camera in the maker note. We now open such a frame at that
aspect: an edit with no crop shows, develops, thumbnails, and exports
at the camera's frame. It is a default, never stored; opening a frame
writes nothing.

## The fact

The note's Image Processing sub-IFD (0x2040) holds AspectFrame
(0x1113), x0 y0 x1 y1 inclusive, in the coordinates of the valid area
CropLeft/CropTop/CropWidth/CropHeight (0x0612 to 0x0615). rawler's crop
for an ORF is that valid area. `decode::camera_frame_path` reads it from
the file's head, with no decode, as a `CameraFrame`: the area and the
frame in sensor samples and the IFD0 orientation. `None` for every other
maker, a DNG, a note that does not say, a frame outside the area, and a
frame that is the whole area (every 4:3 frame). An .ORI reads as an ORF
does (the reader goes by content). The style reader and this one share
the Olympus note's header parse and sub-IFD lookup.

The mechanism is general: another maker fills the same `CameraFrame`
(a DNG's DefaultCrop, Panasonic's or Fujifilm's aspect) and everything
below follows. Olympus is the only one read now.

## Where it goes

- **Facts.** `Facts::camera()` gives the frame as fractions of the
  develop (minus the valid area's origin, over its size), turned by
  `tag.turned(turn)` the way `develop::orient` turns the picture. Only
  a raw that is not a merge, and only when the decode's crop is the
  note's area.
- **Geometry.** `crop: None` resolves to the camera's frame through a
  parameter: `effective_crop`, `frame`, `is_identity_on`, and
  `with_aspect` take `camera: Option<Crop>` beside the size, so the
  compiler asked every caller. `bounds` reads no crop and takes none.
  The frame is mapped into the plane through the quarter turns and the
  mirror (exact); where an angle or a keystone leaves it off the
  source, the largest of its ratio about its center, as a user crop is
  refitted. An aspect over no crop (§666) is the user's framing and
  still wins: fitted about the camera frame's center over the whole
  sensor, as picking that ratio in Crop fits it. A frame centered on
  the picture (within a thousandth, as a body's are) is centered at
  exactly 0.5, so a ratio over no crop frames bit for bit as before. A
  stored crop that no longer fits is refitted exactly as before, the
  camera's frame no part of it.
  `with_camera_frame` turns the default into a crop for the renderers
  alone, so they take a whole geometry and no new parameter.
- **Original** on such a frame is the camera frame's ratio
  (`Geometry::own_shape`, `ratio`). The whole sensor is Free or 4:3
  away. The Portrait switch stands the camera's shape on end.
- **Crop mode.** The box opens on the camera's frame over the whole
  picture. Done untouched stores nothing. A drag, a draw, or a ratio
  picked stores a crop, as any does. Reset (Crop's, or Reset to
  Original) is `crop: None` again: the camera's frame. A quarter turn
  in the Crop tab stores a crop as before, now the camera's frame
  turned.
- **The key (K2).** The develop key is made from the sidecar alone.
  `key::framed_key` hashes the camera frame into the key of an edit the
  frame frames (`geometry::framed_by_camera`: no crop, and not a ratio
  over a centered frame), and only where the frame has one, so every
  other key is bit-identical to before and no picture is made again
  for nothing. The index applies it as it writes a row's key (its
  column); the worker applies it where it keeps pictures. The window's
  thread keys by the frame as known (`key_known`), reading nothing; a
  row not read yet gets the edit's own key as its hint, which the
  thumbnail's read corrects.
- **The index (T2), schema 17.** A `camera_frame` column holds the fact
  as text. The tag pass fills it from the head (`index_read`, and
  `read_style` for rows already indexed). The step from 16 adds the
  column (idempotent: it is added only where missing, so an older
  library chains through it) and marks every Olympus and OM System row
  `style_read = 0`; the next folder pass reads their maker tags again,
  from the head only, and writes the key of any row whose frame
  changed. A re-read that fails writes nothing: the row keeps its
  style and its frame, unread, for a later pass. So the step never
  loses a stored style. The cost: a raw whose tags never read is read
  again (its head) at each pass over its folder.
- **Pictures with no edit.** The thumbnail cache keeps the camera's
  picture whole; the grid tile, the strip, the lightbox, culling, the
  local previews, and the camera's picture standing in for a develop
  are cut to the frame as they are handed out. No recipe bump: nothing
  in a cache is made again. The frame comes from the index's rows (a
  per-path map the rows fill, of `.ORF` and `.ORI` files alone, at most
  65,536 entries, a row read since saying again), or, where a file was
  read for the picture anyway, from its head once a session (a failed
  read kept as no frame). A picture from the cache alone (an offline
  or network root, a cached thumbnail) never reads a head: it stays
  whole until the row is read, and is asked for again then. A picture
  not of the sensor's shape (the camera's JPEG beside the raw, a
  preview a body embeds already cut) is never cut; and the camera's
  JPEG of a framed raw is not kept as its local preview, which the
  pool makes from the raw's own picture.
- **The stand-in.** The camera's picture is cut by the same fractions
  as the develop is framed, so nothing jumps when the develop lands. A
  frame with a crop of its own stands in with the whole picture, as
  its crop was made on, where its sidecar has been read; while its row
  stands in for the sidecar, the crop is not known and it is cut.
- **Export, kept pictures, the viewport, the Navigator.** All frame
  through `Facts::camera()` or the panel's state: `finish::render`,
  the worker's scene, `framed_size`, `fitted_size`,
  `edited_pictures::make`, the reduced develop, the export sheet's
  size, and the viewport's `ViewMap`.
- **Camera match** develops with no camera frame: it registers against
  the embedded JPEG, which is the whole 4:3 area.
- **Copy, paste, sync, presets, virtual copies.** No geometry crosses
  frames (presets leave it out, paste and sync lay sections on the
  target's own edit), so a default resolved per frame needs nothing
  there. A pasted crop is a stored crop and wins.

## What changes for existing edits

On a frame shot at 3:2 (or 16:9, 1:1, 3:4), intended, and their kept
pictures made again once (the key moved):

- An edit with no crop and Original or Free shows, exports, and
  thumbnails at the camera's frame where it showed the whole sensor.
- Original with the Portrait switch over no crop (§666) takes the
  camera frame's shape on end: 2592 by 3888 (2:3) where it was 2916 by
  3888 (3:4).
- A ratio over no crop about a frame off center (none seen from a body
  yet) is fitted about the frame's center.

Unchanged, byte for byte: an edit with a stored crop (refitted or not),
a ratio over no crop about a centered frame (key and all), every 4:3
frame, and every other maker.

## Cost of the schema step

Only Olympus and OM System rows are touched. Each is read again from
its head (the maker note, a few hundred KB at most), no hash and no
decode, on the next pass over its folder; a row whose frame is new has
its key rewritten. A library of 100,000 rows with 10,000 Olympus frames
reads 10,000 heads once.

## Tested

- The reader on a scrubbed, in-test ORF (built byte by byte, no image
  data) under each note header: 3:2 and 1:1 frames; none for 4:3, past
  the area, inverted corners, no tag, another maker, a DNG, no TIFF.
- `CameraFrame` through all eight orientations, its text round trip,
  and a picture cut to it, on end too.
- The geometry: no crop is the frame, a whole crop is the sensor, a
  crop of its own wins, the frame through every turn and mirror and on
  a frame shot on end, an angle fitting the shape inside, Original
  holding the camera's shape, `with_camera_frame` framing alike.
- The key: the frame enters only an uncropped edit's key.
- Export: no crop on a framed frame exports the frame; a whole crop
  exports the sensor.
- The index: a schema 16 library gains the column, its Olympus rows
  are marked and filled, the framed key written, and a second open adds
  nothing; the schema 15 step chains through.
- The viewport frames by the same crop as the export through every turn
  and a turn not yet landed; merges and whole frames get none.
- Crop mode: opens on the band under Original, Done stores nothing, a
  drag stores a 3:2 crop in the band, Reset is no crop and the band,
  4:3 stores the whole sensor.
- The camera's picture handed out cut: the thumbnails' path (grid,
  strip, Navigator) and the previews' path (loupe, lightbox, culling,
  the stand-in), the edit's picture and the camera JPEG beside the raw
  left whole.
- Headless exports, CPU, against master: nine frames that are not cut
  (4:3 Olympus ORF and ORI, CR3, NEF, RAF, DNG, RW2, ARW) are pixel
  for pixel master's. The 3:2 sample exports at 5184 by 3456; against
  master's full export it sits at row 216 exactly, and against the
  camera's 3200 by 2400 preview at row 133 (the band is 133.3 to
  2266.7). On the GPU the same size, about 90 dB from the CPU's; the
  engine's and the UI's tests pass on lavapipe too (the export
  declines a software adapter, so lavapipe is the tests').
- Captures (private weston): the grid tile, the strip, the Navigator,
  Develop at Fit, culling's camera JPEG, and the crop box all at the
  band; the camera JPEG standing in and the develop land on the same
  pixels.

- A stored crop refitted as before; a ratio over no
  crop about a centered frame bit for bit as without one, over four
  turns, two angles, four ratios, and both ways up, and its key kept;
  Original on end over no crop pinned at 2:3; a failed re-read keeps
  the row's style, frame, and mark; a picture the camera cut already
  (its JPEG, a kept local preview of it, a cut embedded preview) not
  cut again; one asked for whole not cut; a crop of its own asks for
  the whole picture; the frame cache read only where asked, a failed
  read kept, other makers' files never read or kept, a row's word
  over a read; the window's key reading nothing; a cached thumbnail of
  an unread row left whole.
- The index's read of the 3:2 sample (the samples test pointed at its
  folder by `GREYCARD_TEST_RAWS`): the frame read from the shared
  pieces after the style equals the frame read alone, and is a frame
  (1 of 1).

## Bounds and refusals

- What rows and decodes told is kept unbounded (a row is in memory
  anyway); only head reads are bounded, at 65,536, emptied past that.
- A decode's frame (checked against its crop) is noted after every
  open, the last word over a row's or a head's. A failed head read is
  not kept. A head read, the index's included, refuses a frame whose
  valid area is not the size the decoder says it develops (where it
  says), as the decode does.
- A row whose tags will not read: warned once a session per file, at
  debug after; a failed read ahead on a share is not made again in the
  same pass.
- The companion check reads nothing of the raw (what is known of its
  frame).
- A row that comes to say a frame drops the loupe's, the stand-in's,
  and the lightbox's camera pictures of it, to be asked for again cut,
  and its cells' thumbnails are asked for again only as they are shown,
  not all at once.
- A frame's shape for the masks is never read off a picture that may
  be cut: known framed, the area's; known whole, the picture's; not
  known, a whole picture's or none (it waits).
- `olympus_frame` refuses numbers not plausibly of one picture: an area
  past IFD0's declared size, or a frame spanning neither the area's
  whole width nor its whole height (every aspect a body offers spans
  one). High Res Shot ORFs are unverified: no sample. Their note is
  taken only where it passes these checks and the decode's area check.
- Ruled out: the stand-in of an edited, angled, uncropped framed frame
  without kept pictures is drawn unturned, as on master, where the
  camera's picture never takes the edit's angle. At 3° the develop's
  fit is 3.2% inside the cut 3:2 picture; on master the same frame at
  4:3 lands 6.4% inside the whole picture. The camera frame halves the
  step and changes nothing else.

## Open

- A merge of 3:2 frames opens at 4:3: a merged DNG carries no note.
- Other makers can fill the same fact later.
- The reduced develop's pictures of a framed edit with an angle sit up
  to a quarter pixel (at 2048) off the full develop's, measured on the
  3:2 sample at bin 2 (ΔE mean 0.97 and 0.87 against 0.67 and 0.81
  with the frame switched off; a quarter-pixel search puts the framed
  cases at (+¼, −¼) and (−¼, 0), the unframed at 0). Exports are
  exact; the kept pictures are previews. Not found yet: the sizes
  agree (5160 by 3440 and exactly half), and resolving the frame at
  each size changed nothing. The sample is past the check's median
  limit with the frame off too (0.67 against 0.5).
