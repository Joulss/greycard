# §636. White balance presets (2026-11-11)

Issue 128 asked for the presets cameras and other editors have.
Six on Auto's menu: Daylight, Cloudy, Shade, Tungsten, Fluorescent,
and Flash. Each sets Temperature and Tint and turns As shot off, one
step, as Auto does.

## Decided

**Values.** The global sliders are kelvin and Duv (rawcolor's
`TempTint`: CCT plus the signed CIE 1960 uv distance from the
Planckian locus, positive green), the scale As shot reads a camera's
gains into through the profile. A preset is a fixed light in that
scale, the same on every camera:

| Preset | K | Duv | Source |
|---|---|---|---|
| Daylight | 5500 | +0.003 | CIE D55 (5503 K, Duv +0.0033) |
| Cloudy | 6500 | +0.003 | CIE D65 (6504 K, Duv +0.0032) |
| Shade | 7500 | +0.003 | CIE D75 (7504 K, Duv +0.0032) |
| Tungsten | 2850 | 0 | CIE illuminant A (2856 K, on the locus) |
| Fluorescent | 3800 | +0.007 | Adobe's preset (3800 K, tint +21) |
| Flash | 5500 | 0 | Adobe's preset (5500 K, tint 0); xenon tubes run 5500 to 6000 K |

The D-series Duv we computed from CIE 15's daylight locus against the
Kim et al. Planckian locus rawcolor uses, and rounded to the Tint
slider's step of 0.001. Adobe's tint is uv times 3000 (the DNG SDK's
tint scale), so its +10 on the daylights is our +0.003, the D-series'
own offset: the two sources agree. For Fluorescent we took Adobe's
+0.007 over the CIE F-series' chromaticities (F2 4230 K at +0.002,
F11 4000 K at 0): a tube's mercury lines read greener to a sensor
than its chromaticity says, which is the cast the preset is for.

**Placement.** Auto becomes a split button, as the footer's Fit is:
`has-menu` puts a chevron at its right, which a click (or a
right-click anywhere on it) opens the presets from; a click on its
words is Auto as before. No row added, nothing taken from the
sliders' width; Auto grows by the chevron in a row aligned to the
end. The menu is the window's `ContextMenuArea` popup, as Fit's and
People's are. The callback carries an English key
(`panel::color::WHITE_PRESETS`), as the People menu's does, so the
translated words never reach the Rust.

**Raws only.** A JPEG, PNG, or TIFF's sliders are a mired and Duv
offset from its own white, and that white is already the camera's
choice: a kelvin preset has nothing to be read against. Auto there
has no chevron, and the callback refuses a picture. A mask's white
balance (a light or a mired offset from the global's) has no Auto
and so no menu.

**No shown state.** A preset only sets the two sliders; the section
does not say which is in force, and the menu checks none.

**Focus, as Fit's.** The window's keys take the focus before the menu
opens (`menu-opening`), so `menu-up` is set and the click that closes
the menu, on the strip or on the picture with the dropper in hand, is
no click; a pick calls `menu-done`, so Return and Space after it are
the window's, not Auto's. Opened from a click on the chevron, Auto had
the focus first and kept it: Return after a pick ran Auto over the
preset.

`--sheet wb-presets` opens the menu for a snapshot.

## Limits

- Fixed values, not the camera's own presets: a maker note's
  per-preset white (Canon's and Nikon's carry them) would match the
  camera's own menu better and is left for later.
- The same values for every camera means a given preset can look a
  little different between bodies, as it does in Lightroom.
- No preset on a picture, by design above.
- The mask panel's People and Landscape menus have the
  same gap (no keys focus before `show`, no `menu-done` on a pick)
  and want the same wiring.
