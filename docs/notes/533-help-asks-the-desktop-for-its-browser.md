# §533. Help asks the desktop for its browser

*2026-11-02*

Issue 60: on a Linux desktop whose default browser is Brave, Help
opened the guide in Chrome. The `webbrowser` crate (1.2) tries
`$BROWSER` before anything of the desktop's, so a `BROWSER` a shell
profile set for the terminal beat the desktop's choice. That order
suits a command-line tool; a window should ask the desktop.

Every page the window opens (the guide, the bug report's form, and
the release page) now goes through one helper, `web::open_url`. On
Linux and the BSDs it runs `xdg-open`, or `gio open` where there is
no `xdg-open`, and falls back to `webbrowser` when neither is on
`PATH` or the one found fails. `xdg-open` reads `$BROWSER` too, but
only after the desktop's opener and the `x-scheme-handler` default
have had their turn. macOS and Windows keep `webbrowser`, which asks
the system there already.

The opener runs detached: no handles of ours, its own process group
so a Ctrl-C in the starting terminal does not reach the browser, and
the environment scrubbed as for every other command we start (§374).
`xdg-open` waits for a browser it started itself, so its status
counts only within two seconds; a failure in that window passes the
page to `webbrowser`, and after it the child is reaped on a thread of
its own. Where `webbrowser` fails too, the guide and the report's
form say "no browser would open" on the status line; the release
page only logs it. A test checks which program would run for a
stand-in `PATH`, without running it.

The AppImage leaves `PATH` alone and sets no `LD_LIBRARY_PATH`, so
the host's `xdg-open` is found and the browser it starts loads
nothing of ours.
