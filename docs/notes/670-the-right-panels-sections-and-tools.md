# §670. The right panel's sections and tools (2026-11-14)

From a contributor's pull request (136), finished at the merge.

Each section of the right panel (Develop, Crop, Masks, Retouch, and
Cull's) is a header over a raised box with a 1 px border: 8 px of
padding above and below the body, 6 at its sides, both measured from
the border's outer edge. Folded, the box
closes to a 1 px rule under the header. The box shows where a section
ends, so no rule runs between headers. Rows inside have no spacing of
their own; each section sets a 4 or 8 px gap between its groups.

The tabs, the Masks list, and the sections take a 4 px gutter
(`Theme.space-1`) on the left, and the scrolled sections pad right by
`max(Theme.space-1, ScrollBars.band + 2px)`, clear of the scroll bar:
the left pane's rule (§669). The panel head and the foot narrow to
match, and the width goes to the controls.

Titles are `Theme.text`, brighter than before, as are the chevron and
the icon: the header heads its box. Scopes stays where it was, in the
panel head above the tabs, not a section of its own. The tab row and
the scope picker are flat, and a segmented control's options fit their
words unless it asks for `equal-width` (the Display curve does).

A boxed header answers the pointer as one: the whole row takes the
press and fills with `Theme.hover`, as a button does, behind the
title, chevron, icon, and the mask's name. It reads as "this
folds." The accent would read as "chosen," which is what it means
elsewhere (the grading range, a tab). The title on the fill: Charcoal
11.5:1, Grey 7.7:1, Light 10.8:1; the fill against the panel is the
step every hover takes.

`Section` takes `boxed`, on by default. The left pane's sections turn
it off and keep §669's look: a header over a rule, master's colors and
hover, and an unpadded body. There the lists are unframed, and their
three-row floors assume no padding round them; a box would frame them
again and narrow them by 16 px.

Two tests moved with the layout, and prove what they did:

- The vectorscope's fold takes three 14 px lines, the 6 px spacer that
  folds with them, and one 2 px gap of the head's column: 3·14 + 6 + 2
  = 50 px, where it was 3·18.
- The submitted Navigator test kept its one height and expected the
  box's 8 px padding round the picture: the same check, read through
  the box. With the left pane unboxed, it is master's again.

At the merge we changed:

- `boxed`, above, so the left pane is §669's to the pixel.
- The header hover, above, where the submitted title alone turned to
  the accent.
- The Masks list's `chrome` is 95 px, measured: header 32, the body's
  padding 16, the list's border 2, New mask 28, and 17 outside the
  section. Its test now checks that the sections keep their 150 px
  (§300) whenever the list shows more than one row.
- The body's sides are 6 px, not 8: the body is as wide as before the
  box, and French buttons keep the letter they lost ("Réinit...").
- The tabs' options keep 2 px each side of their words, and Cull's
  button is 40 px, not 56, so the French tabs have a gap between them.
- Lens has 8 px above its profile line, as below it, not 16.
- CULLING measures its Picture row inside the body's padding
  (`Section.body-width`), so in French the label goes over the two
  buttons again rather than cutting them.
- The canvas swatches under MONITOR are ringed: one of them is the
  box's own grey (Charcoal's #262626, Grey's #444444) and vanished.
