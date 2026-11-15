# §673. WHITE BALANCE rests on a monochrome frame (2026-11-14)

§668 made every white the identity on a monochrome sensor's frame and
left the panel's Temperature and Tint live, doing nothing. We now rest
them: on a monochrome frame the Develop tab's WHITE BALANCE and a
mask's are disabled whole, in the look a section has when another
takes it over (the mixer under the black and white): the body dimmed,
each control in its disabled colors, a mask's switch shown off and
dead. The section still shows and folds.

- **Inert.** Temperature, Tint, As shot, Auto and its presets' menu,
  Neutral, a mask's switch, and Follow global take no press, drag,
  double-click, wheel, or key; the sliders' comma and period walk
  passes them over. W does nothing, as R does with no frame, and
  brings no tab.
- **A dead switch takes the click.** A boxed section's header row
  folds the section on a click, and a disabled `Switch` let its press
  through to that row: a click on the mask's dead switch folded WHITE
  BALANCE, and the mixer's dead switch under the black and white
  folded the mixer (§670's header row). A disabled switch now takes the press and
  does nothing. The header's switch is the only `Switch`, and nothing
  relied on a dead one letting a press through.
- **A disabled control that keeps the focus.** Slint's FocusScope
  hands a key to a focused scope whether it is enabled or not, and a
  control disabled while it holds the focus keeps it. Temperature
  focused on a color frame, Home to a monochrome one, and an arrow
  stepped the stored white from 4000 K to 4150 K. So a disabled
  Switch, Toggle, or PanelButton that holds the focus takes Space and
  Return and does nothing, and a disabled slider takes its step keys
  and makes no step. Its own keys stay its own: passed on, Space would
  zoom the picture, an arrow step to another frame, and Return on an
  export preset's Delete, disabled by its own click, would export.
  Every other key goes where it went before. A slider's field being
  typed in closes when the slider is disabled. This holds app-wide,
  not for the white alone.
- **And behind them.** Every callback that writes a white (As shot,
  the slider resets, Auto, the presets, a mask's switch, its reset,
  Follow global, the dropper taken up, and a press with it) asks one
  helper, `panel::color::white_refused`, first. On a monochrome frame
  it refuses and shows the edit's whites again, so a change that
  reached the panel by any route is taken back before a save reads it.
  These are a second line behind the disabled controls; the tests call
  each directly. A slider's step has no such line (the develop it asks
  reads the panel as any slider's does), so its own guard is the one
  that holds there. The White dropper in hand is put down as the frame
  is known monochrome, by its decode or as it is chosen (the window's
  change handler on `monochrome-open`).
- **The edit is not touched.** A white the frame arrives with, pasted,
  synced, or from a preset, stays in the sidecar; copy, paste, sync,
  and presets carry it through as §668 has them. Auto and the dropper
  write nothing on one; a preset or a sync onto one writes its white
  into the edit, which develops as none.
- **How the panel knows.** The worker's `Outcome::Opened` already
  carries the frame; its layout decides (`panel::color::
  learn_monochrome`), and we remember the file for the session
  (`Develop::monochromes`), so a monochrome frame chosen again rests
  the moment it is chosen. One not yet decoded is live until its
  decode lands: a color frame is the common case, and a move made in
  that moment develops as none, as any white on it does. A picture,
  and a color raw opened after, are live. The panel is the open
  frame's alone, so a multi-selection changes nothing.
- **Heard by a screen reader.** Before this, a `Toggle`, a
  `PanelButton`, and a section's `Switch` told a screen reader nothing
  of themselves: only a Toggle's or a button's words were there, as
  text. Now a Toggle is a switch named by its label (its label's text
  says nothing, so the name is heard once), on or off. A section's
  switch is a switch named by the section's title, on or off as it
  shows, so a dead one is heard off. A PanelButton is a button, named
  by its own text, or by its pointer words when it has a picture
  alone. Each says whether it takes a hand, which is where a dimmed
  section's controls are heard as disabled; the tests read it as they
  read a slider's.

No new words: the section says nothing of why; the dimmed controls
are the word.
