# §527. A strip cell no narrower than its name

*2026-11-01*

Since §518 a cell was its picture's width and the ring's, so a
portrait's was 65 px and a camera's twelve-character name, such as
"4Z4A2978.CR3", was cut to "4Z4A2978....". Now a cell is never
narrower than a floor of 96 px.

**The floor.** We measured the names in the theme's face: the strip's
names are `font-xs`, 11 px, in the system's sans, which is Noto Sans
here (`fc-match sans-serif`). Its advance widths, read from the font's
`hmtx` and `cmap`, make "4Z4A2978.CR3" 74.1 px and "DSCF0438.RAF"
74.4, and the wider camera names "_DSC0001.ARW" 78.1 and
"P1010001.RW2" 77.0. The before capture agrees: "4Z4A1919.dng" spans
74 px in it. We took 80 for the name, room for a wider system face,
and `space-2` either side: 96.

**The cell.** A picture narrower than the floor sits centered in its
cell, its ring 15 px in for a portrait (half of 96 less 65, in whole
pixels so the ring stays sharp). The ring and the hover's ground, a
`frame` inside the cell, hug the picture and the color band's room
under it and end there, on every cell: the name sits 2 px under the
ring and outside it, across the whole cell, so a name wider than a
portrait's ring never crosses it. The badges, the marks, the
version's tag, and the stack's badge keep their places on the
picture's corners. The cell's height is the ring's 99 (2, 91, the
band's 4, and 2), the 2 px, and the name's 17 (its 13 px line and
`space-1` for the descenders, which a 13 px box cut off a ".dng"'s
"g"): 118, 4 px more than §518's 114, which the viewport gives up.
`StripPlace` carries the picture's inset and width beside the cell's
left and width, from `Layout::photo`. A landscape (141) and a
panorama are wider than the floor and unchanged; a square (95) and a
sliver (50) take the floor. The running sum, `shown`, `reveal`, and
`held` read the cell widths and are unchanged: a portrait's pitch is
104 for 73. Longer names, such as a phone's, still elide.

**What it measured.** On the sample folder at 1500 wide, in a
`--snapshot` under headless weston: 13 frames whole and part of a 14th
before, 12 whole after, every name whole and the portraits centered
in their cells.

**The tests.** `strip::tests` check the floor for a portrait, a
square, and a sliver, a landscape and a panorama above it, the inset
of a narrow picture and none for a wide one, and the arithmetic of
the floor's pitch. In the window,
`a_portraits_cell_shows_its_name_whole_and_a_landscapes_is_its_picture`
checks the ring at the portrait's inset and the landscape's edges,
each ring the picture and the band's room high, and each name,
"4Z4A2978.CR3" and "DSCF0438.RAF", as wide as its cell, whole, and
below the ring.
