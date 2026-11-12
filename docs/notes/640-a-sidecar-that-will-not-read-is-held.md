# §640. A sidecar read from disk that will not read is held (2026-11-11)

A frame's `.gcd` that will not read (a later build's, its edit's
schema past ours, or one damaged) was held only when the frame had an
index row: listed, standing in, nothing written over it. A frame with
no row (a folder not yet indexed, files on the command line, the index
off) was read from disk by `rows::bring`, which handed back the
default edit marked as read and dropped the trouble. The frame was
then writable, and its next rating, edit, preset, paste, or sync wrote
the default over the later build's file. Whole edits lost.

`bring` now holds such a frame as the row case is held: not read, the
same status words ("could not be read; nothing is written over it").
Every window write already refuses a frame not read (`file_writable`,
`load_frames`, `refused`), so nothing else changes. `take`, which held
an unreadable read only over a frame still standing in, now holds it
over a loaded frame too: a read again after another writer (the
Lightroom or RawTherapee import) that finds the file unreadable keeps
what the window held and writes nothing.

What the user sees: the frame listed with the camera's picture, no
pencil, and the status line's words when anything is done to it. No
new strings.

A damaged `.gcd` on a frame with no row is now held too, as one with a
row already was, and nothing in the window starts it over. Move or
delete the `.gcd` by hand, then open the folder again; the guide's
"Where your edits live" now says so in one sentence.
