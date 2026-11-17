# §707. Edited neighbors read first for the prefetch (2026-11-17)

A tester's log (discussion 168) showed every step through a folder of
edited frames developing in full. Under the index most frames stand in
from their rows until picked (§207), and the window sent such a frame
to the prefetch (§379) only when its row said it held no edit and no
versions, since only then could it guess the edit. A neighbor with an
edit was left out and logged, so on a folder of edited frames nothing
was ever made ahead.

**The design.** The window now sends such a neighbor marked to be read
first, with the cell's version and a placeholder edit. The worker, in
the prefetch's own idle slot, reads the neighbor's sidecar itself:

1. It asks whether anything at all is queued, and gives way if so.
2. It asks whether the folder may be read (below).
3. It peeks the sidecar (`on_disk::peek_sidecar`): the window's reader
   without a placement, so it takes no Lightroom develop, writes
   nothing, and logs nothing.
4. It takes the cell's line by version, the file's turn, and the seed,
   asks the queue again, and then goes on as for any neighbor.

The window's read at the real open stays the authority. The shelf's
take compares steps (§378, §706), so a sidecar changed after the peek,
or a Lightroom develop only the window's read takes, misses and
develops fresh: wasted work, never a wrong picture. The placeholder
edit is never developed or compared.

**Shares.** The read cannot be interrupted, so the worker never peeks
into a network folder or an automount not yet mounted, where the read
would mount it and wait on a dead server. The window hands the worker
`mounts::not_to_peek`, asked for each neighbor's folder at each read
(about 25 µs on Linux, a read of the mount table; never a touch of the
folder). The path is made absolute and its `..` resolved by name first.
A folder the table does not list is not trusted. On macOS it asks
`getfsstat` with `MNT_NOWAIT`, and skips where the answer is not sure;
the roots' own check is unchanged.

**Also.** A sidecar that is missing, will not read, or lacks the
version is remembered (the latest 64, by the file's length and time)
and not read again until it changes. The prefetch now also gives way to
the masks' picture, a context handed over, a lost device, and masks
whose settling is due; it begins nothing while a lost device waits.

**Measured** (the engine alone, a release build, five 24 MP raws each
with a sidecar, 2.5 s between steps, from the open's send to its
develop delivered): 2,081 ms a step before, 704 ms after, every step
from the shelf. Not measured through the window.

**Tests.** The edit, turn, and seed come from the sidecar, and the open
takes the base, bit for bit a fresh develop's; the folder is unchanged
to the byte and the time, an XMP beside it; a virtual copy's line; a
changed sidecar misses; the give-way before and after the read; a
network folder and an unmounted automount touch nothing; fake mount
tables for Linux and macOS.

**Limits.**
- Untested on a Mac: `getfsstat` itself, how it spells firmlinked
  mount points, and that it never waits on a hung share. On Windows
  `GetDriveTypeW`'s cost on a dead mapped drive is unmeasured.
- A symlink, a Windows junction, or a `fuse.portal` path into a share
  is taken for local and may be peeked.
- Any `fuse.*` filesystem but `fuse.portal` is never peeked, local
  ones (gocryptfs, mergerfs) included.
- A malformed sidecar's versions still log from the edit's own reader
  at a peek.
- Edits under the learned denoiser are not prefetched, as before.
