# §655. The Cull panel's filters, a row each (2026-11-12)

A contributor's redesign of the Cull panel (PR 137). Stars, Flag, and
Label each take one row: Flag and Label put their captions beside their
chips instead of over them, which saves two lines. A row that runs past
the panel scrolls sideways under the wheel, as the facet rows already
did. The search field has a control's height, so it lines up with Clear
and Sort. The Cull switch is drawn flat beside the mode tabs, through
three new `Segmented` options (`rounded`, `bordered`, `padded`). The
panel's actions get small gaps between their groups.

**Trimmed at the merge.**

- **Chip padding.** The PR raised it from 4/8 px to 8/12 px. With the
  captions inline, the Label row then ran 29 px past the panel at the
  default window (1500 px, panel 320 px) in English with a big card's
  counts (412, 36, and 8 on the chips): the chips end at 1513, the panel
  at 1484. At 6 px it still ran 5 px over. At master's 4 px all three
  rows fit, Label with 19 px to spare. The header's chips use master's
  8 px again, so the grid's header is pixel for pixel master's at
  1500 px; 12 px had clipped the green, blue, and purple chips there,
  and the header does not scroll. A UI test holds the panel's fit.
- **The wheel.** An overflowing chip row took every vertical wheel turn,
  even at its end, so the panel stopped scrolling under the pointer.
  `FacetChips` did this before the PR. The rows still turn up-and-down
  into sideways, since Slint's Flickable alone moves sideways only for
  Shift, but at the row's end the turn goes on to the panel. A UI test
  covers the Label row's right end; the other rows share the code.
- **`Segmented`'s default padding** stays at 2 px; 3 px would have
  reached about 45 places.
- **`equal-width`**, declared and unused, is dropped.

**What it costs.** Master fit the label colors by spending a line on
each caption. With the caption on the row, French at the default width
shows four of the five colors where master showed all five, and a
280 px panel shows four in English and three in French. The fifth is
reached by the wheel or a drag, and nothing marks a row as scrolling:
when the cut falls between two chips there is no hint at all. The
caption scrolls away with its chips. The chips take no keyboard focus,
as before; the label keys still set labels.

The fit test measures with the machine's own font, since the tests
bundle none: 19 px clear with Noto Sans, 11 px with DejaVu Sans. A
wider system font on another platform could fail it.
