# §586. The Canon EOS R8 Mark II

*2026-11-07*

A tester's R8 Mark II frames would not open: "Unknown camera" (issue
85). Two things were missing in rawler. The body had no camera file,
and its CR3 carries COLORDATA version 67, which rawler's Canon reader
had no arm for. Version 67 has the layout of 64 to 66, the R5 Mark
II's, R50 V's, and R6 Mark III's: exiftool reads all four with one
table, with the white balance at 0x69, the per-channel black level at
0x17f, and the white levels at 0x294 and 0x295. The version joins
that arm, and the camera gets its file.

The color matrix is the EOS R8's. None for this body is published
yet, in Adobe's converter, libraw, or rawspeed, and the sensor is the
same 24 MP 6000x4000. The tester's craw frame at ISO 1600 decodes
with a black level of 2048, a white level of 14888, and a 6000x4000
crop, and its white balance and color match the camera's JPEG. When
a matrix is published the camera file takes it, upstream.

Both are in rawler (dnglab/dnglab#937, on the fork's pinned branch
until a release carries it). The sample stays with the tester; they
are asked for one for rawler's raw database.
