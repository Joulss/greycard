# §472. The library over a network

*2026-10-31*

Two library items from the roadmap, one of them a bug seen after a
catalog import wrote two roots.

**The root that was skipped.** The check before a pass counted a
root whose look was still out as not answering, at once. When the
roots change, the watcher's build looks at every root, and the
indexer asks a moment later; on a loaded machine the first root's
look was still out at that moment, so its pass was skipped, while
the sibling added a moment later had a fresh look of its own. Now a
read that finds a look still out waits the rest of its own wait,
counted from when the look was asked, so a hung share still costs
each read one wait at most; and the look before a pass tries
twice, six seconds in all. The race explains the incident but was
not reproduced; a test with a look that answers late covers the
mechanism. The check lives in the window, where the roots' looks
are, not in the library.

**Reads with a limit.** The window tells the library which roots are
network mounts. On one, each file's reads (stat, hash, probe, and
sidecar) run together on a thread of their own, a listing on its
own, with a twenty-second limit. A read past the limit is recorded
as an error, the row kept as it was, and asked again on the next
pass; after one, the pass starts no new reads on that root and
records the rest as not read, so a sleeping NAS costs a pass one
limit, not one per file. A timed-out read never decides a move: the
carry, the stat behind a move, and the folder-in-use check skip the
file and leave the decision to the next pass, so a sidecar is never
orphaned and a move never becomes a copy. Reads given up on are
counted per root across passes, since a thread stuck on a hard
mount returns only when the kernel does, and a root with any still
out starts none until one comes back.

**Several at once.** The first phase of each file is read ahead,
four at a time, the answers taken in order, so the rows are what a
local pass writes. Measured on the archive's SMB mount: a first pass
is bound by the disks' seeks (a cold 64 KB read is about 28 ms) and
no faster with more at once, eight and sixteen slower; a pass over
rows already current is round-trip bound and two to three times
faster at two or more. Four, for shares further away than a LAN.
The content hash was already the first 64 KB and the size. The
listing takes each entry's type from the directory, a stat only
where the filesystem gives none. What a first pass really pays is
the probe in core, about 2 MB a CR3; that is a roadmap line.
