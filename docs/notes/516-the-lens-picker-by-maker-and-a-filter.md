# §516. The lens picker by maker, and a filter

*2026-11-01*

The Lens row of §497 lists every rectilinear lens the database has for
the body's mounts; on a Canon RF body that is a few hundred rows. Two
rows narrow it, both window state: the edit keeps the lens choice as
before, and nothing new reaches the sidecar.

**Maker.** Above Lens: All, then the makers of the body's rows,
alphabetically. A maker with no row for the body is not offered. The
database spells one maker several ways ("Nikon" and "Nikon
Corporation", "Olympus" and "Olympus Imaging Corp.", "Sigma" and
"SIGMA"); spellings sharing a first word (up to a hyphen, a leading
number kept, so "7Artisans" stays itself) are one maker, shown in the
shortest, mixed case before capitals. An Adobe profile's make is the
camera's, not the lens's, so filing by it would put a Lensbaby on a
Canon under Canon. A profile's maker is the first word of its lens's
name when the database knows a lens maker by it; the camera's maker
when the word's leading letters are one of the camera maker's series
in the database (the word after the maker in its lenses' names: EF,
RF, FE, XF, NIKKOR), so "RF 100mm F2.8 L Macro" and "EF24-105mm f/4L
IS USM" file under Canon; else that first word as written. Each row carries its maker from the engine's
`LensOption`.

**Find a lens.** On the Maker row, whose names are short, so Lens keeps
the panel's width for a lens's name: a field whose words narrow the
rows to those whose name and maker between them hold every one, case
aside, in any order ("24-70", "sigma art", "venus" for a Laowa). Maker
and words compose.

**The choice reads back.** From the file is always first. The chosen
lens is always a row: when the maker or the words leave it out, it
stands first after From the file; a lens not offered for the body (a
preset or a sidecar from elsewhere) is listed last under All, as
before. The keys stay the identity of a row, and the window holds the
whole list and derives the shown rows, so the index is recomputed
whenever they change.

**Following the frame.** A frame opened whose edit names a lens shows
that lens's maker; the words are cleared when another frame opens, kept
when the same one opens again, whether the open develops or not (an
offline frame, culling). A lens picked under All leaves All. When the
maker shown would hide the edit's lens (an undo, a paste, a preset),
the row turns to the lens's maker; From the file leaves the maker as it
was, so a run of frames picked from one maker keeps it.
