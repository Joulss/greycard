# §701. The hand-kept lists the compiler keeps (2026-11-15)

A second read of the code found that what a destructure enforces held
(the cache key takes every field apart, so a new parameter cannot be
missed) and what a hand-kept list enforces slipped. We moved two lists
to the first kind and fixed what the others had let through.

**What is learned by path.** `browser::file_moved` re-keyed about
thirteen maps one by one, and `Develop::monochromes`, a week old, was
not among them. `follow_paths` now takes `State` and its seven owners
apart with no `..`: a new field does not compile until it is followed
or named exempt under a comment saying why. That turned up three more:
`Views::reframed`, `Work::merge_edits`, and `Sync::followed`, keyed by
an archive copy's path, so a followed copy renamed outside the editor
lost its follow and its edits stopped going back. `browser::files_gone`
replaces the two `painted::drop_files` calls and also drops `stances`,
`mosaics`, `monochromes`, and `reframed`. With nothing at the old path,
whatever sits at the new one is dropped, so a file moved onto a gone
file's path inherits none of its stance. Nested types that keep paths
of their own (`visited`, `recent`, `stacks`, `collections`, the sheets'
`Asked`s) are named exempt and were not audited field by field.

**Settings' two writers.** The poll rebuilds the struct from the window
and `merged` copies the file-owned fields back. `merged` now takes the
kept struct apart whole, so a new field is placed on one side or does
not compile. None was on the wrong side.

**Notices.** `ported.txt` names ExifTool, whose tag tables seven
headers credit for the maker-note layouts (the reading code is ours).
`bundled.txt` has the AppImage runtime. `scripts/notices.sh` first
reads each source header as one block: one that credits a copyright
must name a project from the script's list, and each it names must be
in `ported.txt`. It cannot catch a credit without the word
"copyright", or a project off the list beside one on it.

**Scripts.** `hooks/pre-push` diffs a new branch from its merge-base
with the remote's default branch (it compared against the working tree
and so skipped the checks); with no base the push counts as code.
`install.ps1 -Remove` refuses a drive root, the user's profile, or a
folder without greycard's binaries; it is unrun here, and still takes
the whole of a folder shared with other programs. `bump.sh` takes the
pre-release versions `release.yml` takes and puts all three files back
when a lock file update fails.

`docs/architecture.md` now covers the output sharpen, the sharpen
guide's tap, the profile tone curve, and WebP.
