# §577. Edit in, several apps

*2026-11-05*

§499 kept one app and named a list as the next step. A frame may go
to one editor for skin and another for a composite, so the one app
becomes a list.

**A list on the Settings sheet.** The Edit in row lists the apps one
a row by name, each with a × that takes it off, and Choose... becomes
Add..., which picks one as Choose... did and puts it at the end. The
order is the order added; an app added again stays where it is, the
two paths compared as files once both resolve and as written when
not, so a link or a `..` to one already there adds nothing. An
app gone from the disk stays on the list, named by its file, until it
is taken off: an unplugged drive should not cost a choice.

**One item or a submenu.** The frame menu is built from the apps on
the disk each time it opens, not only at a press as before. Each
app's name (a desktop entry's own read from it) is worked out once,
as it is added or the settings are read, and kept beside its path;
the menu's build is one stat an app and nothing more. One is
Edit in... as it was; two or more are an Edit in submenu of their
names; none leaves it out. A press names the app by its place among
those offered, and the app rides with the export to the hand-over,
so a list changed meanwhile never sends the file to another app. The
export, the watcher, and the stacking are §499's.

**The file reads the old form.** `edit_in` is a list of paths now. A
string, the one app earlier builds kept, reads as a list of it (an
empty one as none), so no choice is lost, and the next write of the
file, any change or the close, writes it as a list. That is the
file's way: no version field, a lenient reader per field, as for the
theme and the keys. The reader keeps each app once, by the same
comparison as Add....

**Known limits.** The stat at the menu's build is on the window's
thread: an app on a mount that has stopped answering (a dead NFS or
SMB share) holds the menu until the stat gives up. Apps are local
almost always, and a thread for one stat a menu is more than it is
worth; we accept it. The duplicate check and the settings' read
resolve paths the same way, once each. Two apps of the same name (two
versions of one editor) show the same name in the submenu and on the
list; their order tells them apart, and we accept that too.
