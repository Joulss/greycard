# §693. The panel keeps to its sections (2026-11-15)

Issue 157: a tester's Masks panel showed empty room under its last
section, with MASK and WHITE BALANCE out of sight above.

## Cause

Slint's Flickable keeps its scroll in bounds with a change tracker that
puts content-x/content-y back when the content or the view changes size,
but only while the property has no binding
(`internal/core/items/flickable.rs:103-121`, `!y.has_binding()`).
`ScrollPane`'s own alias (`content-y <=> flick.content-y`, controls.slint:2230
on v0.11.0) is merged away by the compiler and makes no binding. The right
pane's `content-y <=> root.panel-scroll` (app.slint:3707 on v0.11.0, :4130 on
master) does: the pane sits inside an `if`, a sub-component of its own, and
the two-way link across that boundary is a binding on the Flickable's
content-y. So the tracker never acts on the right pane. When its sections get
shorter under a scroll near their foot (a section folded, a mask deleted,
Global chosen from a mask) or its view gets taller, nothing pulls the offset
back: empty room under the last section, the top sections out of reach
above. Every other ScrollPane was already kept in bounds by Slint.

The reset to the top on a tab change (app.slint:1199 on v0.11.0, :1346 on
master) was an earlier workaround for one case of the same thing.

Master has the same bug as v0.11.0.

## Reproduced

In UI tests on master: scrolled to the foot at 1683x1000 and LIGHT folded,
the panel stayed at -3672 px against a reach of -3392. With an object mask
made, picked, and put down, the panel stayed in range at each step; folding
the sections afterwards left it at -2105 px against a reach of 13 px, a view
with nothing in it.

On the unfixed build the wheel brings it back: its first notch jumps to the
foot (Flickable clamps its own wheel step), and the next ones scroll up.

## Fix

`ScrollPane` watches whether its offset is outside [-(content − view), 0] on
either axis and clamps it when it goes out (controls.slint, after `max-y`).
It lives in ScrollPane rather than at the one binding so any owner that binds
the offset from outside gets it; where nothing binds it, Slint's tracker and
ours do the same thing and whichever runs first leaves the other nothing.

The right pane is inside `if !root.right-hidden`, so Tab or F8 builds it
anew while `panel-scroll` lives on in the window. Built past its reach (the
sections changed while it was hidden, as on another photo with fewer masks),
it starts with `past-y` already true, and Slint hears no change for a first
value. ScrollPane looks once more on a one-shot 1 ms Timer after it is
built, when the first layout has given the content its height; a look at
`init` came too early and took a good scroll back to the top. The left
pane's ScrollPane binds no offset and starts at its top when rebuilt; no
other ScrollPane binds its offset.

The pane on its way out must not be kept in reach. When `right-hidden`
turns true, the old pane lives until its `if` is torn down, which an event
or a frame does; set from a timer (the lightbox, F8), timers and change
handlers run first. Meanwhile the pane is squeezed to its 50 px least
width, its content rewraps taller or shorter (4169 px for 4225), and a pull
then moved the scroll the next pane is built with by 45 to 56 px. ScrollPane
takes `keep-in-reach`, the right pane passes `!root.right-hidden`, and
neither the change handlers nor the look act while it is false.

The clamp moves `panel-scroll` in the same frames as the panel's two reveals,
which took any move since their last write for the wheel's and let go: One
section open at a time, scrolled to the foot, a header clicked below an open
section, left the opened section above the view. Both reveals now compare
the scroll and their remembered value each clamped to the sections' current
reach (`in-reach` in the panel's ScrollPane, app.slint), so the clamp's pull
is not the wheel's, and a real wheel move still is.

Not sharing the pattern: the strip is a bare Flickable with no binding on its
offset. The grid's scroll is our own property and is clamped where it
changes (app.slint `changed thumbs`, `grid-scroll-to`, the wheel).

## Unverified

- The exact step in the tester's sequence that shortened the sections; the
  object mask's own steps did not do it in the tests. Any shrink does.
- On macOS with a trackpad: the momentum phase of Flickable's wheel handling
  was not exercised.

## The slider reveal's change

It matters when the panel sits short of its foot by
less than what folds above a focused slider near the view's top: the clamp
takes back only part of the shrink, the slider ends above the view, and the
old comparison let go of it (its keys dropped). At the foot itself the clamp
takes back the whole shrink and the slider does not move.

## What a user may notice

- A view that grows pulls a scroll at the foot in, and nothing puts it
  back: hide the filmstrip at the foot and show it again, and the panel
  rests a strip's height short of its foot. Before, the scroll came back
  exactly, with empty room showing while the strip was away.
- A pane brought back after its sections got shorter while it was hidden
  draws one frame past its reach before the look pulls it in.
