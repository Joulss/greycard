# §495. Previews near the screen, and around the cull

*2026-11-01*

The Library's previews behind the camera thumbnails were made for
the whole root at its first open, a quarter of a CPU second a cold
frame and an hour on a 12,515-file root (discussion 51; shown since
§452). Now they follow the rule the edit's pictures took in §452.

**The rule.** The pool starts a preview only for a cell near the
screen, the rows on view plus as many again above and below, the same
set the edit's pictures use; far previews wait in the queue as a path
and nothing more, and a scroll brings them back in the order the grid
shows. While culling, in the Cull view or the lightbox, previews are
also made for the shown cells within two hundred rows of the rows on
screen, nearest first, since those are the frames about to be seen;
the culling loupe itself decodes the file in hand, so the whole list
was never needed, and a filter down to a few frames culls only those.
The status line's count leaves far previews out. The embedded JPEG
stays the first picture, and a frame with no preview yet shows what it
showed. On 2,000 copies of a 7 MB CR3 the pool went idle at thirty
seconds having made 48 previews, against seventy to a hundred seconds
and all of them before; the first screen is unchanged, since previews
always waited behind the thumbnails. The camera thumbnails themselves
are still made for the whole root, about 0.1 CPU second each, which
is what is left of the item.

**A merge lost the queue.** The roots view grows its list in several
merges, and each merge forgot the queued previews and edit's pictures
while the window asked again only for thumbnails it did not have, so
frames already shown never got theirs: about a fifth of the previews
before, and every near one under the new rule. A merge now renumbers
both queues under the new cell numbers, a preview brought by a version
landing on the lowest cell of its file. The near set is worked out
after the list is rebuilt, not before, after a delete or a move.

**Not done.** In the Cull view with the strip hidden, the reach
follows where the strip last stood rather than the culled frame; a
root taken offline after a browse has previews only for what was near
or culled.
