# §545. Four small asks from discussion 59

*2026-11-04*

Four asks from the testers on discussion 59, each a paragraph.

**Edge arrows off the crop's handles.** The Hide panel arrows are drawn
over the viewport, so a crop as wide as the view had its side handles
under them, and a handle under an arrow never sees the pointer. Drawn
under the viewport instead, the arrows would sit under its picture-wide
press area too, and hiding them while the pointer is over a handle
cannot work for the same reason. So with the crop in hand, an arrow
whose column meets the crop's handles on its edge moves along that edge
until it clears all three of them (below the side handle, else above it,
else below or above the crop), and stays a working arrow there.
Elsewhere, and out of the crop, it keeps the middle. The strip's arrow
does the same along the foot, off a full-height crop's bottom handles:
right of the middle handle, else left of it, else beside the crop
(discussion 59).

**The last shape deleted.** A shape's × showed only while its mask had
more than one, so a mask whose only shape was wrong had to be deleted
whole. We show it on the last shape too. The mask stays, empty, as New
mask leaves one: an empty mask is already no place in the develop, the
export, and the thumbnails (`Mask::is_empty` turns the local off
everywhere), and the history step reads "… shape removed" as any other
shape's delete (discussion 59).

**Esc puts a shape's handles down.** A chosen linear or radial keeps its
lines and handles on the picture while its sliders move, which hides
what they change. Esc, last of Escape's handlers, so a sheet, a tool in
hand, the crop, an export, a set, the lightbox, the grid, culling, and
an import still take it first, and only with no tool in hand and no
field typing, puts the handles down: a window flag the viewport reads to
draw neither the outline nor the handles, the shape still chosen. A
press on the shape's row or on its body on the picture brings them back,
as does another shape, mask, or frame. No new UI text (discussion 59).

**A line under New mask.** The mask list and New mask stay put while the
sections scroll under them, and a section card scrolled up under the
button read as drawn round it. A hairline in the theme's border color,
the one the panel's foot uses, now closes the pinned block, with the
block's padding below the button, and the list's fitted row count counts
the 9 px (discussion 59).
