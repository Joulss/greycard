# §538. The FOLDERS fold kept per view

*2026-11-03*

Discussion 59: a tester keeps FOLDERS open in the Library to navigate
and wants it folded in the Loupe for room. The section was one Slint
`Section` with one fold, shown in both views.

We keep a fold for each view. The section still binds one property,
`collapsed-folders`, the fold on screen; `collapsed-folders-library`
and `collapsed-folders-loupe` hold the view off screen. `changed
grid-open` stores the outgoing view's fold and loads the incoming
one's. A handler on the fold itself, storing each fold into the view
on screen, was left out for a race at startup that is not real:
`show_folds` runs while `grid-open` is still false, and Slint runs
change handlers before the first paint.

What is real is the moment between a switch and its handler: the
start's settings baseline is taken there, after `set_grid_open` and
before the handler, and read the Library's fold from the Loupe's, so
the first timer tick rewrote the file. So `collapsed-folders-is-library`
says whose fold the section holds; it changes with the swap, not with
`grid-open`, and the getters read by it.

A fold the view brings lands at once, as the sections around it
appear; a click on the header still moves it. `Section` gains
`animated`, its fold's duration 120 ms or none: the switch sets it
false, the header's click true.

The settings name the two folds `folders-library` and
`folders-loupe`. The old `folders`, written by earlier builds and
never now, folds both; it joins GEOMETRY's three in one table of
sections that became several.

FOLDERS is not one of the panel's sections (`one-of-panel` is off for
the left pane), so One section open at a time never touched it and
holds in each view as before.
