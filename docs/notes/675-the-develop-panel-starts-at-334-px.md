# §675. The develop panel starts at 334 px (2026-11-14)

§672 gave a wider panel's extra to the labels first, and left the
default at 320 px, where the column is its least (68 px) and
"Roundness" is cut. We now start the panel at 334 px: the column is
82 px there, the narrowest at which no English label in the panel is
cut. It is also the width we had settled on by eye before we
measured it.

- The default (`right-size`), the width a double-click on the panel's
  edge goes back to, and the floor of the panel's widest (`right-most`,
  never under the default) are 334 px. The narrowest stays 280 px.
- The column still grows from 320 px (`LabelColumn`), so a panel
  dragged under the default gives the labels' 14 px back before the
  tracks shrink.
- A width kept in the settings is untouched; only a panel never
  dragged changes. The viewport is 14 px narrower for those.

Tested in `panel/sizes.rs`: the column at rest, "Roundness" whole at
the default and cut at 320, the edge's drag from the new default.
