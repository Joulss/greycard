# §504. One instance

*2026-11-01*

Asked on discussion 51: a file double-clicked while greycard runs
opened a second window. Now it opens in the running one.

**Ask first, then become the editor.** A launch whose command line is
a path alone, or nothing, asks the running editor of the same user
and configuration folder before it starts: a Unix socket on Linux and
macOS, a named pipe on Windows. If one answers, the launch hands it
the absolute path (not canonicalized, so a mapped drive or a
symlinked root stays as the user spelled it and lands under the same
root a cold start would), prints one line naming the editor's pid,
and exits 0; the editor opens the path as the command line would (a
file in the Loupe, a folder as its library view, the last file
selected) through the Finder queue it already had, so a path that
arrives before the window can open it waits there, and brings its
window forward where winit allows (on Wayland the compositor
decides; the launch's activation token travels on the wire for the
day winit can use it). A launch with no path only brings the window
forward. Otherwise the launch binds and becomes the editor. The
editor answers only once the window's queue has the paths and never
while closing, so a launch it does not take opens its own window
rather than losing the file. Exports and every run with a flag (but
`--cpu-ops`, so the editor restarted after a GPU loss still listens)
never take part, and a launch that hands over logs to the terminal
only, so the running editor's log is not rotated away.

**Two launches at once, a crash before.** The connect-then-bind runs
under a lock file, so a socket left by a crash is removed only when
its connection is refused and two launches at once make one editor;
a launch that gets no answer in 10 s runs its own window and never
listens. The socket is removed on a clean quit, only when it is still
the one this editor bound.

**The other user on the machine.** The socket lives in the runtime
folder only when that folder is the user's alone (owned by the user,
no group or other bits), else in the cache folder; it is mode 0600
and each side checks the peer's uid. The Windows pipe is named by
the user's SID, the session, and the configuration key, carries a
DACL of the user and SYSTEM only, is opened for identification only
(no impersonation), and the launch checks the server's owner before
sending a byte; every pipe read and write has a deadline, so a
client that connects and sends nothing holds nothing. The name's
configuration key keeps a second profile, or a test run under its
own XDG_CONFIG_HOME, from handing its path to the user's editor.

**Known.** The Windows and macOS code is clippy-checked for their
targets, not run; the Windows pieces first run on CI. On macOS the
Finder's reopen of a running app (a Dock click) arrives as an Apple
event winit does not deliver, so that path still does nothing;
opening a document from the Finder reaches the running window as
before. An elevated editor on Windows cannot be reached by a plain
launch, which then opens its own window. The lock files stay beside
the sockets on purpose. Two narrow races are known: a launch queued
on the event loop microseconds before the quit is answered and never
opened; and on Windows the editor restarted after a GPU loss can
reach the dying editor's pipe inside its 200 ms poll, get no answer,
and run without listening (one retry of the claim would cover it).
