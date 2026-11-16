# §694. A mosaic's noise line past texture (2026-11-15)

Found while measuring the profiled denoise for §691.

**One frame was a fault.** A Micro Four Thirds frame at ISO 320 with a
mesh filling a band of levels lost 59 % of its grain at 0.1 and a
quarter of the mesh at 0.5. Its model read seven times the variance
its own flat tiles show. The mosaic estimate drew its line weighted
by blocks alone (`Line::Counts`), so whole bins of texture tilted it;
the picture estimator had been given `Line::Relative` for exactly that
(b81ef6ce), and the mosaic took it too, at first. Estimates on frames with flat
areas move by a few percent; that frame's fell from 0.0099 to 0.0040
at mid grey, where the sensor's shot noise puts it. On the benchmark
the estimate over truth at noise 0.01 falls from 2.31 to 1.93, and
CPSNR rises at every noise, 0.06 to 0.29 dB.

**What changes for stored edits.** Every consumer of the mosaic's
estimate reads the new line on a textured frame: the profiled denoise
(weaker there, as meant), the dual demosaic's automatic threshold
(less VNG4, a shade sharper), the hot photosite repair's sigma, and
the learned denoiser's input conditioning and its cache key (a cache
miss once; the network was trained on estimates from the old line,
`denoise_data.rs:242`). Frames with flat areas barely move.

**Two reviews found faults in the relative line on a mosaic; the
weights now take the first power.** Weighted by blocks over variance
squared, every block counts the same in relative error, so the shadows,
where a frame's blocks crowd and the black's own handling makes them
quieter than grain, drew the line: a bracket with crushed blacks read
the red at a third of its grain, and a panorama's red at 0.6 of its
body's. Capping the deep bins by count (a first try) fixed the bracket
but was a constant on the wrong lever and left a night frame at 1.6 to
2.2 times its own flat blocks. Weighted by blocks over variance (the
first power), a bin's say grows with its level, between blocks alone
and relative error, and the line holds the midtones. Then a guard: the
line is drawn again without the bins whose level is under its own sigma
there, where the clamp at black cuts the noise short and tilts the line
up past them. One sigma, not a tuned threshold: 1.5 and 2 left the
high-ISO scenario at 0.84.

Chosen against the reviewer's harness: frames with known noise added
(high ISO, mild, clipped, night; rows under 0.85 / over 1.3 of truth at
0.05 or 0.18, about 266 channels each):

| scenario | blocks (master) | relative | deep cap | first power + guard |
|---|---|---|---|---|
| night | 0 / 218 | 0 / 260 | 0 / 243 | 0 / 64 |
| high ISO | 0 / 5 | 0 / 0 | 0 / 2 | 0 / 0 |
| mild | 0 / 37 | 2 / 2 | 0 / 1 | 0 / 0 |
| clipped | 0 / 12 | 2 / 0 | 0 / 0 | 1 / 2 |

The night rows over 1.3 are extrapolation past the frame's levels
(under 0.016); at its own levels the line is within 15 %. The one clipped
row under is 0.84. On 89 frames from a dozen bodies (267 channels),
flat blocks' p5 reads 4 channels under-estimated (p5 over 1.15) and 12
over (under 0.6), where blocks alone read 2 and 68, relative 44 and 5,
the cap 7 and 6. Against an independent flat-block measure at each
level: the mesh frame 1.03 to 1.07 (its fix kept), the 1 s night frame
1.00 to 1.06, the single-flat-band mirrorless frame 0.98 to 1.24, the
fixed-lens X-Trans frame 1.06 to 1.08 (no longer smoother than its
grain). A panorama's red reads 0.83 to 0.92 of its green and blue. The
night frame's blue reads p5 4.3 above 0.02, a few bright blocks the line
extrapolates to. Benchmark CPSNR at noise 0.01, 0.03, and 0.05: Kodak
34.04, 31.38, 29.23 (blocks 33.83, 31.31, 29.21), McMaster 32.43,
29.97, 27.56 (32.25, 29.91, 27.57).

A third review checked the line on 140 further frames: at ISO 1600 and
above the median over an independent flat-block measure is 0.996, and
bursts of one scene spread less from frame to frame than on the old
line. The night test is read-noise bound (read noise 1e-5, or 4e-6
with shot noise 4e-4), where the line without the guard reads a fifth
and an eighth short at the frame's own levels, and with it under a
tenth.

**What the line still gets wrong.**
- Texture confined to a band of shadows (0.02 to 0.2, a quarter over
  the grain) still reads high: +0.5 to +0.95 at 0.05, less than the old
  line. No line we tried passes it.
- Two X-Trans frames read 10 to 20 % under the grain where the old line
  was close; their denoised renders show no grain left over.
- At ISO 8000 to 10000 on one mirrorless body the line reads 1.3 to 2.4
  times the old one, and rightly: the old line read those frames' flat
  blocks short.

`DEVELOP_RECIPE` goes to 6: the dual demosaic's automatic threshold and
the hot photosite repair read the line by default, so kept previews and
thumbnails are made again. Fills kept on disk go to version 6, made on
the old base they would sit smooth in a grainier surround; the hashed
slots of version 4 are no longer read. Nothing deleted stale fills, so
the first fill lookup in a run sweeps its folder: only `.fill` files
that open with our magic at an older version, so a newer build's fills
outlive an older build run meanwhile. A sweep that let go of every
stale fill leaves a stamp (`fills-swept-6`) and the folder is not read
again; a stamp that could not be written means the next run sweeps
again.
