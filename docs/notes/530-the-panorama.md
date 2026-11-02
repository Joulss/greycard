# §530. The panorama

*2026-11-01*

Phase 2 of the merge design (§523): frames shot turning the camera,
stitched into one linear DNG.

**The frames.** Walked as the bracket's (§525, §529), every frame's
highlights reconstructed. A frame of another size or focal length is
refused before its walk. The lens profile is the develop's, read off the
reference; its vignetting runs in the walk in camera space, and its
distortion is the first term of each frame's one map onto the canvas.
Each frame is walked once and spilled.

**The name held.** Both merges hold a hidden folder beside the output,
`.<name>.merging`, and an exclusive lock on `lock` in it for the run's
life. A held lock refuses a second merge; a free one means the last run
was cut short, and we clear what it left and take the folder over. It
holds the file until its rename and the spilled frames; on the way out
the lock is closed before the folder goes, as Windows requires. We
dropped the HDR merge's sweep of a stale `.part`, which could delete a
running merge's file.

**Pairs.** Harris corners, Lowe's ratio, RANSAC; then the homography
fit, a fourth model in `register.rs`. We take a fit within 40 px of its
features and under `PANO_RESIDUAL`, 0.25 on the fit's scale: a stack's
bound is 0.05, but a handheld turn moves the lens, and the parallax
leaves residuals one homography cannot take up (0.12 to 0.16 here; a fit
in the wrong valley comes in past 0.3). `greycard register --model
homography` runs it, as the command runs every model: our harness.

**Placing.** Every rotation and the focal length fitted to every pair at
once, leveled (the plane of the frames' axes across the turn sets the
cylinder's axis), and centered. Planar to 90 degrees, a cylinder past
it; with no focal length or sensor size, homographies on a plane.

**Blending.** Overlap gains, minimum-cut seams, Lanczos-3, and
Burt-Adelson bands over the largest covered rectangle. The lens's
inverse runs in f64: in f32 one step at 6000 px is 5e-4 px, and a stop
at 1e-4 px refused a sixth of a full frame's edge. A distortion folds
back past some radius; the warp takes nothing past the outline's
farthest point, and the inverse refuses a root past the fold.

**The file.** `<first>-pano.dng`, the reference's EXIF less its lens.
The tag says `lens=corrected` only when a correction ran; then the
develop's profile stands down, and `export -v` logs it. With no lens
database, no profile, or the Lens section off, the frames are stitched
uncorrected and the run says which. The edit is the reference's look:
less exposure, the learned denoiser, the lens section, geometry,
retouch, and masks.

## Measured

The three frames: 24 MP raws through a 16 mm lens, handheld, one
exposure. Release build, four threads.

| pair | corners | matches | inliers, px | fit residual, moved | placed |
|---|---|---|---|---|---|
| first, middle | 1388, 1399 | 305 | 247 at 3.23 | 0.162, 9.9 px | 8.1 px |
| first, last | 1388, 1398 | 142 | 102 at 2.82 | 0.136, 16.8 px | 5.7 px |
| middle, last | 1399, 1398 | 315 | 267 at 3.02 | 0.122, 6.3 px | 11.6 px |

Placed residuals are parallax. Focal length 2667 px by EXIF, 2563
fitted. A cylinder spanning 147.4 by 78.0 degrees; 6080 by 2840 cropped
(17.3 MP); headroom 1.37 stops. Without a lens database the fits fail
the leash (75 to 264 px) and the frames are laid flat, uncorrected.

| stage | seconds |
|---|---|
| walk the frames (three) | 5.12 |
| features | 0.25 |
| pairs: match and fit (three) | 2.66 |
| place | 0.00 |
| exposure, seams, and crop | 0.99 |
| resample and blend | 3.94 |
| write | 1.13 |
| total | 14.3 |

Peak resident 2.20 GB, bands 0.37 GB. Seam detail against Lanczos's:
cubic 0.913, bilinear 0.762. The prototype's two tiles: 0.068 px, gain
1.1764 against 1.1765. The learned denoiser adds 2.6 s (fast) or 4.4 s
(balanced).

## Left

- The canvas cap, 100 MP, until a merge in strips.
- A sphere, and the uncropped canvas.
- A merge killed midway leaves its spilled frames (288 MB each at 24
  MP) until the next merge to that name.
- Two merges to one name started within microseconds of each other,
  as one run's folder goes: the second's lock file can be removed
  with it. A check after locking that the file open is the one at the
  path, and a retry, would close it.
- The learned denoise on by default once the model store offers the
  tier without a fetch; today it runs when the reference edit has one.
