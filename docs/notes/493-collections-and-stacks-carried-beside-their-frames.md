# §493. Collections and stacks carried beside their frames

*2026-11-01*

The collections file was per machine, so a set gathered on the laptop
was not on the desktop (§307), and the stacks with it (§334). They
travel now as the sidecars do (§243, §244).

**The copies.** The file beside the library stays each machine's
source. Every save of a collection, smart collection, or stack also
writes a copy into `.greycard/sets/` of the folders it touches under
a root, and into those folders' backup destinations under an archive
through the roots' pairings: a collection into every folder one of its
frames is in, a stack into its one folder, a smart collection into the
folder it looks under or the top of every root. A copy has the
machine file's shape; a member in that folder is kept by file name,
and a copy written into a destination names the backed-up folder's
frames by name, so the other machine reads them as its own. Copies go
only under a root or a pairing's destination, never a loose folder.

**The lineage.** Each set carries up to 256 revisions, each a hash of
the one before and of the content, hints and a stack's open state
left out and a stack's file names in, so the same change on two
machines gives the same revision and no clock decides. A copy the
other side has seen is behind, and the other side wins; the same
content absorbs the other's revisions; two that went their own ways
are a conflict kept as both, the later keeping the id and the earlier
named "<name> (on <host>)" under an id made from its own and its
revision, so both machines make the same pair and converge. A stack
keeps the later alone, and a frame stays in one stack, the stack made
first by (made, id) keeping it. A copy left behind by more saves than
the lineage holds still reads as a conflict and is kept as both.

**Deletions.** Deleting a set leaves `<id>.gone.json` with its
lineage; it removes only copies it has seen, so an edit made since
survives, and each machine remembers its deletions so a stale copy
cannot bring a set back. The deletion is saved on the window's thread
before the delete returns, and a write still in flight when the delete
comes buries what it wrote instead of recording it.

**The passes and the writer.** The index's passes list a folder's
sets only where the listing shows `.greycard`, keyed on size and
time, and read only what changed, about 9% on a pass over folders
that hold sets and nothing on the rest; an edit that keeps size and
time is missed until the next change or launch. The window joins what
a pass brings, deletions first, conflict copies in turn, and Back up
and Bring back carry folder by folder, one way for copies only in the
destination. One writer thread does every folder write under a ten
second limit, one outstanding thread a folder, a failed write tried
again each minute and at launch, part files named uniquely and
renamed into place; at quit the window waits a few seconds for the
queue. A later build's copy, deletion, or state is left alone, read by
its version before its shape.

**Not done.** A frame renamed on one side leaves a destination stack
missing it until Back up carries the rename; a one-stack tie-break
compares hints as strings, so a non-canonical path spelling could
keep a frame in both; a write that times out but finishes later
leaves a copy no record lists; `.gone.json` and stale part files are
never cleaned; a file re-saved by an older build loses its lineage
and reads as a conflict, kept as both.
