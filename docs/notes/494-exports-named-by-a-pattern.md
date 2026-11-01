# §494. Exports named by a pattern

*2026-11-01*

Two roadmap lines (§157, §167): a name pattern on the export sheet,
kept in a preset.

**The tokens.** A Name field holds a pattern of `{name}` (the source
file's stem), `{seq}` (the frame's place in the export from 1, as
wide as the last number needs, `{seq:101}` to start there,
`{seq:0001}` at least that wide), `{date}` (the capture day as
YYYY-MM-DD with the frame's time shift applied, `{date:%Y%m%d}` with
`%Y`, `%y`, `%m`, `%d`, and `%%`), and `{camera}` (make and model as
the import shows them), among literal text. The default is `{name}`,
so nothing changes for anyone. An unknown token, an unclosed brace,
or an empty result falls back to `{name}` with the reason in the log;
the line under the field shows the first selected frame's real file
name, version suffix and the `.greycard` infix included, in the
warning's color for a fallback. The expander is the import's, now
shared, so import patterns take `{seq:N}` and `{date:FMT}` too.

**Where it lives.** The pattern is a field of the export sheet's
settings, so presets, the last settings, Ctrl+E (§475), queued sets,
and the headless export all carry it, and an older file reads as
`{name}`. The window fills the stems once an export and hands them to
the queue's naming, which adds the version, the infix, and the
numbering as before; a collision gets `-1`, `-2` as duplicates always
did. The sheet's Folder row is the destination.

**No whole-file reads.** `{date}` and `{camera}` come from the
library index's row; only a frame with no row is read, through the
sparse metadata read (§491), about 200 KB, once while the sheet is up.
On review the first build read every byte of every file on the
window's thread, twice under Overwrite, about two minutes for 300
frames over a share. The overwrite question now asks about the names
the set will write, a version's included; before, it checked plain
stems, so a version's existing file was written over without asking
and an unrelated `-1` file asked for no reason.
