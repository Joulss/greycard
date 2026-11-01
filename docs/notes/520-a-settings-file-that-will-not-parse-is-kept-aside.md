# §520. A settings file that will not parse is kept aside

*2026-11-01*

A settings.json that failed to parse was written over with the
defaults. The close's write always wrote, merging the panel's choices
over a read that had fallen back to the defaults, and every field
written as it changed (the update check, the lens offer, the match
sheet's ticks, the recent folders, and the rest) read the defaults and
wrote them back. Only the minute's write held off, and it said the
close would write anyway. A hand edit with one typo lost the whole
file.

Now the window's read sets it aside. A file that does not parse is
renamed to `settings.json.bad`, over an older one, and the log names
the path and the parse error; the run goes on with the defaults. A link
is left pointing where it did: what it points at is copied aside
instead, and the next change is written through it. A file that cannot
be read for another reason (permissions, I/O) only warns, as before.

A headless run (`--import`, `--export`) writes no settings, so it reads
a broken file as the defaults with a warning and leaves it where it is,
for the window's next start to set aside. The import's read
(`load_untouched`) now agrees with the export's (the engine's
`Shared::load_from`): a run that writes nothing has no reason to move
the user's file.

Every read-change-write goes through one path, `Settings::change` (the
user's file) and `Settings::change_at` (a window's `settings_file`). It
reads, which sets a broken file aside; applies the change; and writes
only when the change changed something. So after a set-aside the next
write is the defaults plus that change, which is the honest state, and a
change of nothing writes nothing. The three sites the review named
(update.rs, assets.rs, camera_match.rs), the recent folders, the eight
other load-then-save sites, and the last file all use it, and
`load_parsed` is gone.

The close's write and the minute's write are one `Choices` value: the
choices last written, and whether the run started on a set-aside file.
The minute's write goes when a choice has changed since the last write.
The close writes as it always did (it is the one write of the system's
scale), except when the start set the file aside and nothing has
written the file since: the defaults on show are no one's choice. Any
write of the file ends that hold, the update check's or a Settings
switch's as much as the panel's own; `save_to` notes each file this
process has written, and the close asks.

The update check is on by default and due at once on the defaults, so
on most launches a fresh file with the defaults and the check's time
reappears a few seconds in, and the close then writes as usual. Only a
run offline or with the check off, that changes nothing, leaves no
settings.json, and the next launch reads as a first run. We accept
that: it starts on the defaults, which is what a first run is.

A `--snapshot` or `--screenshot` run reads the user's file at the start
as the window does, so it too sets a broken file aside; it writes
nothing.
