# §511. Carried sets' leftovers, and a file away while its folder is renamed

*2026-11-01*

Two of the library track's leftovers: what §493 left in the carried
sets, and §334's member moved outside every root (§486's "not done").

## What carried sets leave behind

**A write that answers late.** A folder whose write does not answer
in its ten seconds is kept on the set's record as late, beside the
folders written, since that write may finish after. The rule: a late
folder still the set's is written again on the minute and listed
when it holds the same; one the set's frames have left has its copy
taken out, which removes only a copy ours has seen (the same or
behind); a deletion is carried to it as to the folders written. A
late folder also keeps the launch's write from being skipped. We
remove no finished copy merely because our record does not list it:
another machine's copy of the same set is such a copy, and its own
record lists it. The one exception is a copy a deletion here has
seen, which a pass now buries where it finds it, whoever wrote it.

**Part files and deletions.** A part file is judged by its age
alone, even when its final name exists: that is also what another
machine's write looks like between its write and its rename, and
removing it fails that rename. The passes already list
`.greycard/sets`; each now names the part files older than a day
(`PART_STALE`) and the deletions older than 180 days
(`GONE_KEPT_FOR`), and the window hands them to the writer, which
asks each its time again and removes it under the folder's limit, a
folder that does not answer tried again on the minute. A deletion is
read for its shape first, and a later build's is left alone, as
§493 leaves every later build's file. A removal refused (a read-only
share) leaves the rest of the folder to be tried, and is said once a
session. The launch's pass clears what an earlier session left.

A day, because a write is waited on for seconds but may finish hours
later on a stalled share, and another machine's clock may be hours
off; a part file is a few kilobytes. Half a year, because no machine
says what it has read, so "every root has seen it" cannot be told
from the folder. A machine away longer can write the set back, and
the machine that deleted it, which keeps its last 512 deletions,
buries that copy again when a pass meets it (above). Both ages are
the file's time on the share against this machine's clock. A clock
more than half a year off clears deletions early: each machine that
reads a deletion learns it before it goes, but a third one away
longer than what is left misses it. A clock a day off can remove a
part file under a write; that write's rename fails, and the minute's
retry writes it again.

**A rename and the destination's stack: left.** Back up does not
carry a rename. It finds the renamed frame in the destination by its
hash, leaves its name, and never renames or writes over there, so
the destination's stack copy names the source's new name until a
hand renames the file there too. Re-pointing the member by hash on
either side would be cheap but never settles: a stack's revisions
hash its members' file names (§493), so the re-pointed side makes a
revision the other side, whose folder has the other name, points
back, a revision each pass for good. One way out makes a stack's
revision without the names wherever the hashes tell its frames
apart, a change to every carried stack's lineage. The other has
Back up, finding a frame in the paired folder under another name,
rename the destination's file and its sidecars to the source's,
which bends Back up's rule that nothing on the destination is
changed; the call is the user's.

## A file away while its folder is renamed

Most of §334's case already held, and a test now shows it: a stacked
frame moved out of every root keeps its place in the stack's file,
not shown, its row missing (with the marker under the missing
filter); moved back into its folder under any name, the index's pass
finds it by the missing row's hash, a move, so the row, the sidecar,
the stack, and the collections follow. Stacks have no Forget
missing, so the member waits.

What failed: the file back after its folder was renamed while it was
away. Its missing row stayed under the old folder, which is gone, and
a row whose folder is gone is never a move's other end (an unmounted
drive looks the same), so the file was a new row, without its edit,
while the stack, having followed the rename (§486), named it in the
new folder at its old name.

We weighed two fixes. The first had the stacks look for a lost
member's hash among the files come live in its folder and take that
path. It moved the path alone: the sidecar stayed behind, so the
frame came back without its edit and a version member stayed hidden;
it needed its own guards against pulling in a copy that was always
there and against re-pointing a destination's stack; and it was a
query by hash for each member on the window's thread. The second, taken,
works in the index: when a pass matches a folder rename, the rows
still missing in the old folder move to the new one at their names,
where no row is. §486 kept a gone folder's rows where they were
because a vanished folder may be an unmounted drive, but a rename
matched by its files (more than half of each side's, by name and
hash, within ten minutes) is as near to not that as the index can
tell: a shoot copied off a drive before the drive is ejected still
reads as a rename (§486 accepts it), and its missing rows then wait
in the copy's folder until the drive comes back, when they return
as moves. A file of it found later in the new folder, under any
name, is then the index's own move: row, sidecar, stack, and
collections follow with no stack code. A file that returns in the
same pass as the rename is matched (all three steps while the
editor was closed, one tree pass at launch) is still a new row
beside the moved one, as before; merging the two by hash in that
pass is left. A row
whose name the new folder has stays in the old one, missing. A member
found in another folder is still a move away from its stack's folder
and leaves its stack, as §334 has it.

## Measurements

The workspace's tests: 3485 passed and 68 ignored; clippy over every
target and fmt clean. The late-write test fails with the late folders
left out of the take-out; the index's new test and the stack test
fail on the return into the renamed folder without the rows moved.
