# §601. The prefetch deeper in the walk's direction (2026-11-07)

The prefetch (§379) made the next frame and the previous one while
the worker was idle. A walk forward with keys quicker than a base
outran it: each key landed on a frame whose base was still to be
made, since the next one's prefetch began only after the last step's
develop and the quiet. With the demosaic on the GPU (§591) a base is
cheaper, and the shelf (§378) has room for more than two on most
machines, so we now go deeper in the direction the user walks.

## The walk's direction

The window keeps which way the strip is walked (`browser::Walk`): the
side ahead, how many steps in a row have gone the other way, and how
many have just gone ahead (the streak), sent with the list as a
`Heading`. A
step is an open of the frame the right or the left key reaches from
the frame opened before it; anything else is a jump, which keeps the
direction and forgets the steps against it, and the same frame opened
again is neither. The walk turns after two steps the other way, not
one: a single step back is a look at the frame just left, which is on
the shelf already, and a back-and-forth compare between two frames
never turns it, so the prefetch does not swing to and fro with it.
The opens that bypass the strip's own (leaving culling for the
editor, the reopen after the lens database arrives) count as steps or
nothing by the same rule, so the frame a step is counted from never
goes stale.

## Which neighbors, in what order

`beside` now gives the cells around the open one with their side and
their distance, in the order their bases are to be made: the nearest
ahead first, so a single step always finds its base; then the one
behind, which on a walk is the frame just left and costs nothing; then
the second and third ahead. Walking right from frame 5 that is 6, 4,
7, 8; walking left, 4, 6, 3, 2. The ends of the strip and a hidden
open cell follow the arrow keys' rules as before. `Worker::neighbors`
and `neighbors_again` take the direction with the list.

## How deep: the shelf decides

The window cannot know the shelf's budget, so it always sends the
deepest list, and the worker trims it in one place, `prefetch::depth`,
from the room the shelf has for bases the size of the open frame's
as a prefetch makes them: its picture, without the guide plane the
open frame carries, a third again, which would count the room one low
(at 45 MP a 12 GB machine would get two where three hold). The open
frame's own base is in hand, not on the shelf. Never more
neighbors than that room, so no prefetched base pushes out another or
the frame just left: one behind from room for two, the rest ahead up
to three. Room for one makes only the next frame ahead, which then
takes the place of the frame just left, and only once two steps in a
row have gone ahead: one step is as likely half a compare of two
frames, and a prefetch then would push out the frame the next key
returns to. So a compare keeps both frames warm as under §379, which
made nothing there, and a walk gets its one ahead. No room makes
nothing.

The room is counted in bases the size of the open frame's, but each
neighbor is shelved at its own size: a shoot from two bodies, or a
crop mode, has neighbors larger than the open frame. So the depth is
only the most the worker tries. Before it makes a neighbor, once the
decode has said its size, it asks the shelf what the put would let go
(`Shelf::pushes_out_before`), and again with the base made before it
is put (`pushes_out_of`), on `put`'s own rule for which of the file's
bases a put replaces: the file's base at another turn or under another
base stays and is counted. A base that would push out one the list
names, or the frame just left, is skipped and said in the log, and
its "prefetching" line comes only after the check, so a skipped one
never reads as begun. The frame just left is the last other file
opened: an open of the file already open (another version, a reload
after an outside edit, culling left on the same frame) does not make
it the frame left. The frame just left is not kept where
the depth keeps none behind (room for one, a walk under way), since
there the next takes its place by design. And before each prefetch the
shelved bases the list names are brought to the front
(`Shelf::bring_forward`): a neighbor skipped as already on the shelf
was otherwise left where it lay, least recently left, and on a full
shelf the step back's own list pushed out a base it named and then
made it again, pushing out the next.

At 45 MP (515 MB a base as the shelf counts it) an 8 GB machine has
room for one and makes the next frame only; 16 GB has five and 32 GB
seven, and both make three ahead and one behind, the rest of the room
left to frames visited earlier.

The worker says the depth and the direction once each time either
changes: `prefetch: 3 ahead, 1 behind, walking right`. The prefetch's
lines name the frame by its place, `the second next frame`. The
window logs a neighbor it leaves out (offline, an edited stand-in) only
for the nearest each way, which any depth makes; and a shelf with no
room says so in the depth line alone.

## What did not change

The idle rule is §379's: a prefetch begins only when nothing is
queued and the window has been quiet, and gives way at the next stage
check to any request but the open of the frame it is making. A new
open's list replaces the last, so the far neighbors of the frame left
are let go unless the new list names them; a neighbor that gave way
goes back only onto its own list, not one sent since by
`neighbors_again`. A prefetch in flight for
another frame gives way rather than finish: the worker is one thread,
so finishing it would hold the user's open for the rest of a base.

The GPU needs no new guard. The engine's context is built and used on
the worker's thread alone: the viewport's develop, the prefetch, and
the save's read-back all run there, one job at a time, so a
prefetch's demosaic on the device never overlaps the viewport's. The
window draws on the same device, as it did before.

## Measured

An ignored test, `walk_timing`, walks right through a folder of raws
on the worker as the window drives it, timing each open from its send
to its develop delivered (the key to the picture less the window's own
frame). Eight 24 MP raws of one camera, on lavapipe under §221's
recipe, a release build, a look of 12 s and then three keys 0.3 s
after each develop lands, twice over; "before" is the same build with
`GREYCARD_SHELF_MB=600`, room for two of these frames' bases (275 MB
each) and not three, which is §379's depth (one ahead, one behind);
at 45 MP the same stand-in is 1100. The times were taken before the
shelf's push-out check, which on uniform frames under this budget
changes nothing. The test build's quiet is 30 ms, not 1.5 s.

| Two runs, ms | after a look | burst, 2nd and 3rd key | mean of 7 |
|---|---|---|---|
| Three ahead, one behind | 5345, 5880, 5963, 5434 | 5076 to 6738, mean 5740 | 5887, 6292 |
| One each way (before) | 4205, 4470, 5734, 5849 | 6425 to 8888, mean 7820 | 6175, 7892 |

Every step took its base from the shelf both ways; the difference is
where it was made. Before, the burst's second and third keys met the
prefetch of their own frame in flight and waited out the rest of it
(about 2.2 s a base here, the demosaic 1.3 s of it); after, all three
were made during the look. On lavapipe a develop from the shelf is
itself about 5 s, the device's sharpen and upload in software, so
these are the shape of the change, not a card's numbers: on a real
card the develop from the shelf is 0.2 to 0.4 s and the base the
burst no longer waits on is most of what a key costs.

## Verified

- Unit tests: the depth for each room from none to beyond seven, and
  never more neighbors than room; the depth at 8, 16, and 32 GB for a
  45 MP base, and for the unknown-memory budget; `due` making the four
  a window sends walking right in its order, trimmed for room for
  none, one, two, three, and plenty, and walking left keeping one
  behind; the log line's text.
- The shelf: what a put would push out, said before it is made, for
  a larger base, one over the whole budget, and the one-base budget;
  the file's own base at another turn counted as staying, at the same
  turn and base replaced; the bases a list names brought forward and
  let go last. The decode's reckoning of a base's size is never less
  than the base, and within twice it.
- Room for one: no prefetch until two steps ahead in a row, end to end
  a walk making the next each step and then a compare of two frames
  finding both on the shelf every time, nothing made meanwhile.
- Neighbors twice the open frame's size on a shelf for three of its
  bases: the first made beside the frame just left, the second skipped
  as it would push that frame off, both kept.
- A full shelf, two ahead and one behind: the step back's own list is
  sent and made, its shelved bases kept and not made again, and the
  next step finds its base; the test fails with the bases not brought
  forward.
- A neighbor of a replaced list that gave way is not put on the new.
- A, then B, then a version of B: the frame left is still A, and the
  list keeps it.
- The worker end to end: four neighbors made in order (next,
  previous, second next, third next), a step taking its base from the
  shelf and making only its new far neighbor, and the step back
  finding the frame just left on the shelf; a jump while a far
  neighbor is held in its prefetch abandons it, never makes it, and
  makes the frame both lists name under the new list's place.
- The window: `beside`'s order both ways, at the ends, from a hidden
  cell, and with one frame; the walk turning after two steps the other
  way, not on a compare, a jump forgetting a step, a frame reopened
  counting as nothing; the neighbors of a walked cell sent turned.

## Not verified

- Real cards: the times above are lavapipe's, which shows the walk
  runs on a device and nothing more.
- The window's own key-to-picture time headless (`--time-select`); the
  measure is the worker's, open sent to develop delivered.
- An 8 GB machine; the depth there is tested through budget values.
- A sort or filter change does resend the neighbors, but under the
  direction walked so far; a walk through a re-sorted strip may start
  the wrong way for two steps.
