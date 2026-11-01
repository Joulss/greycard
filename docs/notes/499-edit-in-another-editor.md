# §499. Edit in another editor

*2026-11-01*

Lightroom hands a frame to Photoshop, Evoto, or Topaz and takes the
result back beside the raw. Testers asked for the same.

**One app, chosen once.** The Settings sheet's Edit in row names the
app and its Choose... button picks it: a `.app` on a Mac, an `.exe` on
Windows, a program or a desktop entry elsewhere. The frame menu gains
Edit in... after Export..., hidden until an app is chosen and hidden
again once the app is gone from the disk, so a stale choice never
writes a file no one opens. One app for now; a list of editors found
once is the roadmap's next step.

**The export is the export.** The frame right-clicked, a version as
itself, goes through the export's own set of one: a 16-bit TIFF in the
sheet's current color space, full size, no output sharpening,
watermark, or border, beside the raw under its stem and `-edit`
(`IMG_0001-edit.greycard.tif`, a version's name after `-edit`), with
Increment so nothing is written over. The status line ends on the
export's own line and the History line is written as for any export.

**Launched without a shell.** The file goes to the app as an argument:
`open -a` on a Mac, the program on Windows, the program or `gio
launch` of a desktop entry on Linux, `xdg-open` when gio is missing.
Only a desktop entry's Name= is read by us; its Exec line is gio's.
The child's stdin and stdout are closed.

**One watcher, a settle, a stack.** One thread looks at every file
handed over once a second, the disk asked with the list unlocked so a
slow share never holds the window's thread. A save is taken once the
file's time and size have held across two looks, so an in-place writer
is waited for and a temp-plus-rename save is caught at the rename. The
raw is hashed once, at the first look, a few tries at most; the file
only when a save is taken. Then the index takes the file, it joins
the raw's stack right after the raw (or a new stack of the two, the
raw on top), the stack is opened, and the list is read again so the
edit shows beside its raw. A later save re-indexes and does not stack
again. A watch is let go after a minute of the file being gone (only
when its folder answers, so a share that dropped out keeps its
watches) or an hour after its last save. A save on the window's thread
of the same name twice (the first file deleted) keeps one watch.

**Known limits.** An app that writes elsewhere (Affinity's own format,
Evoto's export) never comes back; the guide says the app must save
over the same file. Under a watched root the TIFF is listed as a plain
frame as soon as it is written, and joins the stack at the first save.
A raw-first tool (PureRAW, Photo AI) wants the raw, not a TIFF; that
stays on the roadmap.
