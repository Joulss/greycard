# §618. Sections on cards (2026-11-09)

The hairlines of §604 separated the Develop sections, but their
headers still ran into the controls at a glance. A tester asked
for the sections to read as separate blocks, like the darker, spaced
sections of another editor, without making the panels bulky. The same
problem shows in the left pane's Folders and Presets. The scopes and
panel tabs also put two rows of choices together, while changing scope
views moved the controls below them.

## The decision

Each `Section` has a square `base` surface against the panel's
`surface`, with the panel's existing 8 px gap between sections. The
header and folding body paint adjoining parts of the same card, so
opening a section leaves no seam above its controls. The header keeps
its original type size and colors, with 8 px of padding. Its switch
gets a small vertical adjustment to sit with the title. The body has
12 px at each side and at its foot, with 4 px between controls. The
two rules from §604 are removed; the card and gap now mark the
boundary. This supersedes §604's choice of a hairline.

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
successive sliders closer together. `PanelButton` loses its outline.
White Balance has a 2 px gap before its control row; Look, curves,
color controls, and lens controls get small spacing adjustments around
their contents and actions. The point curve's In and Out fields are
spread more evenly. The thin vertical scrollbar thumb sits in the
middle of its wider track at rest.

`Segmented` can now use equal-width choices, an optional border, and
square or rounded corners. The Cull and Develop/Crop/Masks/Retouch
choices and the scope view choices use square corners as tabs. Compact
two-choice controls can retain a border and use equal-width choices.
This keeps each use of the shared control suited to its place without
changing how a choice is made.

The scope view choices and Selection move into a toolbar at the top of
the scope box, instead of adding a second row above the panel tabs.
The box has one 144 px height for RGB, Wave, Parade, and Vector, with
the Vector plot centered in the space below the toolbar. Vector no
longer hides the photo's camera and exposure information; only Cull
does. There is also more space below the filename and Undo/Redo row.
Together, these keep the controls below the scopes from jumping when
the view changes.

A fixed gap stays below the panel tabs while the sections scroll. In
Masks, the same gap follows the pinned adjustments list's divider, so
its scrolling sections do not touch the divider. These are presentation
changes; the controls retain their actions and values.
