# §606. The GPU dual's detail stays on the device (2026-11-07)

§591 left three causes for the gap between the AMaZE dual's 247 ms
and AMaZE's own 90 at 45 MP on an RTX 5070 Ti. We took all three.

## The decision

**AMaZE's result stays on the device.** AMaZE's batches copy their
band into a frame of interleaved RGB kept on the device (`Frame` in
`demosaic.rs`), in buffers of whole rows as many as the device's
largest buffer takes (256 MB by wgpu's default, so three at 45 MP),
copied to and from, never bound. Each band of the blend starts from a
copy of its rows there, halo and all, in place of the upload from the
host; only the blend's result comes back. The halo bookkeeping on the
host is gone: the band reads the frame's own values, so its interior
is what the whole frame gives, as before. AMaZE's outer ring of three
pixels, bilinear on the CPU as the reference's, was filled on the host
after the read back; it now goes up packed (`border_ring` in
greycard-core, next to `fill_border` and checked against it to the
bit) and a last kernel, `border`, writes it into each band, so the
result on the device is whole. AMaZE alone reads back the same values
as before, the ring included, and no longer makes its pass over the
frame on the host.

The automatic threshold's search still reads AMaZE's result on the
host, so under `Tiles` the result is read back once for the search,
and the blend overwrites that buffer row by row. Under the noise
model's or a fixed threshold, the default's case in the editor,
AMaZE's result never comes back.

**RCD runs once for the RCD dual under the automatic threshold.** The
threshold's run of RCD reads its result back for the search and copies
it into a frame on the device as well; the blend starts from it as the
AMaZE dual's does, skipping RCD's kernels. Under the other thresholds
RCD already ran once, inside the blend's bands.

**The bands' staging is made once a run and double-buffered**, as
AMaZE's is: two buffers a band's interior, the next band submitted
before the last one is unpacked. Before, every band made its own and
waited for it.

The engine needs no change: the four ops keep their signatures.

## Measured

On lavapipe (llvmpipe, 4 threads), which shares the machine's memory,
so the bytes moved cost a memcpy and not a bus: these are shapes, not
a card's numbers. Best of three warm, 45 MP synthetic frames:

| 45 MP on lavapipe | before | after |
|---|---|---|
| AMaZE dual, noise model (the default) | 2605 ms | 2606 ms |
| AMaZE dual, automatic threshold | 3210 ms | 3032 ms |
| RCD dual, noise model | 1707 ms | 1529 ms |
| RCD dual, automatic threshold | 2404 ms | 1977 ms |
| RCD alone | 422 ms | 395 ms |
| VNG4 alone | 1267 ms | 1188 ms |

The CPU reference's times in the same runs moved by up to 10 percent
between the two, so a few percent either way is the machine's load.
What lavapipe does show: the RCD dual under the automatic threshold
lost its second RCD (a fifth of its time), and the double-buffered
staging takes a few percent off every banded op. What it cannot show
is the first item, which is the bus: on lavapipe an upload is a copy
within memory. The real card's number is the parent's to take.

Bytes moved a run at 45 MP (8256 x 5504; MiB, from the timing's new
counters after, and from the code before):

| | up before | up after | back before | back after |
|---|---|---|---|---|
| AMaZE dual, noise model | 907 | 357 | 1040 | 520 |
| AMaZE dual, automatic | 907 | 357 | 1040 | 1040 |
| RCD dual, automatic | 366 | 366 | 1040 | 1040 |
| RCD dual, RCD, VNG4 | 183 | 183 | 520 | 520 |

So the default moves 1070 MiB less a frame (three fifths of its upload and half
its read back). Under the automatic threshold the bytes are the
search's and stay; the RCD dual's gain there is compute.

## Memory

On the device at 45 MP, before: AMaZE's working set (the mosaic
texture 182 MB, its planes 180 MB, its band and two stagings 38 MB),
about 400 MB, then the bands' 268 MB and one staging of 94 MB made
and dropped per band: a peak near 400 MB. After: the frame, 545 MB,
lives through both. AMaZE's set without its stagings, 375 MB, is
waited for and let go before the blend makes its own (dropped while
still queued, the driver would hold it into the blend's), so the two
never meet: 920 MB during AMaZE, then the bands' 268 MB and two
stagings of 94 MB beside the frame, a peak near 1.0 GB. At 102 MP
(11648 x 8736) the frame is 1.22 GB and AMaZE's set 680 MB, the
bands' unchanged: a peak near 1.9 GB, during AMaZE. An 8 GB card
holds either. On the host, the AMaZE dual's peak is one frame as
before (it was the result and one halo); the RCD dual under the
automatic threshold held two frames and now holds one.

## Smaller changes

A frame under 16 pixels a side is now `Unsupported` from the AMaZE
dual before AMaZE runs, where it used to come back as AMaZE's result
when the threshold was zero (and `Unsupported` otherwise). The walk
runs the CPU's demosaic on either, so nothing a user sees changes.

## Verified

- `cargo test -p greycard-gpu` on lavapipe (GREYCARD_REQUIRE_GPU set):
  every test of `tests/demosaic.rs` and `tests/amaze.rs` green, bounds
  unchanged.
- Bands equal the whole frame to the bit for RCD, VNG4, the RCD dual
  and the AMaZE dual, now also under the automatic threshold (both
  duals checked to blend there), with the frame on the device split
  into buffers of 37 and 100 rows so a band's copies cross from one
  buffer to the next, and with AMaZE in batches of one row of tiles so
  the frame is written from every batch and the ring at every offset.
- AMaZE's ring on the device is `fill_border`'s to the bit, for the
  four Bayer phases at 41x37 and 301x270, in one batch and in batches.
- The AMaZE dual at 20x20, where AMaZE's result is the CPU's bilinear
  put on the device, holds the dual's bound.
- `border_ring` scattered is `fill_border` to the bit (greycard-core).
- `cargo fmt --all --check`, `cargo clippy --all-targets` with
  `-D warnings` on the workspace.
- The ignored real-frame tests of `tests/demosaic.rs` and `tests/amaze.rs`
  against ~/Pictures/Test (read only) on lavapipe, release: both pass,
  the AMaZE dual at most 8 of 134 M samples past 1e-3 of the scale.

## Not verified

- The peak device memory on a real card: computed from the buffers'
  sizes, not read off the device (the card's times are below).
- The 102 MP peak, measured: it is computed from the buffers' sizes.
- A device whose largest buffer is under a frame's rows of 12 bytes a
  pixel (a frame row wider than 22 M pixels): never met.

## Measured on the card

The same `timing` test on the RTX 5070 Ti, warm, best of three, the
CPU on 32 threads under the evening's agent builds (so the CPU
column runs high against §591's table):

| 45 MP | GPU before (§591) | GPU after | up / back per run |
|---|---|---|---|
| AMaZE dual, the default | 247 ms | 180 ms | 357 / 520 MB |
| AMaZE dual, automatic threshold | | 222 ms | 357 / 1040 MB |
| RCD dual | 137 ms | 94 ms | 183 / 520 MB |
| RCD dual, automatic threshold | | 208 ms | 366 / 1040 MB |
| RCD alone | 134 ms | 86 ms | 183 / 520 MB |
| VNG4 alone | 127 ms | 87 ms | 183 / 520 MB |

At 24 MP the default dual is 109 ms warm and the RCD dual 61 ms.
The default's first run after the context is made pays the
pipelines, 677 ms at 24 MP, once.
