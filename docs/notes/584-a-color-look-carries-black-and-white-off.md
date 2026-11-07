# §584. A color look carries Black and white, off (2026-11-06)

Clicking through the three shipped presets, Red-Filter Mono turned the
picture mono and Muted Slide and Warm Negative left it so, recoloring
a grey picture. A preset carries only the sections it lists (§78), and
the two color looks listed none for Black and white: nothing in them
differed from the bare edit there, and the shipped presets' test held
that a preset lists only what it changes.

That rule has one exception now. A color look lists Black and white
off on purpose: applied over a mono picture it brings the color back,
which is what a hand reaching for a color look means. The two files
list it, and the test allows an unchanged Black and white to be
listed.

**Bringing up a seeded copy.** The shipped presets are seeded once
into a user's store and never written again, so one edited or binned
stays so. A copy still byte for byte as an earlier build wrote it was
never touched, and the seed now writes this build's text over such a
copy; the earlier texts are kept under `presets/superseded/` for the
comparison. A copy changed by hand, moved to a group, or binned is
left as it is.
