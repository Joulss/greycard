# §709. A focus stack read in passes (2026-11-17)

Issue 134: a focus stack held every frame at once, and §573's check
refused 21 frames of 24 megapixels on a 16 GB machine where 30 to 100
frames is ordinary. The frames are now read when the merge asks for
them, and the count no longer matters.

**Where the bytes went.** Measured as peak RSS on real 24-megapixel
frames: 3.54 GB at 5 frames, 7.77 at 20, 19.04 at 60, so 12 bytes a
pixel a frame (each demosaiced frame, held by the caller) over about
2 GB fixed. §573's 40 bytes a frame overstated the first and left out
the second. The core's blend was already one pass with accumulators
(weighted bands, weights, unweighted bands, coverage) and one frame's
working set at a time; only the slice of frames grew.

**Two passes, the same sums.** `stack::stack_camera_from` takes a
count and a reader. The first pass reads in the chain's order, the
reference and outward, scoring each frame and fitting it against the
last that held; it keeps the reference's luminance for the second
direction. The second pass reads the frames the merge uses in their own
order and blends each. Blending in chain order would save a read and
change the order of every float sum; reading twice keeps the result bit
for bit what it was. With the sharpest frame as the reference a scoring
pass comes first, since the chain starts from it: three reads.
`stack` and `stack_camera` go through the same code reading from their
slices, and the merge as it was is kept for tests as the reference.

**Decode again, not spill.** Decode and the short develop cost 0.2 s a
24-megapixel frame against the merge's 1.1. A spill of the demosaiced
frame is 288 MB a frame (29 GB at 100 frames, 72 at 60 megapixels),
written and read at about the decode's speed on a fast disk, and would
want a disk bound and a refusal of its own. The raw files are the
spill. A merge cut short has nothing of its own to leave beyond the
hidden folder every merge already clears.

**The bound.** What is held is one frame's working set: 87 bytes a
pixel in the core, counted by the allocator and the same at 3 frames
and 12; 99 a pixel of RSS at 24 megapixels from 5 frames to 100, 87 at
45. `focus::BYTES_A_PIXEL` is 110, plus a megabyte a frame
(`BYTES_A_FRAME`) for its tags and report, against
`greycard_ai::ram::available`: `MemAvailable` on Linux,
`kern.memorystatus_level` of physical on the Mac, `ullAvailPhys` on
Windows, physical elsewhere. A hundred frames of 24 or 60 megapixels
pass on 16 GB.

| frames | before peak | after peak | before wall | after wall |
|---|---|---|---|---|
| 5 | 3.54 GB | 2.31 GB | 7.6 s | 7.2 s |
| 20 | 7.77 GB | 2.34 GB | 27.9 s | 30.3 s |
| 60 | 19.04 GB | 2.37 GB | 79.6 s | 86.3 s |
| 100 | refused (96 GB wanted) | 2.36 GB | | 143.6 s |

Real 6000x4000 CR3 frames, five frames that link ping-ponged to the
count, release, 32 threads. Every output DNG is byte-identical to
before's, the sharpest reference too.

**Progress.** The first pass says each frame as before ("merging frame
k of n", counted by read), then "placing the frames", then the second
pass says the panorama's existing "blending frame k of n" over the
frames used: §573's leftover of a status line that sat on placing for
the whole merge is answered with no new wording. The CLI's "developed
in" now counts every read, both passes.

**The failure line** is now "this stack needs about X GB of memory and
Y GB is free: close other programs and try again": the count is not
what fills memory, so the old advice to stack fewer frames went, and
the guide says the same.

**Known limits.**

- The bound is stricter than the old one on a loaded machine: two 24
  MP frames with 2.5 GB free on an 8 GB machine passed before and are
  refused now, at a measured peak of 2.2 GB. Chosen because the check
  is against what is free, and the editor needs room beside the stack.
- Frames decoded twice cost more on network storage than the 0 to 15%
  measured locally.
- A frame changed on disk between the passes at the same size is
  blended as it then is; a change of size is refused by the core's
  check.
- Linux ignores cgroup limits, as the old check did.
- The blend's "k of n" counts the frames used, not those chosen.
- The CLI's "developed in" now counts every read, both passes.
- The Mac and Windows readers are compile-checked, not run (roadmap).
- The `ones` plane (4 bytes a pixel) and the expand's temporaries are
  where the fixed peak could come down next.
