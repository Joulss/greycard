# §682. Wheel holds that follow the keys, and their guide (2026-11-14)

The grading wheels and the tint's ring took Shift (keep the hue) and
Ctrl (keep the strength, ⌘ on a Mac) at the press only. Now the hold
follows the key through the drag, and a guide shows what is kept.

## Decided

- A hold taken up mid-drag starts from what the wheel has at that
  moment, not at the press, so the point never jumps back. Let go, the
  point is the pointer's again at once. One key for the other (Shift up
  with Ctrl still down) starts the new hold from where the old one left
  the wheel. Shift wins when both are down, as before.
- The decisions are Rust's: `Hold::of(shift, control)`, and
  `WheelDrag` (press, `keys`, `point`) in `panel/color.rs`, which holds
  the wheel, the hold, the values the hold began from, and the last
  pointer place. Slint passes two bools; the `WheelHold` global is gone.
- One drag cell serves the three wheels and the ring; release (or a
  cancel, which `CurveEditor` turns into a release) clears it and the
  guide.

## How the key reaches the drag (Slint 1.18.1, our fork pin d8686b7)

- Moves: `TouchArea` calls `pointer-event` with `kind: move` and the
  window's modifiers just before `moved`
  (`internal/core/items/input_items.rs`, `MouseEvent::Moved`), so
  `CurveEditor` keeps `modifiers` fresh on every pointer event and the
  wheel's `moved` reads them. `moved` itself carries nothing.
- A key with the pointer still: a pointer grab does not take key
  events; they go to the focused item. A press in a wheel runs
  `Typing.close-all()`, which hands the focus back to the window's
  `keys` FocusScope, and its `key-pressed` and `key-released` now call
  `HeldKeys` first thing (see below). Slint updates the
  modifiers before the event is delivered
  (`WindowInner::process_key_input`), so a release already reads the
  key as up. Slint re-sends a move on a modifier change only for a
  drag-and-drop, not a grab, so this path is ours.
- Where the window's keys do not have the focus (a control outside
  the panel kept it), the next move carries the key instead; tested
  with the key path cut.

## A drag that outlives its wheel

A review found the shared drag cell outliving its wheel: the grading
and tint sections are conditional instances, so a view change mid-drag
(C to Cull, another tab, `grade-big`) destroyed the editor with no
release, and a later Shift or Ctrl took up the dead drag and wrote the
open frame without a history step. Fixed by construction:

- The window's keys no longer call Rust. They set a `HeldKeys` global
  (modifiers, a count); `CurveEditor` watches the count and passes a
  key on (`keys-changed`) only while its own press is held. A destroyed
  editor has no handler, so a dead drag hears nothing; the next press
  replaces the cell. The guide too is drawn only while the editor's own
  press is held.
- In Rust, a drag lives only while its wheel is in hand
  (`live_drag`). Everything that puts a wheel down already sets
  `wheel-hand` to −1: Esc, another section's control, and `show_edit`,
  which every frame change, target change, and undo runs. So these end
  the drag and its hold (guide cleared) at the next move or key, and
  the pointer moves nothing after.
- Esc ends the hold rather than leaving it applied with the guide
  hidden: Esc already puts the wheel down, and a drag of a wheel not in
  hand is that.

Tests: Cull, tab, and `grade-big` mid-drag (the reviewer's scenario),
a frame change, and Esc; each fails with the gate and the in-hand check
reverted (the original wiring).

## The guide

- Rust works out its geometry (`wheel_guide`) in the picture's own
  coordinates, 0 to 1 across and down, on `draw_hue_circle`'s center
  and radius (n − 2 pixels across, centered at (n − 1) / 2), for the
  size the wheel is drawn at (96, 320, or the ring's 256).
- Shift: two strokes, center to the marker's rim and marker's rim to
  the wheel's rim. Ctrl: one arc through the marker, round the long way
  (`large-arc`, `sweep: false`), its ends where the circle meets the
  marker's dark rim. Broken at the marker, so the marker reads as
  drawn over it without moving the marker out of the picture.
- Slint strokes it 1px in `Theme.over-plate-dense`, the dark ring for
  a light ground that is the same in every theme: the wheel's picture
  is that ground (Oklab L 0.72), so the ink need not follow the theme.
  `dark-muted` read at about 2:1 on it, too faint for a hairline.
- Drawn by `WheelGuideLines` over the picture at the picture's own
  bounds, under `HandRing`; no layout around it changes.

## Tested

- `shift_wins_over_ctrl`, `a_hold_begins_and_ends_where_the_drag_is`
  (pure), `the_guide_is_the_held_hues_line_or_the_held_amounts_circle`
  (geometry).
- `a_key_held_mid_drag_holds_the_wheel_from_there`: in the window, on
  the Shadows wheel and the ring: Shift down with the pointer still
  holds the value then and shows the line at once; up frees it at
  once; Ctrl's circle; Shift over Ctrl; release clears the guide with
  keys still down.
- `a_move_carries_the_keys_into_the_drag`: the same with the key path
  cut.
- `shift_holds_a_wheels_hue_and_ctrl_its_strength` unchanged and
  passing: the press-time behavior stands.
- Each new test fails with its piece reverted (the key wiring, the
  moves' keys, the re-anchoring, Shift's precedence, the release's
  clearing, the geometry).

## Captures

`capture_the_wheel_guides` (ignored; `GREYCARD_WHEEL_CAPTURES=DIR`)
draws the window with Slint's software renderer on the testing backend
(dev-dependency feature `renderer-software`), since a drag in the
middle cannot be driven from outside: weston takes no injected input.
They are the software renderer's, not femtovg's.

## Left

- Not checked in the real window under femtovg, nor on a Mac with ⌘.
- A focused control outside the window's keys (a sheet's) still waits
  for a move; the panel's own controls hand the keys back at the press.
