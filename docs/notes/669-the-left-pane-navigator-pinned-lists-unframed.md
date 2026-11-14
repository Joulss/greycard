# §669. The left pane: the Navigator pinned, the lists unframed (2026-11-14)

From a contributor's pull request (138), finished at the merge.

We pin the Navigator above the scrolling lists, under the Library and
Loupe buttons, as Analysis sits above the right panel's tabs: it stays
in sight however far the lists scroll. Its box is the pane's width at
3:2, but never taller than a 3:2 box 260 px wide. A panorama at a wide
pane takes the whole width, a portrait stops growing, and the room
past the cap goes to the lists.

The lists lose their inner frames: no border, fill, or padding. Rows
in all four lists are 20 px, 1 px apart.

One gutter: `Theme.space-1` (4 px) down the left of everything in the
pane, and down the right of what is pinned (the view buttons, the
Navigator, and the foot). The scrolled lists pad right by
`max(Theme.space-1, ScrollBars.band + 2px)`, clear of their scroll
bar, so their right edge sits 12 px inside the pinned parts' with a
thin bar; that step is the cost. The right panel's redesign (pull
request 136) pads its scrolled content by the same rule. The narrow
gutter gives a deep folder tree its room and, at the default width,
shows the French Bibliothèque, Loupe, and Importer whole.

The pane's scroll bar stays on its right, as in every scroll pane. The
band's outer 4 px lie under the pane's resize edge, and the thumb sits
4 px in from the outer edge, against the resize edge, so the edge and
the thumb each keep their presses.

Open folder, and the folder's name with its recent menu, stay outside
FOLDERS, so folding it never hides the way to a folder. The foot
starts with a full-width rule, as the right panel's does.

ScrollPane's horizontal band gets `x: 0px`. Slint centers an element
given no x, so whenever both bars showed, the band sat half a band to
the right. This touches every scroll pane.

At the merge we changed:
- The scrolled sections keep `Theme.space-2` between them. The share
  edges sit in that gap; without it they covered the last 8 px of the
  section above, and took the presses on Take, the last folder row,
  and Sync….
- The bin test in `assets.rs` is master's again: the submitted one
  hard-coded pixel columns.
- The Navigator caps its height, not its width (above).
- One gutter (above), where the submitted pane had three.
- One rule above the Navigator, not two.
- Click targets as wide as before: snapshot icons 24 px, the preset
  star and bin 22 px, each as tall as its row.
