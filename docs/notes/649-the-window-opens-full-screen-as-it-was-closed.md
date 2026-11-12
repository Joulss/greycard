# §649. The window opens full screen as it was closed (2026-11-12)

Issue 125 asked for the editor to start full screen. Since §623 the window
kept its normal size, its maximize, and its place across runs, but not full
screen: closed full screen, it opened as the window under it. Now full screen
is kept too. Quit full screen, start full screen; leave it and quit, start as
before. No new text: no menu item, setting, or tooltip.

**One more field.** The main window's record (`windows.main`) gains
`full_screen`, written only when true. A missing field reads as not full screen,
so a file from before reads as it did and a file that never had it is written
unchanged. A build from before has `#[serde(default)]` on its record and does
not refuse unknown fields, so it reads a newer file and ignores the flag.
Written back by that build, the flag drops out, and the next run opens as the
window under it. A window name this build does not know is still kept as read,
flag and all.

**Over the window, not in place of it.** The tracker records full screen as it
last saw it, beside the normal size and the maximize, which it still takes only
from readings outside full screen (§623). The record therefore holds the window
under full screen whole. A run that opens full screen and stays there to the
close writes back the size, place, and maximize it read, with the flag. Leaving
full screen after a restart returns to the earlier window, not to one the size
of the screen.

**Asked once shown, the F11 way.** The window opens at its kept size and place,
maximized if it was, and then goes full screen. On X11, a full screen asked
before the window exists would go to an unmapped window, the trap §623 hit with
the maximize. Opening full screen on its own also leaves the platform no normal
window to go back to. So the ask comes at the first event that finds the window
shown, after the X11 maximize asked again at that point (§623). It goes through
F11's own way in (`full_screen::reopen`), which keeps the maximize to put back on
leaving (§436), taken from the record, since X11 may not report the window
maximized yet. The ask runs on a zero timer after that event. Slint handles each
event after our handler and sets its full-screen state to winit's, still false
then, which would cancel an ask made within the event.

**Nothing read until it takes.** Between the ask and its taking, X11 and Wayland
report the window normal at its opening size. Such a reading would replace the
kept size or clear the maximize. A tracker started for a full-screen opening
reads nothing until an event finds the window full screen, and then reads as
before.

**The monitor gone.** The window is placed as a new one (§623), and full screen
goes to whichever monitor it opened on.

**Not the user's choice.** Nothing else in the editor sets full screen. The
lightbox, the loupe, Cull, and the crop are views inside the window (§307, §436),
so none can set or leave the flag. Full screen entered by the platform itself (a
Mac's green button, a window manager's key) counts as full screen: we read the
window's state, not the key.

**Runs that keep nothing.** `--window`, `--snapshot`, `--screenshot`, `--keys`,
and the timing runs neither read nor write the record (§623), flag included.
Headless `--export` has no window and reads only the shared half. A second launch
hands its path to the running editor and ends before it reads settings. The
running editor brings its window forward without touching full screen, so a
full-screen window stays full screen.

**Per platform.**

- Linux X11: maximize asked again, then full screen, at the first shown event;
  winit reports the requested full screen at once.
- Linux Wayland: no place. Size and maximize as §623, then full screen at the
  first event Slint shows; winit reports full screen from the compositor's
  configure, and the tracker waits for it.
- Windows: the window is made at its normal rectangle, maximized, then full
  screen. winit saves that placement on entering, which leaving restores.
- macOS: the menu's Full screen (no F11) and the green button. winit's full
  screen there is a native Space, entered just after the window first shows.

**Tested.**

Unit tests:

- the flag kept over a maximized window, and cleared on leaving;
- a run opened full screen ignores the window it opens as, keeps the size,
  place, and maximize to the close, and reads again on leaving;
- a kept full screen restored on its monitor, with the monitor gone, and on
  Wayland, with the log line;
- the file: absent, set, cleared, a record from before, the released build's
  record reading a newer one, an unknown window's flag kept, and the close
  writing and clearing it;
- the settings round trip with the flag;
- in the harness, `reopen` from a maximized window and a normal one, with F11 out
  to each.

**Not tested.**

The harness has no winit, so none of the following ran: the first-shown ask, the
timer, the platform's restore on leaving, or the tracker against real events.
They need a person at each platform:

- X11 (Mutter, KWin) and Wayland (Mutter, KWin): normal → F11 → quit → start
  (full screen) → F11 (back to the earlier size and place). The same from
  maximized: back maximized, and un-maximized to the earlier size.
- The same, quitting while still full screen on the second run: the third run is
  full screen again, and leaving it still returns to the first window.
- Windows: the same three, and a Run: Maximized shortcut over a full-screen
  record.
- macOS: the menu's Full screen and the green button. Quit in a full-screen
  Space and start: whether it enters full screen when asked right after showing.
- Unplug the kept monitor and start: full screen on the remaining one; leaving
  it gives a fitted window.
- A launch with a file while the editor is full screen: it stays full screen.

**A refused ask.** A compositor may keep the window normal. Wayland takes full
screen from the configure, so the ask can be refused. Readings made while we
wait are held aside, not dropped. If full screen takes, they go, so a late
configure leaves the kept size alone. If the run closes without full screen
ever taking, the close keeps them as an ordinary run would, with the flag
cleared. Otherwise the flag would stay set, the ask would repeat every run, and
no resize, move, or maximize would ever be kept again. Deciding at the close
needs no grace period to tune.

The handler's choice per event is now a pure function (`window_fit::owed`):
wait, ask the maximize again, ask full screen, both, or read. It is tested,
including a plain maximized start on X11: it waits until shown, asks once, reads
nothing at that event, then reads as any run.

**Two limits.** A run started full screen and quit within the instant
before full screen takes writes the flag cleared, with the window
under it intact: the next start is a normal window. And where it
read nothing at all, such a run now writes the earlier record with
the flag cleared, where §623's rule left the record alone. On X11
winit reports the full screen asked, taken or not, so a window
manager that refuses it reads as full screen and the refused-ask
rule never applies there.

**Seen.** Under a headless weston, with a record of a maximized
window and the flag: `set_maximized` before the first commit, then
`set_fullscreen`, and the compositor's configure at the whole
output. The write at a real close was not seen live.
