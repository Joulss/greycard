# §618. Sections on cards (2026-11-09)

The hairlines of §604 separated the Develop sections, but their
headers still ran into the controls at a glance. A tester asked
for the sections to read as separate blocks, like the darker, spaced
sections of another editor, without making the panels bulky. The same
problem shows in the left pane's Folders and Presets.

## The decision

Each `Section` has a square `base` surface against the panel's
`surface`, with the panel's existing 8 px gap between sections. The
header and folding body paint adjoining parts of the same card, so
opening a section leaves no seam above its controls. The header keeps
its original height, type size, and colors, with 8 px at each side.
The body has 12 px at each side and 8 px at its foot,
with 4 px between controls. The two rules from §604 are removed; the
card and gap now mark the boundary. This supersedes §604's choice of
a hairline.

The change belongs in the shared `Section`, so the Develop and Masks
tabs and the left pane read alike. Its fold, switches, actions, and
the left pane's height sharing keep their existing behavior. All
three themes use their own `base` and `text` tokens.

Folders, Presets, and History lose the border and rounded corners of
their inner list boxes; Camera Profile loses its inner border too.
The outer section card supplies the boundary. Folders and Presets
also lose their list's inset. Their rows share a 22 px height, a 1 px
gap, and the same 16 px chevron slot and depth step. Each list reaches
left past the body's inset, placing its first chevron under the
section's chevron. Presets keeps 4 px between its list and the buttons
below it.

The shared `EditSlider` is 24 px high instead of 28 px, bringing
successive sliders closer together. `Segmented` loses its outer
border, and White Balance has a 2 px gap before its control row.
These are presentation changes; the controls retain their actions
and values.
