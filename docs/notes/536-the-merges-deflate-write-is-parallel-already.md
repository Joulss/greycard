# §536. The merge's Deflate write is parallel already

*2026-11-02*

§525 left "the Deflate write on more threads." It is on them already.
The writer is rawler's (`dng_put_raw_deflate`, private): it compresses
the strips with rayon's `par_chunks` on the global pool, so
`RAYON_NUM_THREADS` holds, and writes them in order. A three-frame
panorama (39.9 MP) at 4 threads spends 2.76 s in rawler's
`raw_image` on 9.3 s of CPU; at 1 thread 7.6 s, the file identical
byte for byte. The synthetic bracket (45 MP, 152 MB): 2.49 s on 9.1 s
of CPU at 4 threads.

What remains is libflate's speed, about 30 MB of half floats a second
a thread. A faster Deflate changes the bytes, and belongs upstream.

Memory is not bounded: rawler collects every compressed strip (the
file's size, 130 to 152 MB) before writing, and our float copy of the
image (0.44 to 0.54 GB) is whole. Neither moves the peak, which the
walk and the blend set (3.76 GB and 3.21 GB). A bounded window of
strips is a rawler change: `raw_image` takes no compressed strips, and
tagging the IFD by hand would need its private backward version.
