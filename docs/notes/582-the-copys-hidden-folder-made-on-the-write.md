# §582. The copy's hidden folder, made on the write (2026-11-06)

The status bar said "22 edits waiting for Archive" and kept saying it.
Each row in the pending list (§233) had been tried up to a hundred
times, every one with the same reason: not written, no such file or
directory. The archive was mounted, each copy was there, and its
folder too.

The sidecars were set to live in the hidden `.greycard` folder
(0.1.2). A save makes that folder beside the frame every time, and
hides it. The archive's write (§244) did not: `Sidecar::write_to`
staged its temporary beside the target and renamed it into place,
assuming the folder. On a shoot copied to the archive before the
folder was, or made there by another tool, every write to it failed,
and the row was tried again at each save and each archive answer, for
good. Sidecars beside the raw never saw it: the folder is the shoot's.

**The fix.** `write_to` makes and hides the hidden folder when its
target is under one, as a save does, through the same helper. Every
caller is covered at once: the mirror of a save, the join's write of
the settled copy, the take of theirs onto the local frame, and the
archive's join. The pending rows go through on their next try, which
the next save or the next archive answer brings.

Tests: a write into a missing hidden folder makes it and lands; a
write beside makes nothing.
