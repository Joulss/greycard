# §575. Preset favorites and groups

*2026-11-05*

A tester asked for the two things Lightroom gives a long preset list:
a star that lifts a preset to the top, and groups that fold away
(issue 72).

A group is a folder of the preset store and nothing more. The store
lists its own folder first, then each subfolder, the groups
alphabetically, and by name within each. No record names a group, so
a folder made, renamed, or filled in a file manager is a group made,
renamed, or filled, and a store copied to another machine brings its
groups along. One level only: a folder in a group is not read, nor a
folder whose name starts with a dot. A group's name is made one every
platform can have as a folder, a slash or any of Windows' refused
characters a dash and the ends trimmed of spaces and dots; one typed
in another case finds the group there. A move takes the file as it
is, so the preset keeps its name and everything it holds, and its file
name unless the group has one already. The folder a move or a removal
empties goes with it, so a group lives as long as something is in it.

A preset's name stays one across the groups. The save and the import
already looked for a name across the store's list, and they still do,
now across the groups too: a save of a grouped name writes that file,
and an import of one is refused as before. Every lookup sees every
group: the panel's, by a row's place in the list; the command line's
`--apply` and a headless export's, through `Store::find`; and the
import sheet's preset, by its own search of the list. A lookup by name
takes the first in the list's order. Two files that hold one name,
which only a hand in the file manager makes, are both listed, and a
name finds the one outside any group, else the one in the group first
alphabetically.

Favorites are the settings', not the store's: a list of preset files
by their path under the store, a slash between group and file. A path
rather than a name, since the name is inside the file and the list is
read before any file is. A move or a group renamed in the window
carries the stars along; one done by hand in a file manager loses
them, which we take as the price of keeping the store free of a
record. A preset removed takes its star with it. The folded sections
are kept beside the favorites, by key, and written as they change.
Each time the list is read, a star no preset file on disk has and
the fold of a group that is gone are dropped, so a star does not pass to a
later file of the same path, nor a fold to a new group of an old
name; not while the store's folder cannot be read, when every key
would look dead.

The Favorites section lists each favorite a second time, by name, at
the top, and the preset stays in its own section, so a star never
moves anything. The presets in no group sit under no header, just
below Favorites, so a store without groups or stars looks as it did.

A Lightroom preset keeps its group. Lightroom writes the group into
the file as `crs:Group`, which the reader now keeps; the import puts
a new preset in that group. Without one, the folder decides: a folder
handed to the command line's `--import` is a group of its name, and
each folder in it one of that folder's name, as Lightroom's own
`Settings` folder holds a group a folder. Files picked in the
window's chooser, which picks files and not folders, go into
no group but their own `crs:Group`: files picked together from a
downloads folder are no pack, and a folder rule made a "Downloads"
group of them. A Windows device name (CON, NUL) as a group, which no
folder can have, puts the preset in no group rather than refuse it.
Adobe's own presets write their name and group as a localization key
(`$$$/AgSettingNames/Group=Portraits`); the reader keeps the English
after the last `=`.

The new words, Favorites, the menu's five items, and the sheet's
title, sit in one Slint global, `PresetWords`, for review. The group's
name is asked on the collection sheet's card, with the app's own
Create and Rename, and a refusal (a name another group has) is said on
the sheet, which stays up.
