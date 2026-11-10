# §627. Undo takes back a mark out of Cull (2026-11-10)

A tester hid the rejects, pressed X by accident in Develop, and the
frame left the strip; Ctrl+Z stepped the next frame's edit and the
reject stayed. Cull's Ctrl+Z already walks the session's marks (§117's
tag history, across frames); Develop's and the library's walked only
the frame's edit line.

**Asked.** Out of Cull, Ctrl+Z takes back a mark when it was the last
thing done and brings back a frame the filter hid; redo the same;
an edit made after the mark goes first; edit undo otherwise as before.
"After" is the order of the presses: a key whose read is still out
when an edit is made was done before that edit, however late it lands.

**Design.** Two stacks, one clock. The mark history is the session's
and across frames; each line's history is the frame's and in its
sidecar. We order them with one process-wide tick
(`greycard_edit::Order`), stamped on a step each time it is acted on:
when it is done (recorded or redone) and when it is undone. A line
keeps the tick its current state was made current at
(`Line::current_order`) and, on each undone state, when it was undone;
`tags::History` keeps the same two. A state read from disk is
`Order::BEFORE`, older than any mark. A sync join carries each state's
tick through as the copies had it, the first state's and the current's
included, so a state this session made or walked keeps its place; a
state only the other copy had, or one the join makes (its "Reconciled"
step), is `BEFORE`, the join not being something the user did. When
the engine's source makes two states equal and `take_sources` merges
them, the merged state keeps the later tick: undone back to, it was
made current when the later was, and a redo still makes again what
was undone last. The stamp is never written and its `PartialEq` is always true: it is not part of
what a state is, and a sidecar read back still equals the one written.

Ctrl+Z out of Cull takes back whichever of the two, the session's last
mark or the current frame's last step, was done later; Ctrl+Shift+Z
makes again whichever was undone later (`tags::marks_first`, one rule
both ways). So a Ctrl+Z always takes back the Ctrl+Shift+Z before it,
and the other way round, however marks, edits, and new actions
interleave. A first cut stamped a step once, when first done: a mark
redone after a newer edit then counted as older than that edit, and
Ctrl+Z took the edit instead of the redo. Stamping on every act fixes
that without either kind's new action dropping the other's redo. With
no frame current (the library with nothing chosen) the marks step
whenever there is one.

A tick is the press's, not the landing's. A request queued at a press
(a rating, flag, or label key or the menu's, a keyword, a title, a
time shift, a preset, a paste, a sync, a held undo) takes its tick when
it is made (`Order::act`), and runs inside `Order::during`, which hands
the work its own ticks just after the press's: clock ticks are a
stride of 2^20 apart, and the work counts up inside the gap. So a mark
waiting on a frame's read stands before an edit made while it waited,
and Ctrl+Z takes the edit first. A first cut stamped the mark when its
read landed, and took back the mark instead. The one other record we
found landing after its act, a person carried to the other frames from
the Parts offer, takes the offer's tick the same way. Auto light
records at the press. The panel's unsaved edit is still recorded at the
undo, as before, so it counts as the newest thing.

When a mark wins, or a mark key is still queued on
its read, the step goes through `rows::request` as Cull's does and is
decided in its turn. So does an undo pressed while a pick's open is
queued, once the session has marks: the pick is current at once, but
its line stands in until the read lands, and the step is the frame's
own. A mark's undo that moves frames does it by `invoke_select`, which
is the strip click's own open: `st.current` moves before the handler
returns (a loaded frame opens there and then; an unloaded one is a
pick, current at once), so a second Ctrl+Z never weighs the frame left
behind. A tool in hand is put down and the crop kept, as a click does.
An edit's undo with no mark in the way runs at once, as it always did.

The mark's word ("Undo: Unflagged") goes to the status line, there
being none over the picture out of Cull, held past the develop the
reselection starts, as a key's word is.

The guards are asked twice. At the press: a sheet open (Slint drops
the key), Cull (its own marks-only step), and the queue's view
(refused). An undo that waits its turn is asked again when it runs
(`step_held`), since the window can move under it while it waits: a
sheet opened drops it, Cull turned on makes it Cull's step, and the
queue's view drops it, as a press there would. A first cut checked
only at the press; an undo held behind a pick's read, then C, undid
the pick's edit when the read landed and took Cull down with the
develop that followed.

Out of Cull the history panel's Undo and Redo buttons are the edit's
alone, as before: they are greyed out with the edit's line, and a
button that undid a rating on another frame would surprise. In Cull
they step the marks, as the keys there do and as they always did. They
have callbacks of their own (`undo-edit`, `redo-edit`); only the keys
take the mixed order.

**Left out.**
- Neither kind's new action drops the other's redo: a rating does not
  wipe the frame's undone edit states (the history panel's muted
  rows), and an edit does not drop an undone mark. A strict single
  timeline would drop both; the per-act stamps keep the walk
  consistent without it.
- The marks' stack holds keywords, titles, and time shifts too, so
  those are undone out of Cull as well; filtering them out would break
  the order.
- Ctrl+Z compares the current frame's line only, never another
  frame's; it does reach back to marks made earlier on other frames
  and goes to them.

**Found in review, and left.**
- A request queued from inside a running act takes that act's tick and
  counts from it again when it runs, so two of one act's steps can tie.
  No path ties a mark with an edit today, and on a tie the edit goes,
  both ways.
- A line is a stack, so a step that lands late with its press's tick
  (a Parts answer, a sync onto the open frame) can sit over a newer
  edit, and Ctrl+Z then takes a mark between them out of press order.
  Undo and redo still reverse each other.
- The Cull branch's return for no current frame in `step_held` has no
  test.

**Not proven.** The running app: the event loop's own timers and a
real Parts answer. The tests drive the headless window.

**Tests.** In `panel/history.rs`: a reject under a hiding filter
undone in Develop brings the frame back and selects it, redo hides it
again; edit then mark undoes the mark then the edit, mark then edit
the edit then the mark, redo retraces both, twice over; the panel's
unsaved edit goes before a mark; an edit from before the session goes
after the marks; a mark redone after a newer edit is undone first; an
edit redone after a newer mark is undone first, and its mirror; a
forty-step walk of undos, redos, marks, and edits in which each undo
and redo is taken back by the other; the history buttons step the edit
alone; the Ctrl+Z after a mark's step weighs the frame it went to; a
mark's undo with a brush in hand or the crop open ends as a strip
click does; the library with a frame current and with none; the queue
view refuses. In `roots/tests/picks.rs`: an undo during a pick's read
waits for the open and steps the pick's own line; an undo held behind
that read with Cull turned on, a sheet opened, or the queue's view
opened meanwhile is Cull's step, dropped, and dropped (each fails
without the second asking). In `tags.rs`: `marks_first`'s cases, and a
step stamped on undo and again on redo. In `line.rs`: two undone
states, and two in the history, merged by a source keep the later tick
(each fails without its line); work run in an act's name takes the
act's ticks. In `roots/tests/picks.rs` too: a key waiting on a read,
then an edit, undoes the edit first; an edit, then a key that waits,
undoes the mark first, the undo pressed before the read lands or
after. In `panel/parts_all.rs`: a person carried by the offer stands
before a mark made while the answers were out. In `sync.rs`: a join
keeps the session's ticks.
Cull's own tests are unchanged and pass.
