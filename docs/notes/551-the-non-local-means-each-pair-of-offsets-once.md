# §551. The non-local means, each pair of offsets once

*2026-11-05*

§128's leftover, named again in §164 as the next second. The search
offsets come in pairs, `d` and `-d`, and the patch dissimilarity of a
pixel and its partner does not care which is the center: the pair
`(c, c + d)` weighs `c + d` for `c` exactly as it weighs `c` for
`c + d`. So we walk the first half of the offsets, each over the tile
and the tile moved back by it, and give each weight to both pixels of
its pair that fall in the tile. The zero offset, the middle one, is a
weight of exactly one and costs nothing.

§128 thought taking this meant darktable's scatter, a whole-frame
buffer per offset and the tiling given up. It does not: the region
for an offset is the 96-square tile grown by the offset, at most 7
pixels at the default dense window, so the tiles stay independent and
the rayon split is the same. The differences, box sums, and weights
are made over about 1.08 tiles a pair where they were made over two;
the accumulation is the same count as before, and it is now the larger
share. When the tile and its shifted copy do not overlap (scattered
offsets wider than a tile), each half of the pair runs on its own,
which is the old cost.

**Exactness.** Not bit for bit, and it cannot be cheaply: a pair's box
sums run from different starts, and a pixel takes its offsets in a
different order, so its sum rounds differently. The old function stays
as the test's reference, and over six sizes from 1x1 to 201x150, five
settings of patch, strength, center weight, scattering, and search,
the worst relative difference is 3.1e-6, a few tens of last bits; the
test holds it at 1e-5. On the 45 MP frame, the 16-bit TIFFs differ by
one step on 519,445 of 134 million samples and by more on none; the
8-bit previews on 4,684 of 44.8 million pixels, by one. The synthetic
benchmark's checksum is the same to nine digits. The reference copy
gained a signed check where an offset wider than a tiny frame took
an unsigned bound below zero; real frames never reach it.

**Measured.** A 45 MP frame, release, `taskset -c 0-5` and
`RAYON_NUM_THREADS=6`, before and after alternated five times, median
(min). Other agents were building throughout, load 12 to 16, so the
seconds are high against §164's quiet machine; the ratio is the
figure.

| 45 MP, 6 threads              | before          | after           |
|-------------------------------|----------------:|----------------:|
| means alone, synthetic        | 4.89 (4.64) s   | 3.90 (3.58) s   |
| means in the develop          | 4.72 (4.29) s   | 3.54 (3.24) s   |
| develop, sharpen + hybrid     | 13.46 (11.53) s | 11.44 (10.47) s |

A quarter off the means, about a second; the develop drops by most
of it (a second review's retake under the same load: the means 0.74
of before, the develop 0.93). Not the factor of two §128 hoped for,
because only the weight half of the loop halves. The means are still
the larger half of the hybrid denoise, now by about a second against
the chain's 2.3 s on a quiet machine. The GPU has no non-local means,
so there is nothing to mirror there.
