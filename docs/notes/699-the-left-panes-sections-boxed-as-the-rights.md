# §699. The left pane's sections boxed as the right's (2026-11-15)

§690 put the headers of both panes in one look; their structure still
branched on `Section.boxed`. The left kept §669's: 20 px unpadded rows,
a hairline above and a rule under, no box, and an 8 px gap between
sections that the share edges sat in. We take the right's for every
left section (NAVIGATOR, ROOTS, FOLDERS, COLLECTIONS, METADATA,
PRESETS, SNAPSHOTS, and HISTORY): a 32 px header with 8 px above and
below its title, heading a raised box with a 1 px border, the body
padded 8 px above and below and 6 at its sides, folded to a 1 px rule,
the whole row lit and pressed as one. `boxed` and `line-above` are
gone from `Section`, so the panes cannot drift apart again.

The Export queue becomes a header too (`QueueHeader`): a tester read
the old row as a second-class citizen. Its icon and EXPORT QUEUE sit
where a section's icon and title do, the chevron's room left empty;
its count stays at the right, the whole row lights under the pointer,
a press anywhere on it opens the queue's view, and it is lit as chosen
while that view shows. It has no body and no box, so it never shares
the pane's height. Its title is the catalog's "Export queue" put in
capitals as it is drawn, so no new string, and cut short it has the
whole title as hover text, as a root's row has its name. Promoted to a
header, it behaves as one: `queue` is a section of Library's own in
the order, first under COLLECTIONS, and its header drags as the
others do, with the same drop line. An order kept before it was a
section gets it straight after COLLECTIONS (`reconcile`). It is
Library's alone, as before.

Sections abut on the left as on the right: a box shows where one ends,
and the next header's 8 px above its title are the gap. The share edge
between two open lists lies over the 8 px of padding at the foot of the
box above, so only where the section above is an open list: never over
a row or a header. The header below an edge keeps its whole row for the
fold, live edge or dead. A double-click on an edge evens the two, as
before. While the queue still sat in COLLECTIONS' slot the review
measured what that rule cost: in the first order (ROOTS, FOLDERS,
COLLECTIONS, the queue, METADATA) no edge was lost, but FOLDERS dragged
straight under COLLECTIONS had none, the queue's header lying between
them. With the queue in its own slot, COLLECTIONS ends in its box again
and that edge is back. Two lists have no edge between them only where
something without a box stands between them: the queue's header in
Library (COLLECTIONS, the queue, FOLDERS), or a folded list.

The scrolled part of the pane takes one 4 px margin both sides, as the
view buttons and the foot do, so Open folder, Import, the queue, and
every box run to the same right edge as Library and Settings. The pane
scrolls as a whole once its sections pass its height. With the tests'
standard content that is below about 1020 px of window in the Loupe
and 820 in Library (the boxes' 16 px a section raised both), so it
does scroll on many 1080p Loupes. Its bar therefore reserves no band
and lies over the margin instead: `ScrollPane.band-width` narrows its
presses to 11 px, the margin and a box's border and side padding,
short of a list's own bar and a row's count (the outer 4 px are the
pane's resize edge's). Its track and thumb keep to the box's 6 px side
padding, clear of the box's border, at every bar size and with Stay
(`bar-track-inset`, `bar-thumb-inset`, `bar-most`); a Wide thumb is 6
px there. Every other scroll pane draws as before.

What stays different, and why:

- `Section.body-spacing`: the left's lists keep 4 px between a list and
  what is under it (the toggle, the buttons), as they had. The right's
  sections set their own gaps and leave it at none. It is the body's
  rhythm, not its frame.
- `SectionSlot.fills`: the left's slots share the pane's height, so each
  stretches to its section's bounds; the right's stay at the top
  (`start`), which leaves their maximum unbounded. A layout matter;
  nothing drawn reads it.
- `Section.empty`: NAVIGATOR with no frame chosen has no body; it shows
  its header over the 1 px rule, as folded, not an empty box.

The lists' floors are still three rows; the box adds 16 px round each,
and the share sums take it as part of the section. Each section is
16 px taller, so on a 1000 px window the Loupe's lists show about a row
less, which we accept. The rows narrow by 12 px (the box's sides); long
folder names elide as before, and the tree's hover fill reaches the
box's padding, not the pane's gutter.

Known limits: Open folder and Import sit outside any box at the pane's
4 px gutter, so while the pane scrolls its bar draws over their right
6 px, inside their border, and takes the right 7 px of their presses.
The NAVIGATOR's picture loses 12 px of width and its section grows by
16 px; its 260 px cap is unchanged.

Tests: the Navigator's two tests read the picture's box inside the
section's padding (8 px under it, 12 px narrower), as §670 noted the
submitted test once did. Share tests: a click 1, 4, and 7 px into
SNAPSHOTS' header folds it under a live and a dead edge, and the live
edge still drags; every y of the queue's header opens its view, in
the first order and between COLLECTIONS and FOLDERS; with FOLDERS
straight under COLLECTIONS their edge drags and evens; a press low on
the last folder and the last collection row is the row's. Order tests:
the queue's header stands where every Library order puts it, a fresh
order and a kept one without it put it after COLLECTIONS, and its
header drags it above FOLDERS, where a later move of COLLECTIONS leaves
it.
`capture_the_section_headers` also fills the left's lists, captures the
narrowest pane (200 px), a folder row and the queue's header under the
pointer, Library with the queue dragged above FOLDERS, and a 680 px
window where the pane scrolls, its bar at each size with Fade and Stay,
at rest and lit; it reads each header's box (top rule, left rule, fill)
from the pixels.

Left for later: `QueueHeader` carries a copy of `Section`'s drag (about
60 lines), kept apart so the header keeps its button role and label; a
shared touch area would end the drift risk (roadmap).
