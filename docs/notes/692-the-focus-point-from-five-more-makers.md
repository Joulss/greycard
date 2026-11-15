# §692. The focus point from five more makers (2026-11-15)

§688 read Canon's focus point and took Space and Z there. This reads
Nikon's, Sony's, Fujifilm's, Panasonic's, and Olympus and OM System's.
## What landed

- `decode/focus.rs` dispatches by the file's head: a CR3 to Canon as
  before; a RAF to `focus/fujifilm.rs`; a TIFF raw by IFD0's Make to
  Canon's CR2 path, `focus/nikon.rs`, `focus/sony.rs`,
  `focus/panasonic.rs`, or `focus/olympus.rs`. A DNG is refused for
  every maker, as before.
- Each maker reads to a `Spot`: the area as fractions of the picture
  the maker measures in, and that picture's size (its shape). The
  decode takes the spot on its crop when the crop has the same shape
  (within 1%), else none; the head read (Cull) takes it as fractions of
  the maker's picture. A point with no size is drawn as a square 1/16
  of the shorter side. The same on-picture checks as Canon's: center
  within the frame, size above zero and no larger than it.
- `may_have_focus_point`: cr3, cr2, nef, arw, raf, rw2, orf, ori.
- The style reader's guarded TIFF reader, JPEG-head reader, and
  Olympus sub-IFD helpers are shared (made `pub(super)`); no new parse
  of untrusted bytes outside them but the AFInfo2 record, the RAF
  metadata walk (at most 256 entries, held to its section), and the
  rational values.
- The fuzz target covers all of it; five built seeds (one a maker, no
  image data, no serials), from `fixtures.rs`'s `write_fuzz_seeds`.
- Guide: the Space and Z sentences name the file types read (CR3,
  CR2, NEF, ARW, RAF, RW2, ORF). NRW is not read, so "a Nikon raw"
  would not be true.
- The style reader's JPEG-head reader indexed two bytes of a JPEG
  said to be 0 or 1 long (a RAF's header, an RW2's JpgFromRaw count):
  a panic reachable from the fuzz entry, fixed with a length check.
  A stable test mutates each seed 3000 times with a fixed generator
  and holds the fuzz target's checks, so this class runs in `cargo
  test` (it fails on the old reader).

## What each maker stores, and what we read

| maker | where | read | units |
|---|---|---|---|
| Nikon | NEF Exif maker note ("Nikon\0" + own TIFF), AFInfo2 0x00b7, versioned | 0100 and 0101 contrast detect (AF image at 0x10 / 0x46, in focus at 0x1c / 0x52); 0300/0301 with AFCoordinatesAvailable (0x2a); 0400 to 0402 with AFCoordinatesAvailable (0x3e) and FocusResult 0x4a = 1 | AF image pixels, center and size, top left origin |
| Sony | ARW Exif maker note (bare IFD, file offsets) | FocusLocation 0x2027 [w, h, x, y], FocusFrameSize 0x2037 [w, h, valid] (UNDEFINED, three words) | pixels of a 7008 x 4672 picture on the A7 IV, top left |
| Fujifilm | the RAF's embedded JPEG's EXIF maker note ("FUJIFILM", LE IFD) | FocusPixel 0x1023 [x, y]; FaceElementPositions 0x4203 and FacePositions 0x4103 as the area when one holds the point | pixels of the JPEG's PixelX/YDimension (4000 x 3000 GFX, 4416 x 2944 X) |
| Panasonic | the RW2's embedded JPEG (IFD0 0x002e) EXIF maker note | AFPointPosition 0x004d (two rationals, fractions), ImageWidth/Height 0x004b/0x004c | fractions |
| Olympus, OM System | ORF maker note, Camera Settings 0x2020 | AFPointSelected 0x0305 (5 srationals, first unused, then l t r b fractions), AFAreas 0x0304 (bytes in 255ths) as the area holding it; Image Processing 0x2040 CropWidth/Height and AspectFrame | fractions of the valid area |

## None (no point) for

- Nikon: phase-detect viewfinder records (grid points; no per-body
  grid layout held), auto area or 3D tracking before a subject
  (coordinates not filled, zeros), FocusResult or contrast-detect
  "in focus" 0, FocusMode "MF"/"Manual", CropHiSpeed cropped size
  below the full (DX, 1.2x, 1:1, 16:9 in camera), versions we do not
  know (Nikon 1's 0200 included).
- Every maker: a point at the top left corner, x = y = 0 (a field
  not filled: Nikon's AFInfo2, Sony's FocusLocation with no valid
  frame or with one, Fujifilm's FocusPixel, Panasonic's 0/1 0/1), and
  one at or past the far edges, x >= w or y >= h (Nikon's AFInfo2 all
  0xffff and Sony's FocusLocation of four 65535s read as (1.0, 1.0);
  Panasonic's and Olympus's fractions of 1).
- In the loupe, every maker, Canon too: a picture of another shape
  than the one the point is of. The loupe takes the camera JPEG beside
  the raw when its long edge is at least 95% of the raw's, and a frame
  shot at 16:9, 1:1, or 4:3 beside a 3:2 point would have put the
  fractions on the wrong picture. `FocusPoint` now carries `aspect`,
  the maker picture's width over height before the orientation;
  `loupe_focus_center` turns it by the tag, takes it through the
  camera's cut where the picture is cut, and refuses beyond 1% of the
  picture's own shape.
- Sony: FocusMode 0x201b = 0 (manual), the picture's middle with no
  valid frame (the camera's stand-in for no point), a FocusLocation
  more than 2% off the raw sub-IFD's size (APS-C crop on full frame).
  Most DSC compacts write 0 in 0x201b whatever the focus (ExifTool
  reads it only for the interchangeable lens bodies and a few DSC
  models), so a compact's ARW reads as manual focus: a false none,
  never a wrong point. Not seen on a file; no compact sample.
- Fujifilm: FocusMode 1 (manual), FocusWarning 1, CropMode nonzero,
  the JPEG of another shape than the RAF metadata's cropped size
  (0x111; an aspect chosen in camera), and the exact middle pixel
  unless a face or eye box holds it. In single point AF, 10 of 17
  GFX100S II frames the review sampled write the exact middle, and on
  some the middle is soft: a couple the face detection found elsewhere
  (FocusPixel (1999, 1499), the couple sharp), a figure from behind
  with no faces found, a lens held against water. That is focus and
  recompose: the camera writes where the point sat, not where the
  subject was. A wrong point is worse than none, so this costs every
  real centered focus on Fujifilm too: a frame truly focused with the
  middle point gets no point, and Space and Z zoom about the middle as
  before, which is that same place. It costs the X series too: about
  11% of one X-series body's frames the review read sit on the exact
  middle. The eye and face cases (the pixel inside a box) keep their
  point. The middle is half the width and height rounded either way,
  or a pixel short, so an odd size's two middles both count.
- Panasonic: FocusMode 2 (manual), a position outside 0 to 1 (the
  "none" and "n/a" values), ImageWidth/Height of another shape than
  IFD0's sensor size (an aspect in camera).
- Olympus/OM: FocusMode 10 (MF), FocusProcess 0 (AF not used),
  AFSearch 0 (not ready), a selection not fractions or all zero, and
  an AspectFrame smaller than the valid area (an aspect chosen in
  camera; which picture the fractions are of there is unmeasured).

Several points: Nikon, Sony, Fujifilm, and Panasonic give one point.
Olympus's AFAreas may list several; the one holding the selected
point, the smallest such, is the area. Fujifilm's face and eye boxes:
the smallest holding the point (the eye over the face).

## Verified, by body

Visual check: a crop of the embedded preview about the point, the
area drawn, turned by the file's orientation (in
`target/focus-check/` of the worktree, not committed). Head read and
decode agree on every file decoded. The review then decoded 161 files
with head and decode agreeing (91 mirrorless Nikon frames, 8 Sony, 8
Panasonic, 53 Fujifilm, 1 OM), read 280 more heads (the HE NEFs this
build does not decode among them), and ran 147 hostile files with no
panic.

- Nikon Z6III (4 frames, AFInfo2 0401, 3D tracking): on the eye of a
  portrait, on a boatman's face, on hanging tags and a lantern front
  in two street scenes. Three of the four are HE raws this build does
  not decode; their head reads only.
- Sony ILCE-7M4 (1 frame, face tracking, portrait turned 270): on the
  eye.
- Fujifilm GFX100S II (3 frames, portraits turned 270): two on the eye
  (the eye box from FaceElementPositions), one none (above). Where a
  face box (not an eye's) holds the pixel, the area is centered on the
  pixel with the box's size: the box's own middle sat about 100 JPEG
  pixels off the camera's point.
  X100VI, X-E4, X-T5 (one frame each, zone AF, no faces): on the
  locomotive, the verge of a road, a ridge; plausible, not sharp-
  checked against a mirror.
- Panasonic DC-S5M2 (1 frame, portrait turned 270, face found): on the
  eye.
- OM System OM-5 (1 frame, S-AF, turned 270): on the window and sill
  of an office, about 3 m (the note's focus distance 3.07 m); a deep
  depth of field makes the frame sharp nearly everywhere, so the
  check is weak; the mirrored point scores sharper on our crude
  measure (it falls on the tree outside). Taken as ExifTool documents
  it (left, top first); one frame, one body.

## Lens correction

The zoom's geometry does not take a point through the lens
correction, as before this change (§688, for Canon). Fujifilm's and
Panasonic's points are measured on the camera's JPEG, which the
camera corrects; Nikon's, Sony's, and Olympus's (and Canon's) on the
sensor. Either way the AF area is far larger than the warp's shift
except at a strongly distorting lens's edge.

## Not verified

- Nikon DSLR versions 0100 and 0101 (contrast detect), 0300/0301 (Z6,
  Z7, Z50, D780): built bytes only. A Nikon DSLR viewfinder point:
  none, not read.
- Any second Sony, Panasonic, or Olympus/OM body.
- Crop modes and in-camera aspects on every maker: refused, never
  seen.
- Manual focus on a real file of any maker but by its flags' documented
  values.
- Fujifilm Wide/Tracking AF, Sony tracking without a face, Panasonic
  without a face.
- NRW, PEF, SRW, 3FR: not read. Pentax: no sample.

## Read sizes and timing

Release build, files in the page cache, `focus_point_path`: first read
in the process, then the median of 20.

| maker | head (128 KiB) enough? | what lies past it | first | warm |
|---|---|---|---|---|
| Nikon NEF | yes (Exif at 35 KiB) | nothing | 0.03 to 5.6 ms | 0.003 to 0.004 ms |
| Sony ARW | no | the raw sub-IFD (~135 KiB), for the size check | 1.2 ms | 0.011 ms |
| Fujifilm RAF | yes for the note; no for the size check | the RAF metadata after the JPEG (about 2 MB on the GFX) | 0.04 to 4.9 ms | 0.008 to 0.013 ms |
| Panasonic RW2 | yes (the JPEG at 6 KiB) | nothing | 4.7 ms | 0.006 ms |
| OM ORF | no | Image Processing (~907 KiB), for the area's size and AspectFrame | 0.05 ms | 0.010 ms |

We kept the 128 KiB head: past it the walk reads only the few small
pieces it needs (an IFD, an entry), piece by piece, as the CR3 walk
does; a larger head would read more for the makers it suffices for
and still miss the RAF metadata and ORF Image Processing. The
RAF metadata and the ARW's raw IFD are read only after a point was
found, so a frame with none costs the head alone. A second run, the
files cached: first read 0.5 to 1.2 ms, warm 0.003 to 0.013 ms, every
maker.

## Upstream

rawler does not expose these tags; a `RawMetadata` focus field per
decoder would be the shape, as for Canon in §688.
