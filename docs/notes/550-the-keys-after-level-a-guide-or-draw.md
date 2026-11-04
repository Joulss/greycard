# §550. The keys after Level, a guide, or Draw

*2026-11-05*

Two pool lines on the picture taking the keys.

**Text boxes.** The first had landed in §391: `Typing.boxes` counts
the panel's focused text boxes, and while it is above zero the
pointer crossing the picture leaves the keys with the box. The boxes
counted: the filter's words, a custom aspect, the words that find a
lens, a mask's name, and the frame's title and keywords. A snapshot's
name keeps its own flag (§290), read in the same place, the
viewport's `typing`. The look's name, the export presets' names, and
the rest of the `LineEdit`s live in sheets, which cover the picture,
so no crossing reaches them. A press on the picture takes the keys
whatever is typed: the box loses its focus and counts itself out. We
add the test the line asked for.

**Level, a guide, and Draw.** A `PanelButton` takes the focus on a
click, so Escape went to the button and no further until the pointer
crossed the picture again, and the level's and the guide's overlays
cover it. Each of the four buttons now hands the keys to the window on
its click, as the panel's other tool buttons do, and Escape puts the
tool down with the pointer anywhere. Draw is not in the line; it had
the same fault.

## Tests

The filter's words typed with the pointer over the picture, then a
click on the picture: the next digit rates the frame. Level, Verticals,
Horizontals, and Draw each clicked, then Escape with the pointer still
on the panel: the tool is down and the crop stays on.
