# Cull filters fit the panel

*2026-11-12*

The Cull panel's filter chips could outrun the width of the panel,
particularly when their labels or counts grew. Stars, Flag, and Label
now each have a clipped horizontal row. A row scrolls sideways by
wheel or trackpad when its chips do not fit; when they fit, the scroll
event is left to the surrounding panel. Their existing selection,
toggle, count, and keyboard-focus behavior stays on the chips. The
chips have more horizontal room around their text, and the search
field lines up with Clear and Sort at the standard control height.

The Cull mode switch uses a flat, unframed segmented style. The shared
Segmented control now exposes its corner, padding, and border choices
for that use. The section's instructions, frame turns, filters,
comparison, move-on switch, reject actions, and Develop button have
explicit gaps, so the shared Section body's close spacing does not
run those different actions together. The panel and its switch retain
their existing Cull behavior.
