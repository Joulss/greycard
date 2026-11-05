# §566. A fill kept for the picture it was made from

*2026-11-04*

A tester's Best fill came back wrong after a white balance change,
right again after the file was reopened (issue 63). The fill cache on
disk (§425) keys a fill by a print of the picture under its window,
so a fill made under another white balance, lens, or noise setting is
another slot; the copy in memory was keyed by the patch and its
window alone. Between a base change and the next open, memory served
the old fill over the new base.

A fill in memory now carries the print's hash, made once per fill
asked and shared with the slot on disk, and is served only when the
picture under the window is the one it was made from. The cost is the
print itself, a mean over 16-pixel blocks of the window, on every fill
that memory answers; the print was already taken for the disk key on
every fill that memory did not. A kept point past the retouch is
asked whether a fill it left unmade could be made now, on its own
picture, which no fill was made from; there a fill in memory for the
patch and window counts as kept whatever it was made from, as before.

The status line says how far a develop with several fills ahead has
got: "the fill model is at work on Fill 5, 4 of 15". The count is
what the retouch had to make when it started; a fill that was kept
is not in it, and one whose window takes in an earlier patch's region
may be counted and then found kept, so the count can stop short of
its total. One fill ahead keeps the old line. Why the fills were
remade, and whether they need to be after a change the fill's
surround does not see, is the open question on the Generative line
(issue 71).
