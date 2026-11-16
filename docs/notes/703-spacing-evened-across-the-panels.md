# §703. Spacing evened across the panels (2026-11-16)

A contributor's pass over the panels' margins (PR 163), taken whole
but for one simplification.

- The left pane's buttons and foot stand 8 px in from its sides, as the
  right panel's do. The boxes between them keep §699's 4 px, so a box
  is 4 px wider than the buttons each side.
- Parametric and Point, and Cubic and Tabular, are equal in width
  (`Segmented.equal-width`, which the control already had).
- "With subfolders" is 6 px in, on the line of the box's rows under it.
- Soft proofing and Monitor keep 4 px between their rows, and Monitor
  8 px more above the canvas shades. The PR wrapped each section's rows
  in a layout for it; `Section.body-spacing` (§699) is that gap, so we
  set it and left the rows where they were.
- The lens name has 12 px above it, not 8.
