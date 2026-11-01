# §498. Many sidecars saved off the window's thread

*2026-11-01*

A sidecar save syncs its file before the rename (§373), about 5 ms
a file on this machine's NVMe and more on a share. A paste, a sync,
or a preset over a few hundred frames did that on the window's
thread, one file after another: 1.6 s held over 299 frames here,
and longer wherever a sync costs more.

**A batch of its own.** The window still makes every save in memory,
each sidecar advanced to its new revision, and hands a copy of each to
a thread of the batch's own. The thread writes and syncs every file
(and its XMP) under a temporary name first, then renames each into
place on its own: a file either lands whole or keeps its old sidecar,
whatever happens to the rest. The window marks the files saved (the
index told, the edit's pictures kept, an archive copy queued) only
when the batch lands, and finds each by path in case the list moved
meanwhile. The window is held about 10 ms for the same paste, plus a
few for the landing. One file alone saves as before.

**A newer save wins.** Each file a batch still holds is claimed by
path. A save on the window's thread (an undo, a rating, a second
paste) takes its file back from the batch first, so the batch renames
nothing over it and the newer save is the one on disk. A stress run
of paste, immediate undo, and a second batch over the same files, 50
times, left no stale mark.

**Who waits.** A move or a delete of frames, a folder read, a look
rename, and the quit wait for the batches out. The quit's wait follows
progress rather than a fixed clock: it gives up only when a batch has
not moved (a write or a rename) for 5 s, so 800 frames on a debug
build land in 18 s, and a share that has stopped is given up in 5. A
batch out at the quit is finished before the index closes, so the
index hears of the landing. Two limits are known and accepted: the
wait is bounded per stalled step, not in total, and the camera-time
read an XMP may need on the batch's thread has no timeout of its own,
on a thread that already writes to the same share without one.

**The rejects queue** goes through the same synced write as a
sidecar, the last file §373 left unsynced that matters.
