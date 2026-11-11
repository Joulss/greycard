# §638. Thumbnail marks centered in their badges (2026-11-09)

On a thumbnail, the flag, stars, color label, and pencil, export, and
collection marks sat high in their dark plates. The stack icon also
sat high beside its count. The plates were taller than the symbols,
but the horizontal layouts placed fixed-height images at the top of
their rows. Centering the rows along their width did not center their
contents vertically. The shared components draw these badges in the
strip and grid; `ThumbBadges` also appears in the loupe and compare
tiles.

Each symbol now has a container as tall as its plate, with the image
or color dot centered inside it. The stack icon uses the same approach
so it lines up with its count. The dot is smaller than the icons, so
it uses its own height rather than an offset shared with them. The
marks use at least 4 px between symbols, keeping the pencil and export
symbols easier to tell apart at small sizes while spacing still scales
with the glyph. The badges keep their original plate heights and
placement on the thumbnail; their meaning does not change.
