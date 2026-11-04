# §543. The window's second merge under the merging folder

*2026-11-04*

A follow-on to §537's last leftover. The window's panorama holds its
name once (`merge::hold`: the frames check, the name settled, the
`.<name>.merging` folder and its lock, §530) and writes each run
under it (`pano::pano_under`) as `first.dng`, then, when the
reference's edit walks apart from the anchor's, `second.dng`. The one
kept is renamed onto the name (`Reserved::promote`); the other goes
with the folder. A second run that fails leaves the first to keep,
with the anchor's edit as its look; the log says why, the status
says the merge as ever. A first run that fails leaves the folder
removed and nothing at the name.

The folder's name, its lock, and the rename stay in one place: `hdr`,
`pano`, and so `again` now end in the same `hold` and `promote`. The
HDR arm of the window is unchanged.

Nothing is renamed over a file the window's merge wrote, so the
Windows case §537 named (the second rename while something holds the
first file) no longer arises. `promote` retries nothing: the rename
of a merge made again over an open file is still untried on Windows
(§541).

## Left

- A crash during the second run leaves `first.dng` in the hidden
  folder beside the spilled frames §530 lists, until the next merge
  to that name clears it; the library's tree pass skips dot-folders,
  so it is never indexed.
