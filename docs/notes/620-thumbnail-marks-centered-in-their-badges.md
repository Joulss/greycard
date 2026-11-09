# §620. Thumbnail marks centered in their badges (2026-11-09)

On a thumbnail, the color label and the pencil and export marks sat
high in their dark plates. The plates were taller than the symbols,
but the horizontal layouts placed the symbols at the top of their
rows. Centering the row along its width did not center its contents
vertically. The same shared `ThumbBadges` and `ThumbMarks` components
draw them in the strip and the grid.

Each symbol now has a container as tall as its plate, with the image
or color dot centered inside it. The dot is smaller than the icons,
so it uses its own height rather than an offset shared with them. The
color badge's plate is also shorter, and the marks have 4 px between
them, leaving the pencil and export symbol easier to tell apart. Their
meaning, size, and placement on the thumbnail do not change.
