# §500. The window fit to the work area

*2026-11-01*

The first-launch fit (§306) guessed a margin of 80 by 140 for the
Dock, the task bar, and a title bar. The guess held on a 14-inch
MacBook with the default Dock and failed on a large or side one: the
window opened short, or with its foot under the Dock.

**Ask the platform.** Each monitor's work area is read where the
platform reports one: NSScreen's visibleFrame on macOS, scaled to
pixels as its share of the frame so a scaled mode comes out right;
GetMonitorInfoW's rcWork on Windows; the window manager's
_NET_WORKAREA for the current desktop on X11, clipped to each monitor,
over a connection of our own through x11rb (winit's is Xlib behind a
pointer). Wayland tells a client no work area, and an X11 window
manager may set none; there the guess stays. Inside a known area the
margin is a frame and title bar, 16 by 48 in the system's pixels, held
under an interface scale below 100 % since a title bar is sized by the
OS and not by the setting. The fit still picks the smallest room, and
the log line says the work area when one is known.

**Windows places the window too.** winit creates a window at
CW_USEDEFAULT and resizes it without moving it, so Windows cascades it
from the work area's corner and nothing keeps it inside. There the
window is also centered, through Slint's position before the window
exists, in the work area of the monitor Windows would have opened it
on: the one the process was started from when the startup info names
one, else the primary, and the smallest room's only when it does not
fit there. macOS centers in the visible frame and an X11 window
manager places the window itself, so those are left alone.

**Known.** One _NET_WORKAREA covers every X11 monitor, so a panel on
one narrows the others. The Mac and Windows paths were built and
reviewed from the sources and clippy-checked for their targets, not
run; a 1440 by 900 Air with a large Dock now opens shorter than
before, since the old guess put the foot under the Dock.
