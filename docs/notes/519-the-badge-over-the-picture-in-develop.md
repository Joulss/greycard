# §519. The badge over the picture in Develop

*2026-11-01*

In the develop view a rating, flag, or label key had no answer but the
strip tile's small badge, and none with the strip put away by Tab.
Culling shows the frame's badge in the picture's top right corner; we
show the same badge there in Develop, for two seconds after a key, then
fade it. A second key restarts the two seconds. While the strip is
hidden it stays up without a key.

The badge draws from the same `frame-rating`, `frame-flag`, and
`frame-label` as culling's, so it cannot disagree with the strip. Rust
raises `badge-flash` where a key lands (`browser::meta_on`), only when
the key reached the open frame, left it in the filtered list, and the
window shows none of Library, Cull, and the lightbox, each of which has
its own badge; a timer of its own (`badge_timer`, as the notice's)
takes it down. The window reads `develop-badge` from the flash, the
hidden strip, and the view; the badge shows at once and only its way
down fades (a state with an `out` transition on opacity). It takes no
pointer and leaves the soft proof pill at top left alone.

The flash answers one key on one frame, so it ends early whenever that
stops being true. Another frame opened (`open_row`: an arrow, a click,
or the selection moving on) stops the timer and drops it; without that,
P and an arrow carried a pick onto the next frame's badge. A key that
takes the open frame out of the filtered list does not flash at all:
under the Picks filter, X on one of two picks moves the selection to
the other, and the badge would have flashed that frame's pick as the
key's answer. Leaving the develop view drops the flash too (a
`changed` handler in the window), so coming back within the two
seconds does not show the rest of it.
