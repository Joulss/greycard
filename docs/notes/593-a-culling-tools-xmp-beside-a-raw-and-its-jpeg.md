# §593. A culling tool's XMP beside a raw and its JPEG (2026-11-07)

Issue 31: a folder shot RAW+JPEG and culled in FastRawViewer or Photo
Mechanic opened here without its ratings. Both tools write `IMG.xmp`
beside `IMG.CR3` and `IMG.JPG`, and the rule in §124 says a short name
shared by two files is nobody's, since either could own it and each
would write over the other. Lightroom's develop in such a file was taken
already (§535); a culling tool's rating fell under the older rule.

**Read, never owned.** What a frame owns is unchanged: the short name
is its own only when nothing else in the folder has its stem, now
compared without case as the pairs (§332) and the rejects move compare
it. Owning decides what is read as the frame's XMP, where a save goes,
and what a move carries. Beside that, `xmp::beside` finds a shared
`IMG.xmp` that the raw reads but never writes, when the frame is a raw,
every other file of the stem is a JPEG, and the packet is a culling
tool's. Two raws of one stem (a DNG made from the CR3) could each be
the one rated, and any other file of the stem, a `.THM` or a `.WAV`,
refuses too: a read lost, never a write. A rating given here goes to
`IMG.CR3.xmp` with the setting on, and the shared file keeps its bytes.

**What a culling tool's packet is.** An allow-list, because the file
is the JPEG's as much as the raw's and a property we do not know could
be the JPEG's edit. Every property is `xmp:Rating` or `xmp:Label`,
one of them at least, or bookkeeping and the shot's facts: `xmp:`'s
`MetadataDate`, `ModifyDate`, `CreateDate`, and `CreatorTool`;
`xmpMM:`'s three identifiers; `dc:format` unless it is `image/jpeg`,
the one fact that names the JPEG outright; `tiff:Orientation`,
which FastRawViewer writes on a rotation and which we leave
untaken; Photo Mechanic's color class, tag, prefs, and version; and
`photoshop:SidecarForExtension` when it names the raw's extension. A
`crs:` develop, a keyword, a caption, `xmpMM:History`, a field of ours,
or a marker naming the JPEG, and the file is nobody's. A rating-only
packet is read as the raw's by design, though the JPEG could own
it in principle: Adobe's tools write a JPEG's metadata into the
JPEG itself, so a sidecar beside a pair is the raw's in practice.
The list is built from the tools' documentation, not from their
files; the reporter's sample will tell whether it is wide enough,
and a property we did not list costs the read, never the file.

**No clocks.** The first build read the newer of `IMG.CR3.xmp` and the
shared file, and was wrong twice. A copy that resets times, a tie at
a filesystem's two-second grain, or a culling tool rewriting its file
for a field of its own (Photo Mechanic's tag, FastRawViewer's label)
made the shared file win, its old rating went over ours, and the next
save wrote it into our file. So the shared file has a mark of its own
in the `.gcd`, `xmp_shared`, apart from §124's mark for the frame's
own XMP: what the file last said for the rating and for the label,
each. A field is taken only when the file says something else for
it than the mark, whatever its time and whatever else in it changed;
a label changed there moves the label alone. A field the tool drops
is a field cleared, and a rating taken off a reject there takes
the reject off here. A shared file that goes away leaves its mark,
so one that comes back unchanged is not taken again.

**Meeting the file first.** Every `.gcd` from before this build has no
mark, and an older build (0.10.0 on another machine, or the archive
side of a two-places pair) drops it on its next write. With no mark,
a field is taken only where the frame's is still empty (no stars
and no reject, no label), and every field is marked, so a rating
given here is never put back by a file it has not met. The issue's
case, nothing rated here yet, takes both. The same rule holds for a
frame with our own `IMG.CR3.xmp` and no `.gcd`, which is read afresh,
unmarked, each time. No schema bump: the field is additive, defaulted,
and read loosely, as §25 and §127 added theirs.

**The two tools do not see each other.** The culling tool reads
`IMG.xmp`; we write `IMG.CR3.xmp`. A rating given here does not reach
the culling tool, and stands here until the culling tool changes its
own, when its new word is taken.

**The index reads once.** A pass calls `beside` once per frame,
folds the shared file's bytes into the row's hash behind a marker of
their own, and hands the parsed packet to the summary's adoption,
so the file is listed, read, and parsed once a pass. The per-frame
listing of the folder for a short name predates this.

**Case.** Stems are compared without case now, so on a case-sensitive
disk a pair whose names differ only in case counts as shared. An
`IMG.xmp` we wrote there before is refused by the allow-list (it
carries our flag) and left orphaned; its meta lives in the `.gcd`.

**Known limit.** A raw moved alone (pairs listed apart, or moved
without its JPEG) leaves the shared file with the pair, as the move
always has, so the moved raw shows its culling rating only if a save
had already put it in the `.gcd` or in an XMP of its own.
