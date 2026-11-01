# §486. A renamed folder's stacks, and the stack in the grid

*2026-11-01*

Four of §334's follow-ups in one landing.

**A rename, seen from both ends.** The index sees a renamed folder as
files gone and files new, and a gone folder may also be an unmounted
drive, so the rows stay as they are: the old marked missing, the new
inserted or revived. What is new is saying the rename. A pass records
two kinds of event: a folder gone, its live rows marked missing; and
rows come live in a folder, added, back from missing, or moved in.
They wait on the library's own connection, 64 a kind for ten minutes,
so the watcher's two passes over a rename, one for each folder in
either order, meet; renaming back revives the old rows and is found
the same way. A match is more than half the gone folder's files by
name and content hash, the best count alone, a tie saying nothing,
each half claimed once. The stacks in the old folder follow their
files; a member carried onto a frame another stack holds leaves its
own, so a frame stays in one stack. A copy made and the original
deleted within the window reads as a rename, on purpose. The window's
own pass over a folder adds its whole report rather than a field at a
time, so a rename it completes is said too, and the file moves it
dropped before are not. Not done: a member moved outside every root,
and the two halves meet only on one connection, which the watcher's
passes share. CI on macOS and Windows then found the halves matched
each gone folder as the walk met it, so a folder gone in the same
pass could be taken for another's new name when the listing came in
path order; a pass's gone folders now drop their own waiting halves
before any is matched, in path order, a tie drops the halves it
weighed, and more than half of each side's files is needed. The
halves keep paths rather than key bytes, since a key's bytes are the
path's wide encoding on Windows.

**The one-folder listings.** `rows_in_canonical`, `count_in_canonical`,
and `burst_rows` gathered the raws' companion keys from the whole
table (§332): 22 ms on a synthetic 100,000-row library. A subquery on
the folder's index does it in a tenth of a millisecond, the results
the same.

**Shift+[ and Shift+].** In the grid, as S is, the frame swaps places
with the member before or after it; moving into the first place makes
it the top, and Shift+] on a closed stack's top brings the next
member up and on screen. The sheet has the row. Bound as the shifted
characters, like the table's other punctuation, so on a German or
French board they take AltGr as [ and ] do.

**The band.** An open stack's tiles sit on one band in the theme's
border color, half the gap each side with rounded ends, drawn under
the cells so selection and labels stay on top; a second walk over the
cells on screen, since a child of each cell would cover the cell's
own tint. In a row of mixed orientations its bottom edge steps.
