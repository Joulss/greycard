# §631. Develop's undo stays on the open frame (2026-11-11)

Amends §627. As built, Ctrl+Z in Develop walked back through every
mark of the session, jumping to each frame, before it reached the open
frame's older edits. A mark on another frame is not the open frame's
business; Cull (§117) is where the session's marks are walked.

**Rule.** In Develop (a frame open, no grid) undo and redo reach a
mark only when it named the open frame's file, or when it is the mark
that carried the window off its frame onto this one and the window has
stayed here since. Other marks are passed over: they stay on their
side of the history, in their order, for Cull. The marks reached and
the frame's edits interleave by tick, as §627 has them. The library's
grid, and no frame open, keep §627's reach, any frame. Cull is
unchanged.

**The carry.** The tester's case: X under a filter hiding rejects,
the frame leaves the strip, the next opens. At Ctrl+Z the open frame
is the next one, and the reject is not on it. So a mark act (a key, an
undo, or a redo) that takes the window off its frame names the frame
it lands on (`tags::History::carry`), and the step counts as that
frame's too until the window's frame changes. Ctrl+Z there brings the
frame back; a mark or an edit made on the landed frame since goes
first, by tick. The carry ends at any change of the current frame
(`browser::current_moved`, called wherever `st.current` is set: the
pick, the open, Cull's step, a list replaced, a renumbering), so
leaving by any path and coming back does not revive it. A renumbering
that keeps the frame (a list merge, a version added or dropped) keeps
the carry: the frame is asked for once the new cells are in place, as
a path, not an index. One carry at a time. Redo is the same: an undo
that hides the open frame (a pick taken back under a picks-only
filter) carries the window, and Ctrl+Shift+Z from where it landed
brings it back. A reject in Cull or in the library's grid that hid its
frame carries the same way, and we keep that: going to Develop on the
frame it landed on, Ctrl+Z there brings the frame back, since it was
the last thing done and it moved the window. From any other frame it
is not reached.

**Out of order, sound.** Taking the open frame's mark past newer ones
means stepping from the middle of a side; the step goes to the other
side's top with a new tick, as any act. Whether a mark over several
frames can go whole is decided by the history, not by what the frames
carry now: it is blocked when a later step on the same side names one
of its frames. A first cut compared values, and a frame put back to
the same value by later marks (5, then 0, then 5) let the step go
whole; Cull's walk then skipped the later marks by value and left a
wrong rating. Unblocked, the mark goes whole. Blocked, only the open
frame's share goes (`History::take`): it becomes a step of its own on
the other side, and the rest stays where the step was, for Cull. Every
frame's steps then stay in the order they were done on both sides, so
Cull's undo of everything ends each frame at its session start and its
redo at its end. The rest no longer names the open frame, so the next
Ctrl+Z moves on to the frame's older mark. A blocked carried step
gives the shares no later step names, or is passed over when none.

**Versions.** A mark is the file's (§117), so a version's cell reaches
the marks on its file, from any of its cells.

**Same everywhere.** The key, the menu, and the held undo
(`step_held`) all decide through `mark_to_step` when they run; the
history panel's buttons still step the edit alone.

**Left out.** A step passed over keeps its tick, so Cull's next Ctrl+Z
after a Develop redo takes the redone step first, then the
passed-over ones in their order.

**Tests.** In `panel/history.rs`: Develop reaches only the open
frame's marks, interleaved with its edits, never moves off the frame,
and Cull then walks the passed-over marks in order; the reject that
carried the window is undone after the landed frame's newer mark and
edit, no older mark on another frame is reached, and leaving and
coming back ends the carry; the carry ends when Cull's own step moves
off the frame; a reject in Cull that moved the window is undone in
Develop on the landed frame and not from another; an undo that hides
the frame carries and the redo brings it back; a selection's mark goes
whole, or the open frame's share when blocked, the next Ctrl+Z going
on to the older mark; a frame back at the same value still blocks
(the review's case), Cull's undo-all and redo-all ending right, twice;
a blocked redo makes again the open frame's share; a version's cell
reaches its file's mark and no other file's; the grid still reaches
another frame's mark; a reject over a selection that carried the
window goes whole, as a share, or is passed over, with Cull's walks
right after each; a grid reject that moved the window is undone in
Develop on the landed frame. In `roots/tests/picks.rs`: a held undo
behind a pick's read passes over the mark on the frame left behind;
the carry outlives no merge, a renumbering merge, and a same-list
merge. In `tags.rs`: `reach`, the carry's start and end, a blocked
step's share taken with both walks in order, and 400 seeded random
walks of marks, Develop and Cull undos and redos, carries, and opens,
checking each frame's chain, every landing, undo-all, and redo-all.
`parts_all.rs`'s offer test now makes its mark on the frame the
person lands on. None of it was tried in a real window.
