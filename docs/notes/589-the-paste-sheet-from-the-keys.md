# §589. The paste sheet from the keys

*2026-11-07*

A tester found (issue 78) that the Paste settings sheet could not be worked from the
keyboard. Nothing in it had the keys as it opened, and Tab, which the
sheet left to Slint, walked the whole window's tree: past the sheet's
buttons into the panel behind the scrim, so only Check all and
Uncheck all ever showed a ring.

We now give the keys to Paste (Apply, for a sync) as the sheet opens,
so Enter takes the checks as they stand: the common case, the same set
pasted onto frame after frame. Tab still moves by Slint's own focus
chain, which inside the card is already the visual order (Check all,
Uncheck all, each check, Cancel, Paste); the card's FocusScope takes
Tab in its capture phase only at the two ends, from Paste round to
Check all and from Check all back to Paste, so the keys never leave
the card. The card itself is no Tab stop. Slint scrolls a check
reached by Tab into the list's view itself, so a short window needs
nothing of ours. The Replace or Add choice under Masks and the Set or
Shift choices are no stops: Segmented takes no keys anywhere yet.

Space toggles a check and Enter fires the default button, the desktop
convention. A check used to take Return as well, so after one click on
a check Enter turned it over rather than pasting; a check now takes
Space alone, and Return goes up to the card, which pastes. The buttons
keep their own Return (Cancel, Check all, and Uncheck all). The preset
sheet shares the list: Enter on a check there saves, as Enter in its
name field already does, once there is a name; with none it does
nothing.

The frame menu's Paste opens the same sheet, and the menu's hand-back
of the keys to the window, a millisecond later, took them from Paste.
It now skips the hand-back when the sheet opened, as the menu's Sync,
Time shift, and Export already do.

Two consequences outside the sheet. A primary button's border is its
fill, so its focus ring could not show; a focused primary PanelButton
now draws the usual 2px focus-ring color as an outline just outside
it, for the keys' focus only. A click sets a flag the focus's loss
clears, so a clicked button shows no ring, and one reached by Tab,
even straight after a click elsewhere or on itself and away, does. A
loss to another window or to a popup leaves the flag, so a clicked
button does not ring on coming back.
The sheet closing removes the button that had the keys, so the
window takes them back on `sync-open` going false, as every other
sheet's close already does; without it the window's keys were dead
until a click.

PanelButton gained `forward-focus` and an out `focused`, and CarryList
a `focus-first()` and `first-focused`, so a sheet can name its ends.
Six UI tests in `panel/sync.rs` hold it (the sixth, the preset
sheet's Enter on a check, saves once named and never without): the sheet opens on Paste and
Enter pastes and hands the keys back; Tab and Shift+Tab go round,
every check a stop, Space toggling, Enter on a check pasting, Cancel
reached both ways; the menu's Paste survives the hand-back; a check
below the list's view comes into it; the ring follows the keys and
not a click.
