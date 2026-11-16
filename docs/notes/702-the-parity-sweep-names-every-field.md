# §702. The parity sweep names every field (2026-11-15)

The sweep that draws random edits and holds the viewport to the CPU
reference never varied three parameters added this cycle: a mask's
Opacity, the output sharpen, and the profile tone curve. Each had its
own test, but the sweep is what finds interactions. Reading its
generator found four more it never drew: a mask's Refine edge, a mask's
white balance as an offset, an older edit's kept zero, and a source's
baseline (a body's own, a merge's headroom, a DNG's `BaselineExposure`).

**What it draws now.** Five new parts (`opacity`, `refine edge`,
`output sharpen`, `profile curve`, `baseline`), and `local white` draws
offsets too. They are drawn after every earlier draw, so an old seed
keeps its earlier edit. The profile curve is one of two synthetic
ones: Adobe's shape with an offset, and a toe that reaches 1 at 0.85
and stays flat.

**So it cannot be missed again.** `case_fields` takes apart every
struct the finish reads with no `..`, as the cache key's equality does,
and matches `Shape` and `LocalWhite` exhaustively: a new field does not
compile until it is marked or named exempt with a reason.
`the_sweep_names_every_field` fails if a marked field stays at its
default across the default seed's 400 edits.

**The steep-pixel cap under an output sharpen.** The sharpen's blur
carries the CPU's spread at unlit pixels into lit ones nearby, and one
edit tripped the cap with the GPU 0.6 levels off. Under an output
sharpen a pixel counts as lit only if every pixel its blur reads is
lit. Without one the cap is as it was.

**What it found.** On an RTX 5070 Ti, the default seed at 2,000 edits
and seeds 1 to 20 at 400 each pass, the worst 1.56 levels against 2.5.
On lavapipe the default seed passes (worst 0.79), and seeds 1 to 20
give three edits over: seed 2 edit 84 (2.68), seed 17 edit 289 (3.80),
and seed 20 edit 30 (2.94), each 0.5 to 1.1 on the card and each the
same on three replays. All three draw the toe curve, which hands the
point curves, the color curves, and the shading exactly 0 and exactly
1, as no display curve does. Leaving the profile curve out brings the
first two to 0.50; the third goes to 0.50 without opacity and gets
worse without the curve. The cause is not found; we changed no
rendering. Replay with `GREYCARD_PARITY_SEED` and
`GREYCARD_PARITY_ONLY` under the lavapipe environment.
