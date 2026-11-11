# §632. Lossless WebP export (2026-11-11)

Issue 121 asked for WebP. We added it as a fourth format, lossless
only: the pure-Rust encoder, image-webp 0.2.4 (MIT OR Apache-2.0,
already in the tree under resvg), writes VP8L and nothing else. Lossy
WebP is libwebp's, a C library, and waits.

We call image-webp straight rather than through the image crate. The
image crate's encoder takes a profile and EXIF but no XMP, and the
export's own account of itself (source, edit, settings) lives in the
XMP. image-webp takes all three and writes them as the container's
ICCP, EXIF, and XMP chunks, in the order the spec asks. So a WebP
carries what a PNG carries, under the same metadata policy and embed
switch, and `exif::with_xmp` stays the JPEG's and the PNG's.

The image crate's `webp` feature, which brings the decoder, stays off
in the build: only greycard-render's tests turn it on, to read the
files back. No WebP is read as a picture, and the library lists none,
since its extensions are a list of their own.

A WebP side holds at most 16383 px: libwebp's `WEBP_MAX_DIMENSION`.
image-webp writes 16384, which libwebp's encoder would not; we refuse
past 16383 so every reader takes what we write. The refusal is the
encoder's own error, "Invalid dimensions", so a too-large export fails
as any write fails, with the frame named in the status line, and
nothing is resized behind the user's back.

The sheet shows WebP as it shows PNG: 8 bits, no quality, the color
space and its profile. A preset keeps the format by name, so an older
build reading "WebP" falls back to JPEG, as it would have for an
unknown PNG.

**Checked outside our own reader.** The round-trip test reads back
with the image crate, which is image-webp again, so the files were
also read with ImageMagick (libwebp), vips, ffmpeg, and exiv2 in
every color space, with and without the profile: the pixels equal the
PNG's, the chunks are in order with the container's flags set exactly
when a chunk is there, and each metadata setting gives a WebP what it
gives a JPEG and a PNG. No browser was tried, and no export past
16383 px on a real frame.

**Left as it is.** The refusal's words are the encoder's alone; a
border can push a picture that fits past the limit; and the frame is
developed before the write is refused. `greycard develop`'s preview
writes JPEG and PNG only.
