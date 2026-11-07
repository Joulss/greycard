# §587. The window's app id

*2026-11-07*

On GNOME the window could not be pinned to the dock and showed no
icon there; its WM_CLASS read NULL (issue 86). Slint's winit backend
names a window on Wayland (app_id) and X11 (WM_CLASS) only when an
app id has been set through `slint::set_xdg_app_id`, and nothing set
one.

The start sets it to `greycard` before the window is built, which is
when winit reads it. GNOME on Wayland matches a window's app_id to
the desktop entry's file name, and ours is `greycard.desktop`, so that
is the id; the entry's `StartupWMClass` changes from `greycard-ui` to
`greycard` so X11 matches the same way. The AppImage copies the entry
and gets both. Off Linux the call does nothing.
