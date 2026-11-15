# §688. The camera's focus point (2026-11-15)

## What landed

- `greycard_core::raw::FocusPoint` on `Shot::focus_point`: the AF area
  the camera found in focus, as fractions `[x, y, w, h]` of the
  decoder's crop before the orientation, with `oriented` and `center`
  (the orientation math shared with `CameraFrame::oriented`).
- `decode/focus.rs` reads it from Canon's maker note, two ways:
  - at every decode (`decode_source`), on the crop the decoder took;
  - with no decode (`decode::focus_point_path`), for the culling
    loupe's camera picture: one 128 KiB read of the head, a walk of the
    CR3's box headers to the moov's Canon box, its CMT3 (the note) and
    CMT1 (Orientation); a CR2's IFDs through the style reader's guarded
    TIFF reader. The head is read as the whole file (its end the
    file's), and only a walk that needs a byte past it reads the file
    again, a piece at a time: a CR3 whose moov lies in the head is
    answered by it, a point or none. Anything at the path that is not a
    regular file is refused before the open (a FIFO named `.cr3` would
    block it). The crop's size comes from the note's SensorInfo
    (rawler's crop on every file read).
  rawler's BMFF parse is no longer used here: the walk is ours, bounded
  (64 boxes a level, a moov of at most 64 MiB, a CMT box of 1 MiB).
- The UI keeps the head reads (`focus_points`) by path, with the
  file's modified time and length at the read: a file replaced at the
  same path is read again. A read that finds nothing or fails is kept
  as none, so a missing or unreadable file is asked once. Filled on
  the cull decode thread after its picture is delivered (prefetched
  neighbors too, each read once), which wakes the window when the read
  found something new; and on a thread of its own when the zoom key
  asks for a frame whose picture came from the local previews (which
  read nothing of the file): one read a path at a time (a second press
  waits on the first), at most four at once, and a press past them
  waits for nothing. No index or sidecar change.
- The zoom keys, Space (1:1) and Z (3:1), from fit, go to the point
  when one is known: in Develop by the develop's units and the panel's
  geometry; in Cull, the lightbox's loupe, and while the camera's
  picture stands in for a develop, by the camera picture's own units
  (its turns and mirror, never the edit's crop or straightening), the
  point taken into the camera's cut where the picture comes cut. Cull
  drawn developed keeps the Develop path. From any zoom but fit, with
  no point, or in a compare view, the keys are as they were, to the
  letter. Clicks and the middle click are unchanged.
- Late arrival: a key pressed from fit before the picture has said
  where it focused zooms about the middle and leaves a mark (the
  frame, its zoom, and whether in Cull). When the point comes (a
  develop landing, a Cull picture or head read landing) and the view is
  still that frame, at that zoom, centered on the picture's middle, the
  view moves to the point. The mark goes with a pan, a zoom, another
  frame, entering or leaving Cull, and as soon as the source has
  answered for the press, whatever the answer: the Cull picture in hand
  with its head read, or the frame's own develop landed, giving no
  point the view can take (none, or one the crop leaves out), drops it
  then, so no later re-center (a crop changed, a turn) takes the view
  anywhere. It is never made in a compare view, nor once the source
  has said. Checked after each draw lays the frame out, so a picture
  of a new size re-centered by that draw does not fool it.
- A fuzz target, `focus_point`, over `focus_point_bytes` (the box walk,
  the IFDs, AFInfo2), with one synthetic seed. No new dependencies, so
  fuzz/Cargo.lock is unchanged; fuzz.yml's comment counts eleven
  targets.

## Tag layout as verified

AFInfo2, maker note tag 0x0026, int16u array (exiv2 prints it as
`Exif.Canon.AFInfo`):

| index | field |
|---|---|
| 0 | AFInfoSize, bytes (8838 on R5, R5 II, R6, and R6 II; 5472 on R7) |
| 1 | AFAreaMode (seen: 6, 9, 13, and 22) |
| 2 | NumAFPoints (1053; 651 on the R7) |
| 3 | ValidAFPoints (1 for a single area or tracking box; 1053 when the R5 II reports its whole-area grid) |
| 4, 5 | CanonImageWidth, CanonImageHeight |
| 6, 7 | AFImageWidth, AFImageHeight |
| 8.. | NumAFPoints each of widths, heights, X, and Y (X and Y signed) |
| then | AFPointsInFocus, AFPointsSelected: (N + 15) / 16 words each, LSB first |
| rest | 67 words (42 on the R7), all zero on every file seen; unread |

SensorInfo (0x00e0): elements 5 to 8 the cropped area's left, top,
right, and bottom, inclusive; on every file it is rawler's crop.
Only the first ValidAFPoints areas are read. Manual focus is
CameraSettings (0x0001) element 7 = 3, 6, or 16, or AFAreaMode 0;
neither was seen on a file (no MF frame in the set), so that branch is
tested on synthetic bytes only. Source: ExifTool's Canon tag docs
(AFInfo2, SensorInfo, and CameraSettings FocusMode), cited in the
file header. The reviewer's sweep confirmed the layout, the bit order,
and the bounds on 60 real CR3s.

## Coordinate chain

1. AF area center (X, Y) is from the AF image's center, Y up.
2. Scale by CanonImage / AFImage (1:1 on every body seen). Refused when
   the two axes scale differently by over 1%.
3. The Canon image sits centered on the decoder's crop; on every frame
   CanonImage = AFImage = rawler's crop = SensorInfo's cropped area =
   the camera's full-size JPEG. Refused when the Canon image differs
   from the crop by over 1% either way: an aspect or a crop mode chosen
   in camera may come with an AF image of another picture, and none of
   the 60 files has one to check against.
4. Fractions of the crop, turned by the EXIF orientation: what the
   develop's picture, and the camera's JPEG as Cull turns it, are
   before an edit.
5. Develop: turned as `camera_crop` turns the camera's frame (the tag
   after the frame's quarter turns, with the develop's lag), to source
   pixels, `Geometry::to_plane`, minus the frame's origin:
   `develop.center`'s units. `view_to_source` run backward, the mapping
   a click and a mask point use. A point the crop leaves out falls back
   to the view's center.
6. Cull and the lightbox's loupe (`cull::loupe_focus_center`): into the
   camera's cut where the picture comes cut to the camera's frame
   (`CameraFrame::shown`), to the JPEG's own pixels (`Preview::source`),
   through `Geometry { turns, flip }` of the frame's thumbnail turns,
   as `draw_cull` lays the picture out. The edit's picture standing in
   has no point (it is cut by its edit).

Not mapped: the lens correction's warp in Develop. The develop's source
is corrected, the point is the sensor's; at 1:1 near a frame's edge on
a strongly distorting lens the zoom lands a little off the point (the
AF box is hundreds of pixels across, so it stays inside). No helper
maps a point through the warp today. Cull's JPEG is the camera's own
rendering, which these bodies correct in camera or not as set; on the
frames checked the JPEG and the develop line up to the pixel.

## Cost of the head read

All 59 CR3s in the test folder, release build: first read in the
process (the head off the disk) median 0.53 ms, max 0.90 ms; warm,
median 0.008 ms (6 us the open and the 128 KiB read, 2 us the walk and
the parse), max 0.016 ms. It runs on the cull decode thread, beside the
read of the picture, or on a thread of its own; never the window's.

## Per body

All five bodies: Y up, positions against the AF image which equals the
Canon image and the crop. Checked by 1:1 crops at the point and at the
point mirrored about the sensor's horizontal axis (Y down): the point
lands on the sharp eye or subject on every frame, the mirror off it.
For Cull, seven frames across the five bodies (four portrait) cropped
from the camera's JPEG as Cull decodes it, beside the develop at the
same point: the same pixels on all seven.

- R5 (2 frames, mode 6, Eye AF box): on the eye.
- R5 Mark II (4 frames, mode 13): tracking box on the subject; two
  frames report the whole 39 by 27 grid with ~30 points in focus.
- R6 (1 frame, mode 9, landscape): on the cliff, sharp.
- R6 Mark II: eye boxes on the eye. The 432 by 432 box at (0, 0) is on
  27 R6 Mark II frames, AFAreaMode 13, which is 1-point AF with the
  point at the center (the default position), not a whole-area
  default: the photographer's centered point, and where the picture
  is sharp on the frames looked at.
- R7 (1 frame, mode 9): on the eye.

## Several in focus

Of the in-focus areas, the one nearest the frame's center. On the
whole-area frame of two people on rocks, the in-focus set is the
people plus a band of the rock they stand on (same distance) running
to the frame's bottom; the mean of the set lands on a knee, the
nearest-to-mean point too; nearest the center lands on the top row,
level with the two heads (the area itself is on the dark rock between
them, which is soft: an AF area is a coarse cell, and the camera
marks it in focus for the hair crossing it). A 1:1 view of the window
holds both heads there; the knee choice holds neither. Always an area
the camera called sharp, never a gap between subjects. A better
target (a face inside the in-focus set, or the set's top cluster) is
for the overlay slice, where the set can be drawn whole.

## Nothing in focus

Two R6 II AI Servo frames have a selected area and no in-focus bit.
We give no point there (the camera did not confirm focus; on one, a
still life, the selected box sits beside the subject). The keys then
zoom about the middle as before.

## Unverified

- CR2: read paths written and tested on synthetic bytes; no CR2 at hand.
- Older bodies' AFInfo (0x0012) is not read; AFInfo2 only.
- An aspect ratio other than 3:2 set in camera, and a crop mode (APS-C
  on a full frame body): refused, not read, until a file shows what
  AFImage and CanonImage are there.
- Manual focus flags on a real file.
- A Canon DNG (Adobe converted) carries the note in DNGPrivateData: not
  read.
- A companion JPEG beside the raw in Cull is taken as the camera's
  picture of the crop (cut to the camera's frame where there is one);
  not checked on a real pair.
- The late-arrival rule's "about the middle" is a test of the center
  against the picture's middle within half a pixel: a pan that ends
  exactly there counts as untouched.
- No run of the app itself; the UI paths are covered by tests.

## Upstream

rawler parses the CR3 maker note into a TIFF IFD (`Cr3DescBox.cmt3`)
and hands it out; it does not name 0x0026. An upstream change would be
small: a `CanonMakernote::AFInfo2` tag constant and a parsed
`RawMetadata` field (af image size, areas, in-focus and selected masks)
filled by the CR2 and CR3 decoders. The head read here would still be
ours: rawler's `FileBox::parse` walks every top-level box to the end
(and needs the mdat), where the head read stops at the moov.

## Next slices

- Other makers, each in `decode/focus.rs` beside Canon's, as fractions
  of the crop, each with a head read for Cull: Nikon AFInfo2 (0x00b7,
  versioned, AFAreaXPosition/YPosition against AFImageWidth/Height, Z
  bodies), Sony (FocusLocation 0x2027, or Tag9400's FocusPosition2),
  Fujifilm (FocusPixel 0x1031, against the RAF's full size), Olympus
  (AFPointSelected in CameraSettings 0x0305, fractions), and Panasonic
  (AFPointPosition 0x004d, fractions). Each wants its own orientation
  and crop check on real files; `may_have_focus_point` takes their
  extensions as they come.
- Culling overlay: `loupe_focus_center`'s chain on the area's corners
  gives the rect in the loupe's units, then the view's; in Develop,
  `ViewMap::to_view` on the oriented rect. `focus_points` already holds
  the point for every frame Cull has read; the overlay could show all
  the in-focus areas, which the reader would then hand out whole.
- Focus score at the point: `sharpness` measures the frame; it would
  take the rect (fractions of the crop, oriented) as a region. The
  score is indexed, so taking it there changes indexed values and
  wants a version marker for a rescore.
