# §623. The window opens as it was closed (2026-11-09)

Issue 114: maximize the window, close the editor, open it again, and
it opened at its default size and place. We kept nothing of the
window: each run opened at the preferred size fitted to the screen
(§306, §500), maximized only when a Windows shortcut's Run setting
asked.

**A record a window.** The settings file now holds `windows`, a map
from a window's name to its record; only `main` today. A second window
on a second monitor, which we expect, gets its own entry, and a name a
build does not know is read and written back untouched. A record is
the normal size, the corner, whether it was maximized, and the monitor
it was on (winit's name for it, its corner, and its size, in the
desktop's pixels). An entry that will not parse is left out with a
warning rather than setting the whole file aside; a file from before
reads as no record.

**Per machine.** The record sits in the window's half of the settings,
beside the interface scale and the system scale last seen, and never in
`shared`, the half a headless export and the command line read. Window
geometry means nothing on another machine.

**Points, from the monitor's corner.** The size is the window's inside
in the desktop's logical pixels: physical pixels over the monitor's own
scale, not over Slint's, which the interface scale multiplies. A change
of the interface scale between runs then leaves the window covering
what it did: 1500 x 900 points opens at 1000 x 600 of Slint's pixels
under 150 %. The corner is kept from the monitor's corner in that
monitor's points, so it follows a monitor moved in the arrangement or
rescaled, and macOS, whose physical desktop pixels mix scales, is exact
on the window's own monitor. The corner is the frame's on Windows and
X11 and the inside's on macOS, which is what each platform creates a
window at.

**The normal size, not the maximized one.** Each resize and move of the
window is read with its state. One while maximized says only that it
is maximized; one while full screen says nothing; one while minimized
or with no size says nothing. A normal reading is taken once the next
event finds the window normal too, or at the close; at a maximize or
full screen it is taken unless it is that rectangle's own, by its size
or its corner, since Windows reports the move into a maximize before it
says maximized. A window closed full screen opens as it was before,
maximized or not, at its normal size. A window opened maximized and
never restored keeps the size kept before. A snapped or half-tiled
window (Aero Snap, Mutter's or KWin's tiling) is not maximized to
winit, so it is kept as the normal size and reopens unsnapped at that
rectangle.

**Nothing from the close.** Once a close is asked, and once the window
is hidden, no reading is taken; the event loop's end freezes the
tracker for good. On X11 a closing window is withdrawn: unmapped, then
reparented to the root at its frame's corner, which winit reports as a
move, and `outer_position` takes the frame's extents off a corner that
is already the frame's. Kept, that reading crept the window up by its
title bar each run on every window manager (37 pixels on Mutter, 32 on
weston). A close refused (an import running asks first) reads again at
the window's next move while it is still shown.

**A maximize on X11 is asked twice.** Slint creates the window hidden,
so a maximize set before it exists reaches winit as a `_NET_WM_STATE`
message to an unmapped window, which Mutter passes over (as EWMH
allows; KWin likely the same): the window opened normal and the close
wrote it so. It is asked again through the winit window at the first
event once it is shown, and nothing is read in between.

**The scale's correction waits.** When the guess of the system's scale
was wrong, the first event sets the window to the size it was fitted
to; on a window opened maximized that resize would take it out of the
maximize, so the correction is owed and set the first time the window
is normal after the platform has shown it maximized or full screen. Not
before: on X11 the maximize Mutter passed over is asked again only once
the window is shown, and a correction set at an event in that gap (the
window not yet maximized) fought it. Under Mutter with the interface
scale at 150 % and a wrong last-seen system scale (2 for a true 1), a
window kept maximized opened at 1680 x 1509 on a 1920 x 1080 screen
and was kept not maximized.

That case showed a second fault, older than the kept window: Slint
hands the layout's least on to the platform only when the logical
constraint changes, which a new factor does not, so after the first
event's correction X11 held the least at the guessed factor's pixels
(1680 x 1509, larger than the screen), and no resize went under it and
no maximize fitted. The correction now sets the least again at the new
factor. With both, the same window opens maximized, and un-maximized
takes its fitted size, 1200 x 754 pixels (800 x 503 at 1.5, the height
held at the layout's least).

**Still fitted.** A kept size goes through the same fit. On its monitor,
still there by name and size (and place, where two could be it), it is
fitted to that monitor's room, the work area where the platform says;
elsewhere, or with no place kept, it is fitted as a new window is and
placed as one. The place is used only when at least half a title bar's
height and 120 points of its width (or all of a narrower window) land
on a monitor, and is then moved in so the window fits its room. A
window is never opened off the screens.

**What wins.** A Windows shortcut's Run: Maximized still opens it
maximized. `--window` (the hidden size flag captures use) wins over the
kept size, and such a run, a capture, a timing run, and one sent keys
neither read nor write the record, so every capture recipe opens as
before. The test harness has no winit and reads none.

**When it is written.** At the close and at a quit by key or menu, with
the panel's choices, through `Choices::write`; the once-a-second write
of a changed choice keeps the file's record. A crash loses the last
geometry, and a close while the settings file is set aside (it would
not parse at the start and nothing has written it since) writes no
record, as it writes nothing else.

The log says what was done, before the fit's own line:

    window: kept 1200x700 at 95,155 on headless
    window: kept 1200x700; its monitor is gone or changed, placed as a new window
    window: kept 1200x700, maximized; the compositor places it

**Wayland** lets a client ask no place, so there the size and the
maximize alone, silently.

## Tested

Unit tests: the readings while normal; maximize then close (with the
Windows order of events); one resize then a maximize keeps the resize,
and a last reading at the maximized size is dropped; nothing read after
the freeze, and read again once a close is refused; full screen then
close, from normal and from maximized; restore on its monitor, and on
that monitor moved in the arrangement; the monitor gone; two alike and
unnamed taken for neither; a big window onto a 13-inch Air's work area;
places off each side dropped and one partly off moved in; the interface
scale changed (100, 150, 50, by hand, Wayland); a record with no size;
a file from before; an unknown window and a broken entry; the round
trip; the scale's correction owed on a maximized or full-screen window
and due once it is normal. A harness test applies a record to the
window (size fitted, place, maximized) and checks that `--window`,
`--snapshot`, `--screenshot`, and `--keys` keep none; another shows the
timer's writes leave the record alone and the close writes it beside an
unknown window's.

Under a headless Mutter 51.1 at 1920 x 1080 through its Xwayland,
before the fix: a window moved to 300,150 and closed was kept at
300,113, then 76, 39, and 2 over three more runs; maximized and closed,
it was kept as 1920 x 1043, not maximized. After: the same window kept
300,150 through three runs and reopened there; maximized and closed, it
was kept at 1200 x 700 at 300,150, maximized, reopened maximized twice
(the log saying it was maximized again once shown), and un-maximized
to 1200 x 700 at its place. Under weston, earlier: `--window 900x600`
neither read nor wrote; at 150 % the window covered the same pixels,
held to the layout's least; on Wayland the log said the compositor
places it.

## Known

Unproven on Windows and macOS, built from winit 0.30's sources and
the pinned Slint fork's, and checked for neither target here (the
cross builds need a C compiler for the target we do not have):

- Windows: winit creates the window at the default place, sizes it,
  then moves it with `SetWindowPos` to the outer corner we give, and
  maximizes after creating, so the normal rectangle is ours; a kept
  monitor at another scale than the one Windows opens on resizes the
  window by the DPI change, which we have not seen. The event order we
  allow for (a move reported before the maximize) is read from
  winit's `WM_WINDOWPOSCHANGED` and `WM_SIZE` handling.
- macOS: winit creates the window at a content rectangle, its corner in
  points. At 100 % we hand Slint a logical position in points, which it
  passes on unconverted. Under an interface scale other than 100 % (or
  `SLINT_SCALE_FACTOR` by hand) Slint makes pixels of a logical
  position at its factor and winit points again at
  `NSScreen.mainScreen`'s scale, so there we hand pixels at the primary
  display's scale; with displays at two scales the main screen at
  launch may not be the primary, which would misplace the window by the
  ratio. The title-bar check treats the content's top as the window's.
