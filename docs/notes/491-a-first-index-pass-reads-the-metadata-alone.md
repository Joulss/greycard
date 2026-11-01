# §491. A first index pass reads the metadata alone

*2026-11-01*

§472 put a limit on the library's reads over a network and left the
cost: a first index pass read about 2 MB a CR3, its head with the
preview, a 1 MiB tail for a 120 KB record, and the style reader's own
walk, and every byte of a NEF, ARW, RAF, DNG, ORF, or RW2, the whole-
file fallback for the mosaic. A page-fault trace showed rawler needs
about 240 KB of a CR3 and under 300 KB of the others.

**A sparse file.** rawler is handed the file's length in zeros with
only the pieces read into place, in 4 KB blocks, each run of missing
blocks one read, the 64 KB head first since the content hash reads it
anyway. By format: a CR3's box headers and `moov` through rawler's
own box parser, then the CTMD sample at the offset its track table
gives; a TIFF's IFD chain, sub-IFDs, Exif, GPS, Sony's private block,
and every value stored outside its entry, the maker note always
whole, any other value over 256 KB skipped, the walk bounded to 512
IFDs and 64 pointers an entry with bytes copied only for pointers it
follows; a RAF's header, its JPEG's EXIF, its directory, and the CFA
TIFF. The probe, the mosaic, and the style come from one pass over
that buffer; the style streams the real bytes it asks for, so it
never sees a zero it did not request, and a JPEG's EXIF is found by
walking its segments rather than reading its first 160 KB. Any
error or panic, in the walk or in rawler, falls back to reading the
file whole, as before; a crafted head that trips rawler's box parser
is caught (its oversized sample-table allocation aborts on the
whole-file path too, rawler's to fix).

**Shown equal.** On the 78 sample raws of eight formats every index
row is byte-identical, and rawler's metadata from the pieces matches
the whole file with the unread bytes filled with 0x00 and again with
0xFF; a CI test over a 160x120 linear DNG checks the pieces against
the whole and the fallback. Distinct bytes a file: CR3 192 KB, NEF
259 KB, RAF 95 KB, ARW 136 KB, DNG 152 KB (a Pixel 9's 105 KB), RW2
64 KB, an ORF 1.1 MB since its preview sits inside the maker note. A
cold first pass over 78 files: 0.73 s against 2.97 s one at a time,
0.40 s against 0.82 s four at a time; 33 MB a file became 239 KB.
Nikon HE files read 260 KB and store the same unknown mosaic they
did. CR2 is untested and reads whole.
