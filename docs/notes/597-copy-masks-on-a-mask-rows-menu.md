# §597. Copy masks on a mask row's menu (2026-11-07)

A user who looks for a way to carry masks finds Copy mask on a mask
row and not the paste sheet, where Masks starts unchecked. We add
**Copy masks** after Copy mask: a Copy settings of the open frame whose
paste opens with every mask checked and every other section unchecked.
The paste is the existing one, so the sheet shows what travels and a
mask can be unchecked.

The sheet's checks are not held per copy; a paste computes them from
the last applied sync or paste. We mark the copy instead
(`Clipboard::masks_only`): its sheet opens on the masks alone, the
masks replacing or joining as the last sync or paste had them, Replace
when there was none. Applying such a paste records only its join for
the next sheet, not its sections, so a later Copy settings opens with
its usual checks rather than the masks alone.

Copy mask and Paste mask stay: one mask, its own clipboard, added. The
Sync sheet is unchanged; it reads the open frame, not the clipboard.
