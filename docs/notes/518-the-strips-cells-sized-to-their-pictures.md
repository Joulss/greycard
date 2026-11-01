# §518. The strip's cells sized to their pictures

*2026-11-01*

The strip drew every frame in a 178 px box at a pitch of 186, the
picture fitted inside with 8 px of padding: a portrait showed 61 px
wide with 60 px of empty cell either side, a landscape left 20 px a
side and the gap, and the row stood taller than its pictures by the
paddings. Now each cell takes its frame's shape at the row's picture
height, as Lightroom's filmstrip does: a portrait narrow, a landscape
wide, a square square, 4 px (`space-1`) between cells and the 12 px
pad at the ends.

**The cell.** The picture is inset by the selection ring's 2 px alone,
so the ring and the hover's ground hug it; under it the color label's
band and the name, centered and elided to the cell's width. The badges,
the marks, the version's tag, and the stack's badge keep their places
on the picture's corners. The picture stays 91 px high (3:2 on its
side is 137 by 91, as before). The row is 10 px shorter, 138 for 148,
since the 8 px paddings above the picture and below the name are now
the ring's 2 and a 4: the viewport gets the 10 px. A picture is held
between 1:2 and 3:1 (46 to 273 px wide), so a sliver keeps a cell to
click and a panorama does not take the strip.

**The layout.** The shapes are the grid's (§460): `cells` keeps each
row's picture's height over width as it comes, zero before it has
come, which is laid out as 3:2 on its side (`grid::UNKNOWN_ASPECT`), so
a far row's cell takes its shape when its thumbnail arrives. `strip.rs`
holds the arithmetic: the cells' lefts as a running sum of widths, the
row's length, the rows a scroll shows (a binary search over the lefts),
and the least scroll that shows a frame with the pad beside it. The
window asks it through pure callbacks with the layout's number, as the
grid's layout is asked, and draws each cell where it says; Rust sets
the window's picture height, ring, and pad from the same constants.
`cells::strip_layout` caches the layout by that number.

**The scroll under late shapes.** A shape that changes a cell's width
moves every cell after it. When shapes come in, the window keeps the
selected frame on screen where it was, else the first frame on screen,
so a cell left of the view that grows or shrinks moves nothing in view
and a click where a frame was is still that frame; a selected frame
that was whole and grew past an edge is shown whole again, and a strip
at its start stays there (`strip::held`, the grid's rule of §460 in
one dimension). Every reshape then says again what the strip
shows, whether or not the scroll moved: on a cold cache the cells are
laid out as 3:2 until the pictures come, and portraits that narrow them
at a scroll of 0 bring rows on screen at the right that need cells and
thumbnails. The strip used to lay itself out afresh whenever its
length changed; that is now when its count changes, a new list, since
its length changes with every shape.

**What it measured.** On the sample folder at 1500 wide: 8 frames on
screen before, 14 whole and part of a 15th after.

**The tests.** `strip::tests` lay out a list of shapes (lefts, widths,
length), the rows a scroll shows, the reveal of a frame at the right
edge, and the hold under late shapes left of the view and of a whole
selected frame that grows. In the window,
`the_strips_cells_take_their_shapes_and_a_late_one_moves_nothing_in_view`
checks the drawn cells against the layout, delivers portraits for
frames left of a scrolled strip and finds the frames on screen where
they were and a click there on the same frame, and reveals a frame
past the right edge. Before that, at a scroll of 0, it checks that
the rows the portraits brought on screen are said, have cells, and are
the rows near the screen the worker makes pictures for; with the report
made only when the scroll moved it fails (rows 0 to 10 said, 0 to 13
shown). `testing::strip_cell` reads the window's layout
and scroll, and the strip's existing tests read the row's length from
it.

**Left for later.** A list rebuilt with the same row count as the last
(a folder of equal length opened with the same frame index chosen)
relays nothing and reports nothing, so its shown range can go stale
until a scroll; the report cannot run from inside the rebuild, which
holds the state the range handler needs.
