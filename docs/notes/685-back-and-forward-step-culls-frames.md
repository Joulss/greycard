# §685. Back and forward step Cull's frames (2026-11-15)

**Caught before Slint routes it.** Slint gives a press to the topmost
TouchArea under the pointer, whatever the button (`TouchArea::input_event`
grabs every `Pressed`), and a parent TouchArea's filter sees the press but
never calls its `pointer-event`. So no root-level TouchArea hears a side
button over the viewport, a strip cell, or a pane's slider; §249 found the
same and had the grid's every TouchArea ask `side-press`. Doing that across
Cull (viewport, strip, cull section, panes, scroll bars) would be a dozen
TouchAreas and every new one a place to forget, and the viewport's press
already decides among menu, dropper, shape, and pan. An overlay would
swallow clicks. Instead `panel::side_buttons` installs the window's winit
event filter (`WinitWindowAccessor::on_winit_window_event`), which runs
before Slint dispatches the event. It asks the window's
`side-button(back, shift)`: in Cull, with no grid, lightbox, sheet, menu
(`menu-up`), typed field (`Typing.editing`), or text box (`Typing.boxes`),
it calls `step(∓1, shift)`, the arrows' own callback, so the clamp at the
ends, Shift's adding, and the prefetch are theirs. A press taken returns
PreventDefault, and so does its release; one not taken propagates, so the
grid's `side-press` (folders visited, §594) and everything else is as
before. Other buttons, the wheel, and focus are never touched.

**Press, not release.** Windows apps conventionally navigate on
the release (`DefWindowProc` makes `WM_APPCOMMAND` of `WM_XBUTTONUP`), GTK
apps on the press. We act on the press: the grid's side buttons
already do, the arrow keys act on their press, and a step on press feels
immediate. A button never repeats; a second press with no release between is
not taken again, and focus lost clears what is held.

**Delivery per platform** (winit 0.30.13, Slint's winit backend maps
`MouseButton::Back/Forward` to `PointerEventButton::Back/Forward`):
- X11: buttons 8 and 9.
- Wayland: `BTN_SIDE`/`BTN_BACK` and `BTN_EXTRA`/`BTN_FORWARD`.
- Windows: `WM_XBUTTONDOWN`/`UP`, XBUTTON1 back and XBUTTON2 forward.
- macOS: `otherMouseDown` with buttonNumber 3 and 4, the vendors' usual
  pair; a mouse whose driver maps the buttons to keystrokes (Logi Options,
  for instance) sends those keys instead.
Only the winit backend has the filter; the testing backend and any other
deliver nothing to it.

**Unverified.** No real mouse was pressed. The tests feed winit
`MouseInput` events straight to `SideButtons::event`, which calls the
window's `side-button`; winit's own call of the filter, and the platform
mappings above, are read from the source, not run.

Tests: back and forward step in Cull (and in Cull's Developed), taken with
their release, Shift adding, a doubled press stepping once, other buttons
passed; Develop, the grid, the lightbox, and Cull's grid step nothing and
pass the press on, which over a grid cell still goes to the folder visited;
a sheet, a menu, a text box, and a typed field keep the frame; the ends hold
as the arrows do, through the browser's own step.

**Left as they are (review).** Focus lost clears the taken press as
well as the held one, so a release after Alt-Tab reaches Slint with no
press before it; nothing listens to a side button's release. Ctrl,
Alt, or Super with a side button still steps, where the arrows with
them resolve to no chord. A slider holding the keys in the Cull of a
developed frame takes the arrows and not the buttons. A menu opened
while a text box had focus does not set `menu-up`, as with the grid's
side buttons.
