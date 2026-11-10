# §624. Alt+arrows go back and forward (2026-11-10)

**Back and forward were the mouse's alone** (discussion 59). A hand
on the keys, or a mouse with no side buttons, had no way back to the
folder a subfolder was opened from. A browser and a file manager
answer Alt+← and Alt+→.

**We bind those two, as the side buttons' keys.** A pair in the key
table, `visit-back` and `visit-forward`, rebindable as the step and the
turn are, under Moving around. The handler calls `visit-folder`, the
side buttons' own callback, so the stacks, the frame left (§620), and
every guard of `visited::go` are the same path.

**Where the buttons work, and nowhere else.** In the grid and the
lightbox (`grid-open`), not under a sheet, not while a menu is up. Not
in the Loupe, where the buttons reach no `GridSheet`. A held key goes
once: a button does not repeat, and a repeat would race the look that
`go` drops presses under.

**A text field keeps them.** The window's keys sit beside the panel, so
a focused field (the filter's words, a name) has Alt+arrows first, and
on a Mac Option+arrow moves by word as ever. A focused slider passes
them on (`SliderKeys.passed`), as does a clicked button (§621), so they
work after a click on the panel. No Alt+arrow was bound before.

**Not Cmd+[ and Cmd+] on a Mac.** Safari's and Finder's back, but
Ctrl+[ and Ctrl+] (Cmd on a Mac) are the brush's opacity. Left out.

Tests: Alt+← and Alt+→ in the grid go back and forward over real
folders; they ask once each in the grid and the lightbox, once for a
held key, and not in the Loupe, under a sheet, or in the filter's
words, the plain arrow still stepping; after a click on Flip, G and
Alt+← go back; rebound to Q, Q goes back and Alt+← is nobody's.

**Not proven.** A real Windows or macOS window: Windows' own use of
Alt for a menu bar, Option+← in a Mac's text field, and a window
manager that takes Alt+arrows itself.
