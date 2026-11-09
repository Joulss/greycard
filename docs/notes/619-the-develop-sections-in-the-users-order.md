# §619. The Develop sections in the user's order (2026-11-09)

The separate cards of §618 make the Develop sections easier to read,
but their fixed order still makes a photographer scroll past controls
they use less often. The requested order is a choice about the panel,
not about the order in which the engine develops the picture.

## The decision

Every section on the Develop tab can be taken by its header and dropped
between any two sections. A short movement still clicks the header and
folds or opens the section as before. During a drag, a blue line in the
gap shows the destination; the panel scrolls when the pointer nears its
top or bottom. Moving the pointer away from the panel clears the drop
position. An open section stays open when it moves, as do the other
sections. The Masks, Crop, Retouch, and Cull tabs keep their existing
order.

The panel remembers the Develop order in `settings.json` as a list of
stable section names, written when a section moves. On startup, names
that are no longer in the panel are ignored, repeated names are kept
once, and sections added since the order was saved follow the saved
ones in their default order. An older settings file with no order uses
the existing order. The folded state is still kept separately, by
section name, so moving a section does not change its fold.

Slint's panel lays the sections out from that list. Each name selects
the same typed section and bindings the fixed panel used; the shared
`Section` handles the header gesture only for sections marked as
reorderable. Rust owns the saved list and the move on drop. No edit,
sidecar, or development pipeline setting is reordered.

The broader roadmap item also covers default folds, ordering on the
Masks tab, and controls in Settings. Those remain separate work.
