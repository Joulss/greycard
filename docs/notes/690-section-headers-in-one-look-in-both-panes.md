# §690. Section headers in one look in both panes (2026-11-15)

Since §670 the left pane's headers kept master's colors: title and icon
in `Theme.text-secondary`, the chevron in `Theme.text-muted`, brightening
under the pointer with no fill. The right panel's were `Theme.text`
throughout, with the `Theme.hover` fill. One `Section` drew both, its
colors branching on `boxed`.

We take the right's look for both: chevron, icon, owner, and title in
`Theme.text`, and the hover fill under the pointer. §670 chose it last
and on purpose, with its contrast measured; the left kept the old colors
only so that pull request 136 left it alone. The colors no longer read
`boxed`, so the panes cannot drift apart again.

Size, weight, letter spacing, the muted hint and note, and the rules in
`Theme.border` were shared already. What stays different is layout: the
left's row is 20 px and unpadded, so its fill is too; hairlines on the
left, a box on the right (§669, §670).

Every title and icon keeps its place and size in every view, theme, and
scale; `capture_the_section_headers` (ignored) captures both panes and
reads the inks from the pixels.

