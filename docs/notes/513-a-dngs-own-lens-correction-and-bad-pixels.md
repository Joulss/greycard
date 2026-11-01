# §513. A DNG's own lens correction and bad pixels

*2026-11-01*

The rest of §474's opcodes but the ProfileGainTableMap, built against
the same three Pixel 9 files and for every DNG that carries them: a
Leica Q's, and an Adobe-converted file from a body with a built-in
profile, carry a `WarpRectilinear` too. None of the three Pixels has a
bad-pixel list. The wide's `OpcodeList3` holds one `WarpRectilinear`;
the ultra-wide's and the front camera's two `WarpRectilinear2` (a set a
color for the lateral CA, then the distortion) and a `WarpRectilinear`
after them.

**Both warps, and the rule between them.** `WarpRectilinear2` (DNG 1.6)
is the same mapping with up to fifteen radial terms in every power of
r (these files stop at r^10), a valid radius range, and a reciprocal
flag; `WarpRectilinear`'s four terms are its even powers to 6. A file
may write its distortion both ways, and the SDK's rule is ours: a
`WarpRectilinear` straight after an applied, optional
`WarpRectilinear2` is the older readers' copy and is skipped. Reading
only the first kind would have dropped the CA correction and fitted the
distortion with a degree-6 stand-in: on the ultra-wide the copy is off
the real warp by about 100 px at the corners. The parse refuses a plane
count other than one or three, a byte count not the planes', a center
off the image, a valid range not rising within 0..1, and any NaN, and
keeps at most four warps (a crafted thousand would cost minutes). The
mapping is the specification's formula as Adobe's SDK reads it
(`dng_filter_warp`), no SDK code: whole-pixel coordinates over the
active area, the radius relative to the farthest corner and held at 1,
in f64.

**Where it runs.** A stage of the engine's list, the file's warp, on
camera RGB after the demosaic and the profiled denoise (which reads the
demosaic's noise as measured) and before the move into the working
space: the camera's own geometry, before the turn and the crop. After
the bin in the reduced develop, a binned pixel at its block's middle;
after the learned denoiser, on both of its pair. Consecutive warps are
one map: each pixel traced back through every warp and read once,
bilinear (the SDK resamples after each, bicubic), so the two-warp files
pay one resample and soften once. Both of the ultra-wide's warps take
0.37 to 0.40 s at four threads in a release build. Core's develop, its
reduced develop, and its demosaic (what the linear DNG writer stores,
which carries no opcode list) run the same op, and a walk is core's
develop to the bit.

**The file has the last word on its lens.** The lens section is on by
default with the profile's distortion and CA, and lensfun has entries
for bodies whose DNGs carry a warp (the Leica Q2 and its kin). Under
the file's own lens a warp takes the profile's distortion, and a warp
with a set a color takes its lateral CA and the automatic CA stage too,
which would otherwise move red and blue a second time (up to 0.85 px at
the ultra-wide's corners): the file's correction is the authority, as
Lightroom takes it. The profile's vignetting stays, since
`FixVignetteRadial` is not applied; a lens picked by hand, and the
corrections set by hand, keep all they ask for. The command line's
develop takes the same rule.

**Bad pixels.** `FixBadPixelsList` and `FixBadPixelsConstant` from
`OpcodeList1` (sensor coordinates, the constant against the stored
value) and `OpcodeList2` (the active area's, the constant against the
value as it stands, scaled to 0..65535), on the cropped mosaic before
the noise is measured. The two lists run as one ordered pass with the
gain maps, the first's then the second's in file order, so a constant
before a gain map is tested on the values before the gain. A bad
photosite takes the median of the good ones of its color within two
photosites, or four, from the frame's own pattern rather than the
opcode's Bayer phase, so X-Trans is served and a cluster never feeds
itself; the SDK's repair averages fixed Bayer neighbors instead. A list
after the demosaic is left, as the SDK refuses one on more than one
plane.

**Checked against the phone.** Each file's embedded preview was matched
against our export, scaled to it, by phase correlation on a grid of
64-pixel patches, an affine fit taken out. The front camera's preview
is warped: our warped export meets it at 0.15 px RMS against 1.15 px
unwarped, at the center and the edges alike. The ultra-wide's is warped
too: 3.84 px RMS warped against 7.19 unwarped (outer quarter 10.6
against 22.0), and the preview's radial displacement against the
unwarped export follows `WarpRectilinear2`'s nonlinear part bin by bin
(measured against predicted: −5.22/−5.12, −3.52/−3.28, 2.21/2.18,
2.82/2.80, −14.02/−13.86 px), not the `WarpRectilinear` copy's (+9.7 px
where the real warp says −96). The wide's preview is not warped (0.13
px unwarped against 0.59 warped); Adobe's reader applies `OpcodeList3`
whatever a preview did, and so do we.

**Cost.** DEVELOP_RECIPE goes to 5 and THUMB_RECIPE to 4, so every kept
picture and preview-less thumbnail is made again once, not only a
DNG's: a key of the file's own would thread the frame's facts through
some 65 call sites and rekey every row. One bump covers the warp, the
lens rule, and the CA rule together, for phones, Leicas, and converted
DNGs. A crop, a spot, or a brush already placed on a warped DNG moves
with the picture: up to about 96 px at the ultra-wide's corners, plus a
4 % zoom.

**Left.** The warp on the GPU; demosaicing a margin past the default
crop, since the warp reads only the crop and where its source falls
outside (about 1.9 px at the wide's corners) the edge repeats as a
faint smear, where the SDK reads the active area's 8 px border (both
roadmap lines). `WarpFisheye`, `FixVignetteRadial`, the
ProfileGainTableMap, and §474's neutral.
