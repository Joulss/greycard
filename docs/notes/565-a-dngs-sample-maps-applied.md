# §565. A DNG's sample maps, applied

*2026-11-04*

A tester's Lightroom Classic panoramas opened magenta and blown
while their thumbnails were right (issue 67). The files are DNG 1.7
linear raws, JPEG XL compressed, and their `OpcodeList2` holds three
`MapPolynomial` opcodes, one a plane. We read the opcode lists
ourselves (§474, §513), and of the opcodes we applied the gain maps,
the bad-pixel lists, and the rectilinear warps; `MapPolynomial` was
named in the log and left. Lightroom puts the whole tone and color
correction of a merge's linear data into those three curves, so a
reader that leaves them sees a picture without a white balance or a
tone curve. The thumbnail is Lightroom's own render.

The reader now takes `MapTable` and `MapPolynomial` from
`OpcodeList2` as a `Remap`: the area, planes, and pitches a gain map
has, and a curve, a polynomial by power or a table over the 16-bit
sample. The develop runs each on the linear samples in the list's
order, as the SDK's 16-bit stage does, and holds the result at 0 and
1. The first list's sample maps run on the stored values, which the
develop no longer holds there, and stay left in the log; no file we
have seen writes one.

No panorama DNG was at hand: the develop's tests apply a polynomial
and a table to synthetic frames, and the tester's file checks the
rest when one arrives.
