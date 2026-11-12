# §645. A list read before the frame on screen moved does not list its old path (2026-11-12)

A merge (`roots::kept_up::merge`) gives each file new to the list the
sidecar the read brought for it. One the read did not bring took a
default sidecar marked read: writable, so its next rating or edit would
write the default over the file's `.gcd`, the hole §640 closed in
`rows::bring`.

`land` reads again when any listed file is neither listed by the window
nor brought, so that fallback looked unreachable. It was not: the merge
follows the frame on screen to its new path (`follow_current`) after
that check, which takes the old path out of the window's list. A read
that listed the folder before the frame moved, and found it moved when
it looked for the frame on screen (a pass in between), still lists the
old path, which then counted as new, with nothing brought. The old path
was listed beside the followed frame, blank and writable; a raw moved
without its `.gcd` left the `.gcd` there to be written over.

We take the frame on screen's old path out of the list whenever the
read found it gone, followed or not, as a gone frame not followed always
was. The fallback stays for safety but no longer claims a read: it
stands in, not read, so nothing is written until a request reads the
file from disk (§640's hold). A `debug_assert` says it is not reached.

Tested: a list read before the frame on screen moved to another folder,
landed after the move, lists the frame once, at its new path, with its
rating, and not its old path. It failed before the change (four files,
the old path among them).
