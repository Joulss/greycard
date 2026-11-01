# §484. Three asks: the log, held-out chips, and the folders

*2026-10-31*

Three small asks from the testers, one landing.

**The log, three runs deep.** Two reporters lost the run that
mattered: a relaunch rotated it to `greycard-ui.log.1`, which
GitHub's issue form refuses, and the next relaunch dropped it. The
kept runs are now `greycard-ui-1.log` to `-3.log`, newest first,
three of them: the bad run survives a relaunch to retry and another
to report, and the folder a tester drags from stays short. The kept
runs shift first; an old `.log.1` from a build before this one is
then taken in at `-2`, which is where that run falls in time, after a
downgrade and back included. A rename that fails partway can still
lose one older run, as the single file could.

**A chip held out.** Ctrl+click (Cmd on a Mac) on a flag, label, or
facet chip excludes what it names, drawn in the theme's red; a plain
click clears it; rating chips ignore the modifier, since "not 3+"
means nothing. The filter carries `not_*` sets beside its toggles.
For the index's facets a `NotFacet` term does the work, its SQL `NOT
COALESCE((...), 0)`, so a file with no value for the facet still
shows under the exclusion; keywords go by `NOT EXISTS`; flags and
labels are answered from each frame's own record. Chip counts still
ignore their own group, exclusions included. The filter's description
puts "not" before a held-out item, and a held-out No label reads
"labeled".

**The saved shape.** The saved filter writes the held-out sets only
when there are some, so files without them are unchanged. A smart
collection holding them is written at shape 3, one above what the
released builds read, so an older build leaves the file alone instead
of listing what the exclusions hide; clearing them writes shape 1
again. Picture versions stay at shape 2 in plain collections, which
hold no exclusions.

**The folders.** The four paths on Settings' About open in the file
manager when clicked, through the same spawn the reveal uses: `open`,
`explorer` with the path quoted by hand since Explorer splits on
commas, or `xdg-open`. A folder not yet made is made first. Moving
the library and the caches off the system drive is the other half of
that ask and stays on the roadmap.
