# §556. A sync's first read of unindexed frames, off the window

*2026-11-05*

§322 left one read on the window's thread: a sync carrying the
demosaic onto frames the library has not indexed yet read each one's
mosaic there (`decode::mosaic_path`), once a session. A folder opened
before its first index pass finished paid it on the first sync.

**The read goes first.** The sheet's sync, once the sidecars are in,
asks which frames it would read (`sync::mosaics_unread`: the source
when the demosaic goes, and the targets too when its method, resolved
for the source's mosaic as the sync resolves it, is not every
mosaic's; while the source's own is still to be read, every target is
read with it). None, and it runs as before, nothing more. Some, and
`rows::after_mosaics` queues the sync at the front of the requests,
ahead of any behind the one running, reads the mosaics on a thread of
its own, keeps them through `browser::learn_mosaic`, and runs the sync
on the window's thread when they land. The status says "reading N
frames...", the roots view's line, while the read is out (the count
takes the source in). The memo is the same one; a second sync reads
nothing.

A failed read is now remembered for the session, as unreadable, so no
sync reads it again on the window; the old `mosaic_of` read such a
file again at every sync. A sidecar read landing while the mosaics are
out leaves the bar to the mosaics' landing, and a bar with nothing
queued stops rather than giving up on nothing.

One cost moved rather than went: a mosaic read that hangs, with no
sidecar read out, holds the request queue with no give-up. Before, it
froze the window.

**Measured**, a debug build, 21 copies of one CR3 (the source and 20
targets), no index, the old path run first and the two alternating:
the window's longest stall was the press, 318 ms on a cold read and
16 to 30 ms warm. Now the press takes 0.3 to 0.5 ms and the sync, once
the mosaics are in, 3 to 5 ms; the read, 13 to 20 ms warm (333 ms
cold, when it ran first), is on its own thread
(`measure_a_sync_onto_unindexed_frames`, ignored, folder in
`GREYCARD_SYNC_FRAMES`).

Left on the window's thread: a paste and a preset onto unindexed
frames still read mosaics there, and a sync's `probe` of a frame's
body (a named profile, a seed) still reads its metadata there.
