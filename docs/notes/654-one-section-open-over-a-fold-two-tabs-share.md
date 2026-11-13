# §654. One section open over a fold two tabs share (2026-11-12)

**Found.** Issue 143: under One section open at a time (§397), Develop
showed LIGHT and SHARPEN open together. Open SHARPEN on Develop, choose
a mask, open LIGHT on the Masks tab, and go back: both are open. The
rule folds the sections that are made, and only the tab on screen has
its sections made. But Light through Detail and the white balance are
one fold on Develop and on a mask's look, and METADATA is one on
Develop and Cull. A shared fold opened on one tab left the other tab's
sections, unmade, open.

**Decided.** The rule stays a tab's. A shared section opened under it
now also folds the other tab's sections that are not made: on a mask,
Develop's own; on Develop, the mask's MASK section, or for METADATA,
Cull's CULLING; on Cull, all of Develop's. The Crop and Retouch tabs
share nothing and keep theirs.

The two lists of sections are written out in `app.slint` beside the
tabs' conditions and must follow them. A key on each section, with the
last opener's kept in `Folds`, would need none, at about the same
change across some 25 sections; we left it until a third tab shares a
fold.

**Left as it was.** A settings file saved with two open still opens
with two, until the next opening. The grid's METADATA still leaves the
panel behind it alone, as §397 chose.
