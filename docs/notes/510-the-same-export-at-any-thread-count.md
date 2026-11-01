# §510. The same export at any thread count

*2026-11-01*

From the engine track (§506): one bar run beside a release build
moved some 3800 samples of one frame by one 16-bit level. The
suspects named were rayon sums whose order follows the work-stealing.
They were not it; AMaZE was.

**Reproduced by thread count.** Exports of two frames (a 24 MP CR3
and a 45 MP CR3, 16-bit TIFF, headless) at RAYON_NUM_THREADS of 1, 2,
3, 4, 7, 8, 16, 32, and the default, pixels compared after
decompressing. At the default edit all were equal. With Texture,
Clarity, Structure, Dehaze, and Defringe on, the 24 MP frame at 32
threads differed from one thread in 114,284 samples, each by one
level, in rows 0 to 3110 and columns 133 to 3999 of the 4000 by 6000
export; 16 threads gave 112,724 there, 2 to 8 none. The thread count
where it starts depends on the frame: 8 on another 24 MP CR3, 32 on
an ARW. Bisected by section: Texture alone 310 samples, Clarity 2794,
Structure 10,181, all one level, all gone under RCD, VNG4, or
RCD+VNG4, still there under AMaZE alone and with noise reduction,
sharpening, or lens corrections off. Load alone did not move it: 32
spinning processes, or two other exports beside it, gave the idle
run's bytes three and four times over. What split the bar run's
work differently is not known; that the splitting decides is.

**AMaZE read the last tile's leavings.** Its tiles run under
`map_init`, a scratch per rayon job, and each tile cleared six of its
26 buffers. The reference's loop bounds read a few cells no pass of
the tile writes: rows and columns 2, 3, rr1-4, and rr1-3 (the third
pass at ±2, the fourth at ±3). In a full tile no tile ever writes
them, so they stay zero. A tile cut short by the right or the bottom
edge reads cells inside a larger tile's extent, and they held what a
larger tile run before on the same scratch left there; at one thread
nearly always its left neighbor or the one above, at more threads
whatever the splitting put first. RawTherapee has the same buffer,
allocated once a thread and never cleared but for the Nyquist mask,
so under OpenMP's dynamic schedule it reads the previous tile's
leavings too. Neither computes those cells, and zero is what a new
buffer holds, so the fix is to clear them, not to bound the reads:
each tile now zeroes every buffer, and the module's "every buffer
starts zeroed" holds per tile. That costs 1.63 s against 1.55 s
before on a 45 MP mosaic at four threads, within the noise.

**What moves.** The float output of the short tiles changes at any
thread count: 81 samples on the 24 MP CR3, 19 on the ARW, none on the
45 MP frame, whose width is a multiple of 128. After 16-bit rounding
no sample of either CR3's default export moved at one thread. A bar
run against an older master may show one-level samples at the right
and bottom tile edges with Detail on. After the fix every export
above matched one thread to the bit, under load too. The CA
correction's scratch, the same pattern, reads nothing it did not
write when filled with garbage first; non-local means and the wavelet
denoise clear or overwrite theirs per tile.

**The sums anyway.** A parallel float sum's last bit does follow the
splitting, and it reaches the pixels through a threshold; that it
crosses an f32 rounding only rarely is luck. `develop::sum` cuts a
sum at fixed places (65,536 values a piece), sums each piece in
order, and adds the pieces in order; rows that sum themselves are
added in row order. Every parallel reduction in the develop, the
render, and the engine:

- Sharpen's plane mean (the auto threshold): an f64 sum. Fixed.
- Defringe's mean deviation (the threshold): an f64 sum. Fixed. Its
  count over the threshold is an integer: exact. Its chroma before
  and after, for the stats the CLI prints: rows' f64 sums in a tree.
  Fixed.
- The highlight reconstruction's chrominance offset: rows' f64 sums
  reduced in a tree. Fixed.
- Dehaze's transmission mean: rows' f64 sums in a tree, for the log
  and the stats only. Fixed, since a log that differs is a false
  lead. Its minimum is order-free.
- The dual demosaic's and sharpen's flattest tile: a least with ties
  to the first, which rayon keeps in order. Order-free.
- VNG4's ceiling and sharpen's radius measure: a most. Order-free.
- Noise estimation's blocks: collected by row, summed serially.
  Already fixed.
- Local contrast, the guided filter, refine edge, registration, the
  stack, dehaze's box means and minimum filter: per pixel or per row,
  no sum across tasks.
- greycard-render and greycard-engine: none.

The helper's tests hold a vector's sum to the serial one to the bit
in pools of one and four threads. AMaZE has two: a short tile run
after a full one on the same scratch comes out as on a new scratch
(158 and 4 samples apart on the old reset), and any tile run on a
scratch filled with garbage does too, which also reaches the cells a
full tile over-reads.
