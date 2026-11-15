# §687. A set that shows in the strip, and a menu known to be up (2026-11-15)

## A. The set, visible in the strip

The strip's ring is 2 px round a picture that fills the cell, so the set's
mark (1 px `accent-muted`, 25%, over the 15% `selection` ground) was close to
nothing; Shift+arrow looked like a plain step. The grid gets away with the
same tokens because its plate shows `space-1` round a smaller picture and
behind the name.

Now (strip.slint): every frame in the set has a full 2 px ring, the set's in
`selection-ring` (#6ea8ff99, the accent at 60%; a new token, one value for
every theme, as `accent-muted` is), the current one's in `accent-text`. On
Charcoal and Grey `accent-text` is the accent itself, so the current frame is
as before; on Light it is the darker blue (#174a96), the precedent being the
lightbox's `accent-dark` on light grounds: on Light's #d4d4d4 surface the
pale accent and a 60% accent read alike, and the current must stay the
strongest. Ground unchanged. No size or layout change: the ring already took
2 px; the set's border went from 1 to 2 px inside it.

Tried and rejected (after-a): 1 px full accent over an `accent-muted`
ground. The set shows, but the current is only a pixel thicker, and on Light
the two are one mark.

The grid's current frame takes `accent-text` too (owner's call), so the two
agree on Light; Charcoal and Grey captures are byte-identical to before. Its
set marks are as they were: its plate carries the set. The lightbox (the
same cell in grid.slint) keeps the accent on its dark and grey grounds and
`accent-dark` on white: those grounds are the viewport's surround, not the
theme's, so `accent-text` (dark blue on Light) would be wrong over a black
lightbox. The compare view's rule round the current frame (viewport.slint)
is the accent over the picture's canvas, likewise not the theme's ground;
left as it is. Nothing else marks a current frame with the accent.

Captures: target/captures-2026-11-15/set-mark/ (before, after-a, after-b;
three themes, x1 and x2, current over dark and over bright).

## B. `menu-up` whenever a menu is up

Root: `menu-up` was set only in the keys' FocusScope's focus-lost with
`FocusReason.popup-activation`. Slint takes the focus from whatever has it
as a popup opens (`WindowInner::show_popup`) and gives it back to that on
close; when a text box, a slider, or a combo box had it, the keys heard
nothing and `menu-up` stayed false.

Fix: `menu-up` is now `in-out`, and the window's winit event filter
(`panel::side_buttons`, installed at startup) sets it from Slint's own popup
list (`WindowInner::active_popups`, tooltips excluded) before each window
event is dispatched. Every consumer reads it while an event is dispatched,
so each sees it as it stands; after a close that does not give the keys the
focus back it lags only until the next event, and nothing reads it in
between. The keys-focus path stays: it is instant, and it is what the
testing backend (which has no winit filter) sees. Uses
`slint::private_unstable_api` (on the pinned fork); a Slint bump must keep
`active_popups` and `window_kind`.

Popups in the window are Slint's child-window popups on winit (the winit
adapter has no `create_popup`), so their events reach the main window's
filter. Native menus (muda on macOS and Windows) are not in the list and
take no focus; `show_context_menu_for_hwnd` and `_for_nsview` run the menu's
own modal loop, so no side-button press reaches the window while one is up
(read from the source, not run).

Consumers of `menu-up` / `menu-shown`, all "a press that closes a menu is
nothing more" or "no tips while a menu is up", each now also right for a
menu opened over a focused box:
- app.slint: `side-button` (Cull's side buttons); Alt+Left/Right
  visit-folder (keys only; unaffected); the viewport's `focus-keys` (not
  while a menu is up) and its press; the `menu-refocus` timer (unchanged:
  it refocuses the keys when they lack the focus anyway).
- strip.slint: `quiet` tips, middle-click, `closing`, its scroll band.
- grid.slint: `side-press` (folder visit), `quiet` tips (three), the tile's
  and the cell's `closing`, the cell's middle-click, its scroll band.
- controls.slint `ScrollBand` (`took`), recent.slint, roots.slint (row,
  AddLink, All roots), collections.slint (row, chip), presets.slint,
  mask.slint, and viewport.slint.
All existing UI tests pass.

Where a menu really opens over a focused box: the frame menu from the
strip, the grid, and the viewport, and the help button's click, give the
keys the focus first; so does a tile's right-click, in practice (the test
sees `menu-up` true without the filter). What does not: menus opened by a
key or a property (help, zoom, sort, parts, landscape, `--menu`), combo
boxes, and any menu opened while a slider or a combo box has the focus.

Tests: in Cull, the frame's title focused and the help menu opened, the side
button refuses and `menu-up` comes from the popups (fails without the fix);
Escape gives the box back, which still refuses as a box does; let go, the
step. In the grid (tree.rs), the filter's words focused and a tile's menu
up, a back press over another tile only closes it and the next visits. That
one passes without the fix too, since the right-click hands the keys the
focus first; no in-window path was found where a grid side press reaches a
cell under a menu opened over a box (the frame menu's outside press reaches
nothing).

Two exactness fixes in side_buttons.rs: focus lost clears `held` only, so a
release after Alt-Tab still goes with its taken press (test); `side-button`
refuses while `SliderKeys.holder != 0`, as the viewport's `typing` does
(test). Ctrl, Alt, and Super unchanged. Not added: `snapshot-editing >= 0`,
which the viewport's `typing` also checks; worth a look.

Unverified: no real mouse pressed, no native menu run on macOS or Windows;
the captures are Slint's software renderer, not femtovg on a GPU.

## Left as it is (review)

On Light the set's ring comes to 1.55:1 against the strip's ground
and 2.0:1 over a bright picture, under the 3:1 a mark wants; a
per-theme `selection-ring` (the dark accent at 60%) would give 2.7:1.
The owner looked and kept it. The popup list is read through
`slint::private_unstable_api`, which upstream 1.18 has too and any
bump may move.
