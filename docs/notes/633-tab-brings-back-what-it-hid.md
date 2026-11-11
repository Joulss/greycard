# §633. Tab brings back what it hid (2026-11-11)

A tester (discussion 59) put the filmstrip away, then pressed Tab
twice: the strip came back with the panes. Since §178, Tab from all
away brought all three back, whatever had been put away by hand.

The rule now, in `panes::tab` with the window calling it:

- Any of the three showing: Tab puts all three away and adds those
  showing to what it keeps.
- All away: Tab brings back what it kept, or all three when it kept
  nothing, so it is never a dead key. It keeps nothing after.
- A pane shown or put away by hand (F6, F7, F8, or its arrow) leaves
  what Tab keeps. A pane put away by hand is not showing at the next
  Tab, so it never comes back uninvited; one shown by hand and still
  showing joins the kept again at the next Tab.
- Over the grid, Tab is the left pane's alone and counts as a hand.
  The left pane brought out in the grid and still out in the loupe
  rejoins at the next Tab, so Tab, the grid's Tab, a frame, and Tab
  twice bring back all three. Brought out and put away again in the
  grid, it stays away: the hand's last word.
- The lightbox saves what Tab keeps with the panes it puts away and
  puts both back on the way out, so Tab inside it, in its grid or its
  loupe, leaves nothing behind.

We add to the kept rather than replace it. Replacing let a glance by
hand between two Tabs (F6 to see the strip, say) shrink the memory to
that one pane, and the panes never put away by hand stopped coming
back. We clear per pane rather than all at once, since clearing all
falls back to all three and brings back the strip the tester put
away: the same bug by another road.

The memory is not saved between sessions; the hidden flags are
(`hide_left`, `hide_right`, and `hide_strip`). So a session opened
with all three away has nothing kept, and its first Tab brings all
three back, as before: hide the strip by hand, Tab the rest away,
quit, and the next session's Tab brings the strip back with the
panes. A session opened with a mix hides the rest and keeps what
showed.

**Tested.** The rule over every pairing of the three panes' state
with what Tab keeps; in the window, each pane put away first, the
tester's case, all put away by hand, a glance by hand between two
Tabs, the grid's Tab and back, the strip's arrow clicked while Tab
has the panes away, and the lightbox in and out. The left and right
arrows are wired as the strip's is and are not clicked by a test, and
nothing was tried in a real window.
