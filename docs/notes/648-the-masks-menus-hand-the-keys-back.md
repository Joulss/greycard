# §648. The masks' People and Landscape menus hand the keys back (2026-11-12)

§636 left the masks panel's People and Landscape menus without Fit's
wiring: no keys focus before `show`, no `menu-done` on a pick. A click
on People gave the button the focus and the menu opened over it, so
`menu-up` stayed false and the click that closed the menu was a click
on whatever was under it; after a pick the focus went back to the
button, so Space or Return pressed People again and reopened the menu.
Landscape's button kept the focus the same way through the model's
run, and Return after a pick ran the find again.

**Fix, as §636.** `AdjustmentsSection` gets `menu-opening`, called
before each of the two menus shows, from a click and from Rust's
`parts-menu-shown` and `landscape-menu-shown`; the window answers it
with `keys.focus()`. `add-part` and `add-landscape` call `menu-done`
after the pick. A pick that offers the People model leaves the keys
the focus too: the model sheet takes none and answers Escape through
the keys.

**The rest, checked against Fit.** Every other menu in the window
already takes the keys first and hands them back: the frame menu
(each surface focuses the keys before asking), Recently opened, a
root's, a collection's, a preset's, the masks list's, the tiles', the
grid header's archive and rejects menus, Sort, Import, Help, Fit, and
Auto's presets. Picks that open a sheet skip `menu-done` on purpose, as
noted beside them. Sort's picks and the archive header's have no
`menu-done`; both menus are flat, opened with the keys holding the
focus, and Slint gives it back to them on close.

**Tested.** `the_masks_menus_hand_the_keys_back` opens People from a
click and Landscape from Rust after a click on its button, checks
`menu-up`, picks from the keys, and puts each away with Escape; after
each, Space and Return press neither button and Right steps the frame.

**To try.** On a mask, click People, pick Face with the pointer, then
press Right: the next frame. Click Landscape, wait for its menu, pick,
then press Space: no new find.
