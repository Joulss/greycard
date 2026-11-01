# §483. Natural scrolling on Windows and GNOME

*2026-10-31*

§432 turned a slider's wheel back under a Mac's natural scrolling and
left Windows precision touchpads and GNOME touchpads, natural by
default too, reading as not. Now their settings are read as well.

**One flag was wrong there.** A Mac's setting turns the trackpad and
the mouse wheel together; Windows and GNOME turn the touchpad alone,
and the mouse keeps its own. One flag for all would have turned every
mouse wheel beside a default touchpad, this machine's included. So the
read gives two values, the wheel's and the touchpad's, and the
slider's flag starts at the wheel's. Where the two differ, the
window's winit handler sets the flag before each wheel event from the
event's device: on Wayland pixel deltas are the touchpad and line
deltas the wheel; on Windows the message's source says touchpad. The
brush's wheel follows the same flag. On a Mac the two values are one
read, so the per-event step does nothing and nothing changes.

**What is read.** Windows: the precision touchpad's `ScrollDirection`
under the user's registry, natural when 0 or missing. GNOME, only
when the desktop names itself GNOME: gsettings' `natural-scroll` for
the touchpad and for the mouse, a failed read counting as not. KDE
keeps its setting per device under names that differ by version and
is left out.

**Off the start-up path.** gsettings on a session bus that accepts
and never answers sits forever, found by a silent socket in review.
The read runs on a thread after the window is built and hands its
result to the event loop; a wheel event in the window's first
milliseconds moves the old way, every one after it the new. The same
silent bus still stops the window in winit's own decorations on
GNOME, which is winit's to fix.

**Not done.** X11 delivers a touchpad as lines, so it counts as the
wheel there and stays wrong. A TrackPoint or a trackball scrolling in
pixels on GNOME Wayland counts as a touchpad; winit gives no better
signal. The Windows source check has not yet run on a Windows machine:
if a precision touchpad's wheel message reports no source, nothing is
turned, which is today's behavior.
