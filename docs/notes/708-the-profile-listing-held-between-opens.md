# §708. The profile listing held between opens (2026-11-17)

The third part of discussion 168. On every open the window walked the
whole camera-profile folder on its own thread and emptied the
loaded-profile cache: 0.65 to 0.71 s with the window busy on a tester's
4,439 profiles, and the cause of §706's shelf misses.

**The design.** `greycard_edit_io::profiles` now holds the listing: the
entries, a generation, each folder's modification time, and the headers
already read, by a file's length and time. The window shows the held
listing on an open. A look for changes runs off the window's thread:

- at every open, pick, sync, paste, and preset: the folders' times, and
  the length and time of the files shown for the camera and the one
  chosen, so a profile added or rewritten in place is seen;
- when the CAMERA PROFILE section opens: a whole walk, which reads
  again only the headers that changed.

One look runs at a time. A request made meanwhile is merged into the
next (`LookQueue`), and the queue marks itself idle in the same locked
step that finds it empty, so none is stranded. A listing that differs
is shown when it comes; it takes no history step and writes nothing.
The loaded-profile cache lets go of a profile only when its file
changes (its stat taken outside the cache's lock) or the cache is past
its count.

**The tone curve switch.** With the listing no longer read on the open,
a profile an edit names may not be in the list yet. The switch is edit
data (`read_edit` stores it), so: a profile not in the list keeps the
edit's own setting, and only a listed profile known to have no curve,
or the file's own, clears it. While the start's listing is awaited
(`ProfilesKept::awaiting`, cleared when a listing is shown or the read
ends) nothing is called missing and the switch stays shown.

**Measured** (4,400 synthetic profiles in 440 folders, NVMe, a debug
build): the window's thread at an open, 47 to 62 ms before and 3.7 to
4.0 ms after; the look behind it 0.2 to 0.3 ms; the section's whole
walk 39 to 55 ms, off the window's thread.

**What went wrong on the way**, for the next change here: the first
version cleared the tone curve switch while the listing was pending
(an edit changed by a slider move), and a later one held the queue's
lock through each pass, a `MutexGuard` made in a `while let` condition
living to the end of the loop's body. The tests now lock inside the
pass with `try_lock`, and a stress test of the queue ran under review.

**Limits.**
- Before the start's listing lands, a sync, paste, or preset waits for
  the rest of the start's walk, no longer than the walk it used to make.
- A header changed in place for another camera's profile is seen at the
  section's walk or a restart; it matters only under Show all cameras.
- For the moment between an open and its look coming back, a profile
  rewritten without its curve can still take the switch; the next step
  after the listing clears it.
- Not noticed: a link broken at the walk that resolves later, a folder
  that becomes readable, a change inside one timestamp tick on FAT or
  HFS+, and a share's attribute cache (a few seconds on SMB and NFS).
- The threaded path is tested through its queue and its showing, not in
  a running window; Windows, macOS, and a hung share are unchecked.
- `looks::list()` still walks the look folder on every open.
