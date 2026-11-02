# §531. A first Subtract or Erase stroke cuts a new brush out

*2026-11-02*

A tester added a Brush to a mask whose first shape was a Luminance
range, set the brush's row to Subtract, and painted: nothing changed.
That row (Add, Subtract, Erase) acts on the brush's own paint, and a
new brush has none, so there was nothing to take away; under the shape
mode Intersect the empty brush emptied the whole mask instead. What
cuts a brush out of the mask is Subtract in "Add a shape". The row is
never saved, so after a restart it read Add, the "− Brush" row worked,
and the first try looked like a bug.

The rule: when a new brush lands in a mask with a live shape and the
row is on Subtract or Erase, we store that first stroke as Add and give
that brush the shape mode Subtract, whatever the shape row said, then
put the row back on Add so later strokes keep cutting. An Erase is
stored at full flow and no feather, since Erase clears the whole radius
hard-edged; a Subtract keeps the row's flow. The tool's own mode is not
changed, so after an undo the next brush lands as the rows say. A brush
that is the mask's first live shape keeps its stroke as drawn. The
shape row's meaning and the mask arithmetic are unchanged.

The test paints over a Luminance range with the row on Erase and on
Subtract under the shape modes Add, Subtract, and Intersect, and checks
the weight at the first stroke point is 0 for a bright sample, before
and after a JSON round trip; it was 1 (0 for Intersect, the whole mask
gone). An Erase at flow 0.3 and feather 0.5 clears its center and 0.85
of its radius; after an undo, a stroke with both rows on Add lands as
"+ Brush". The guide's brush section says what the row acts on.
