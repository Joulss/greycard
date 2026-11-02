# §524. rawler at the fork's float-deflate-pin

*2026-11-01*

We moved rawler from the fork's 1766caab to float-deflate-pin
(4e6f0f01): upstream main at 01f3f092, the Deflate float writer
(dnglab/dnglab#896), and the Nikon HE chain behind `jpegxs`. The R6
Mark II white level fix (#840) and the ORF preview (#880) are upstream
now. Four API moves, each ported in place:

- **Black level.** `BlackLevel::as_vec` is gone; the pattern is read
  from `levels`. rawler's black now has DNG `BlackLevelDeltaH` and
  `BlackLevelDeltaV`, which greycard carries as `Levels::black_deltas`
  (the differences and the active area they count from).
- **Color matrices.** `RawImage::color_matrix` moved into camera
  profiles. We read the primary profile's first two slots, one
  calibration per illuminant, and read a slot with no illuminant tag as
  D65, as rawler did before. The DNG specification's default is
  unknown, which would leave such a DNG with no calibration we can use.
  No entry in rawler's camera table has more than two matrices, so the
  table's cameras read exactly as before. Each matrix is now the
  specification's XYZ to camera, `AnalogBalance x CameraCalibration x
  ColorMatrix`, the calibration taken when its signature matches the
  profile's. The forward matrices stay as the file has them, which is
  exact while the calibration is diagonal, as on every file seen.
- **DNG writer.** `raw_image` takes `&CompressionParams` (we pass the
  defaults: predictor 1, as before). `DngWriter::color_matrix` is gone;
  we write `CalibrationIlluminantN` and `ColorMatrixN` on the root IFD
  ourselves, beside the `ForwardMatrixN` we already wrote there, so the
  tags stay the same.

## The black-level decision

A frame without differences develops exactly as before: each pattern
position's black, and a range from that black to white. Where a DNG
has differences, `normalize_levels` takes them off each sample, as
rawler's `rescale` does (a column's difference goes on every row of
that column, a row's on every column of that row). The range then runs
from the highest black that pattern position reaches over the active
area. The DNG specification and rawler scale a whole plane by its one
highest black; we keep a scale per pattern position, the old rule
carried over to the differences. With all differences zero it is the
old rule exactly; with a one-value pattern the two agree, and a test
runs rawler's `rescale` as the reference there.

The new rawler also moves a DNG's `BlackLevel` pattern from the
ActiveArea's corner, where the specification puts it, to the image's,
where greycard's pattern starts. A DNG with an ActiveArea at an odd
offset and a black that is not one value used to get its pattern a
position off; it now gets the right black. No file in the test set has
that shape, so none reads differently.

No camera in rawler's table has differences. Only the DNG decoder reads
them, from the two tags. No raw under the test folder and none of the
three Pixel 9 DNGs carry them (exiv2 finds no `BlackLevelDelta` tag).
So the path is exercised by synthetic tests only: a mosaic DNG written
through rawler with differences round-trips into `black_deltas`, a
written-out per-sample reference, zero differences as none, and
rawler's `rescale` under a one-value black.

The Prepare stage runs only on the CPU (`normalize_levels`). There is
no GPU Prepare to check against.

## Verification

A scratch binary (not committed) read every raw under ~/Pictures/Test
(78 files) through greycard's public decode API, built once at the old
pin and once at the port, both release. Per file it compared: the
camera, dimensions, layout and CFA, a hash of the samples, the black
and white patterns, the as-shot coefficients, the calibrations, the
crop, the orientation, the shot, the DNG tags, the camera JPEG (size,
orientation, hash of the pixels), the probe, the white read, the
stance, the mosaic, and the index read.

| Format | Files | Fields per file | Identical |
|---|---|---|---|
| CR3 | 46 | 18 | all |
| RAF (incl. fuji/) | 22 | 18 | all |
| DNG (Canon via Adobe) | 4 | 18 | all |
| NEF | 3 | 18 | all (2 HE refused the same way without the feature) |
| ARW | 1 | 18 | all |
| RW2 | 1 | 18 | all |
| ORF (olympus/) | 1 | 18 | all |
| NEF HE, `--features nikon-he` | 2 | 18 | all |
| Pixel 9 DNG (target/pixel9) | 3 | 18 | all but the as-shot white and the calibrations (see below) |

EOS R6 Mark II white levels at the port: 14008 (21 files) and 14888
(12 files), none at 12735, the same as at the old pin.

**The Pixel 9's as-shot white now reads as the file's.** These files
give their white as `AsShotWhiteXY`, not `AsShotNeutral`, and carry a
`CameraCalibration` (diag(1.0251, 1, 0.9713) for D65). The old pin made
the neutral from the bare D65 `ColorMatrix`; upstream (fac21dfc) now
puts the calibration in, but still uses one illuminant's matrix, while
our profile interpolates two. Read back through our profile, the
white came out 4369/4724/5426 K at the old pin and 4190/4825/5244 K at
the first port, where the files say 4576/4682/5487 K (the XY's own
temperature). We now put `CameraCalibration` and `AnalogBalance` into
our matrices, as above, and turn `AsShotWhiteXY` into gains ourselves
through our own profile interpolated at that white. As Shot reads
4576/4682/5487 K, tint 0.0069/0.0071/0.0046, the file's exactly, in
the frame and in `white_path` alike. A DNG with `AsShotNeutral` is
untouched by the second change, and a file with no calibration by the
first. In the dump, only the Pixel files' as-shot, calibrations, and
white read differ from the old pin.

**The written DNG**, exported headless (`greycard develop --dng`) from
CR3, NEF, ARW, RAF, RW2, and DNG, lossless and uncompressed: decoded
back, the samples, levels, calibrations, and as-shot white are
identical. The tags are the same except four changes that come from
rawler's writer:

- `DNGVersion` 1.6.0.0 becomes 1.7.1.0, the writer's own version now.
  `DNGBackwardVersion` 1.4 becomes 1.0, which the writer derives from
  the features a file uses; we pass 1.4 as the most it may claim.
- `ActiveArea` is no longer written when it is the whole image (it is
  the default), so the decoded crop reads `None` for a full-frame
  `Some`. These develop the same.
- `RowsPerStrip` is one value (c84bf8dd).
- Adobe's reader and rawspeed both take the missing full-frame
  `ActiveArea` and the `DNGBackwardVersion` of 1.0 (the reviewer
  checked both).
- Some lossless tiles are one byte longer. The LJPEG encoder now pads
  its last byte with ones, as T.81 asks (5dcaf4c1), and a trailing
  0xFF takes a stuffing byte. The samples decode the same.

The fuzz crate checks (`cargo check` in fuzz/).

For raws from cameras, nothing changes that a user can see. A phone's
DNG that gives `AsShotWhiteXY` (a Pixel's) now opens at the white
balance the camera recorded, where the matrix and the neutral used to
disagree by up to a few hundred kelvin. That gets a Fixes line
(changelog.md beside this file).
