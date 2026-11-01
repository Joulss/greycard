# §517. The camera match holds unseen greys, and the library refuses a narrow fit

*2026-11-01*

A look fitted from one folder (one body and style, one indoor shoot
under warm light, white balance fixed at 5200 K) turned greys magenta
from mid-grey up, so white clouds on a daylight frame went pink. Its
fitted ΔE was 0.015: the error was off the data. No bright grey was in
the shoot.

**Where the tint came from.** Not where we first looked. We expected
the table to carry the orange cluster's residual onto the grey axis.
Taking the model apart on the user's sets says the tint starts in the
per-channel curves. Each is a binned median of the camera's channel
against the matrix's, and on one light's colors a bin at high green
holds only the hues that reach it, so the curves bend greys by up to
0.06 to 0.10 (channel spread, encoded) on their own. On wide sets the
table pulls those greys back, because the data has greys. On one
shoot it has none, so the bend stays, and the smoothness term, which
costs nothing for a straight line, adds the data's slope.

**Part 1: the hold.** So the fix holds greys, not the matrix and
curves. Each lattice node has a support, its votes in the data term
(one a node on average). A node near the grey axis (input chroma under
0.02 in Oklab, fading to none at 0.04) with under one vote is pulled,
with weight up to 4 on top of the flat 0.02, toward the input's own
chroma plus the cast the data's greys show. Lightness is not held,
since the matrix and curves' own lightness can fall back on itself
between the greys a set does have; the data and the smoothness keep
it. `LutParams::hold`, `supported`, `neutral`.

**The cast.** A camera may render its greys with a cast of its own,
a warm one say, and the table should keep it. So the hold's target is
not neutral but the cast the data shows: over the pairs whose input is
near grey, the camera's a and b less the input's, fitted per unit of
the input's lightness (a channel gain is a cast in proportion to
lightness in Oklab), in eight lightness bins with the whole set's where
a bin has under eight pairs' weight, and none where the set has under
eight, which is the one-shoot case. The fit has a second term, the
input's own chroma, because a camera that scales chroma over a set
whose near-greys lean one way reads as a cast without it: with a plain
mean, the GFX Provia set's greys went from a spread of 0.010 to 0.044
under the hold, toward a blue the supported greys did not show; with
the term, 0.009.

The brief's first shape, every unsupported node pulled to the matrix
and curves, did worse: it let the curves' bend through (R5 Mark II
Faithful's grey spread went from 0.028 to 0.065) and raised the
coupling of §260 by half to three times (R6 Mark III Faithful under
AgX 0.057 to 0.182). Holding lightness near the axis put falls of
0.01 to 0.02 into R6 Faithful's grey ramp. Holding to neutral pulled a
camera's honest cast off its greys.

**Part 2: a check after the fit.** `greycard_match::grey` runs the
finished model over the 33 greys and measures how much the cast
changes along the axis, not how far it is from neutral: over the
steps between black and white (white is pinned, black has no chroma),
the one cast in proportion to lightness that fits the steps best is
taken off, and the drift is the larger of the ranges of a and of b
left. A channel gain scores nothing (0.0000 for the x1.06/x0.92
camera of the tests), a tint that switches on at mid-grey a great
deal (0.065 for a ramp shaped like the bad table). The largest fall in
Oklab lightness from one step to the next is checked as before. A
table that drifts by more than 0.015 or falls by more than 0.005 is
not written: the group's line says "refused: the fit would tint greys
it never saw", and the log gives the cast, the drift and the fall.

The threshold sits between the two sets of tables we have:

- the user's 26 tables, all from sets across folders: drift 0.0016 to
  0.0136 (the highest two are fit-1 R5 Mark II Faithful tables; the
  fit-2 one is 0.0103);
- the one-shoot set refitted without the hold: 0.0176 per channel,
  0.0258 under AgX.

With the hold every refit below drifts 0.0058 at most, and the
one-shoot set 0.0036 and 0.0071. A donor read only for a borrow is
held to the same check, so a borrow never takes a drifting table.
`--match-compare --match-write` takes the same rule.

**Part 3: in the library, one folder gets no table.** In the library
scope a group whose sample is all from one folder is now skipped
before anything is developed, in the old note's words: "all from one
folder, so the fit would be narrow". It is the sample that is counted,
so a second folder of adaptive frames the sample leaves out does not
rescue it, and one frame from another folder among forty does (the
sampler takes from every folder first). A frame with a copy in another
folder, the same file name taken at the same time, counts in the first
folder that has it: the user's library holds each recent shoot twice,
on the desktop and on the archive, and would otherwise pass as two
folders.

The folder scope keeps fitting. There the user chose one folder on
purpose, and a user with no library has no other scope; refusing
there would leave them no table at all. The hold and the grey check
cover the narrow case, and the plan and result lines keep their note
("all from one folder, so the fit will be narrow"). `--match-compare`
plans as a folder's run does, so the harness still measures a
one-shoot set.

**Measured.** The bar is `--match-compare`'s records of the user's
sets (334 frames over 12 groups, §260's), refitted under both curves
and every frame held out, before (no hold) and after. Held out over
the 18 rows: 0.01457 either way, no row moving by more than 0.0002.
Coupling, mean over the rows: 0.081 either way. Drift: 0.0018 to
0.0125 before, 0.0015 to 0.0058 after. No fall in either.

| group | curve | frames | held out before / after | drift before / after |
|---|---|---|---|---|
| R5 Faithful | pc | 36 | 0.0150 / 0.0150 | 0.0024 / 0.0026 |
| R5 Faithful | AgX | 36 | 0.0183 / 0.0184 | 0.0043 / 0.0042 |
| R5 II Faithful | pc | 22 | 0.0139 / 0.0138 | 0.0125 / 0.0044 |
| R5 II Faithful | AgX | 22 | 0.0163 / 0.0161 | 0.0103 / 0.0051 |
| R5 II Standard | pc | 35 | 0.0136 / 0.0137 | 0.0019 / 0.0022 |
| R5 II Standard | AgX | 35 | 0.0194 / 0.0194 | 0.0033 / 0.0033 |
| R5 Standard | pc | 28 | 0.0081 / 0.0081 | 0.0041 / 0.0041 |
| R5 Standard | AgX | 29 | 0.0126 / 0.0127 | 0.0040 / 0.0039 |
| R6 Faithful | pc | 40 | 0.0104 / 0.0104 | 0.0091 / 0.0058 |
| R6 Faithful | AgX | 40 | 0.0127 / 0.0127 | 0.0110 / 0.0049 |
| R6 II Faithful | pc | 39 | 0.0124 / 0.0124 | 0.0019 / 0.0015 |
| R6 II Faithful | AgX | 38 | 0.0159 / 0.0159 | 0.0025 / 0.0029 |
| R6 III Faithful | pc | 40 | 0.0108 / 0.0108 | 0.0018 / 0.0017 |
| R6 III Faithful | AgX | 40 | 0.0123 / 0.0123 | 0.0022 / 0.0035 |
| GFX Provia | pc | 24 | 0.0216 / 0.0216 | 0.0049 / 0.0043 |
| GFX Provia | AgX | 25 | 0.0224 / 0.0224 | 0.0053 / 0.0055 |
| GFX Reala Ace | pc | 38 | 0.0122 / 0.0122 | 0.0047 / 0.0046 |
| GFX Reala Ace | AgX | 38 | 0.0143 / 0.0144 | 0.0063 / 0.0052 |

The one-shoot set, 40 frames of one body in Standard from one day at
5200 K, developed fresh by the match: drift 0.0176 per channel and
0.0258 under AgX before, 0.0036 and 0.0071 after; grey spread 0.058
and 0.074 before, 0.021 and 0.016 after (the cast its few near-grey
pairs show); held out 0.0130 and 0.0148 either way. Less than the 0.20
spread the user's table had, which we did not reproduce on any
one-folder set of theirs we ran (a second, 23 frames in Faithful,
drifts 0.0047 to 0.0056 either way, its greys in the data). The
mechanism is reproduced by the test below.

**Not a fit bump.** `FIT_VERSION` stays 2. On sets from several
folders the hold moves no table measurably, and a bump would show
every fit-2 table the Look section's refit line, whose reason (color
noise into lightness) is not this.

**Tests.** `greys_no_pair_reaches_stay_neutral`: pairs only in an
orange cluster, the camera's green lower there; without the hold the
greys from mid-grey up spread 0.129, with it 0.001, at the matrix and
curves' lightness within 0.01, and the cluster fits as well. The grey
measure on identity, a contrast curve, a warm channel gain (no drift,
its cast read), a magenta tint switched on at mid-grey, and a folded
ramp. In the editor, on the tests' synthetic camera (a x1.06 red and
x0.92 blue gain, on its greys too): its fit passes the check, its cast
lands within 0.003 of the camera's, and every grey from 0.125 to 0.875
within 0.004 in a and b. Also one folder counted on the sample with a
backup once; a one-folder group refused unread in the library and
fitted with its note in its folder; and the check's wiring, a table
whose greys' green falls a fifth from dark to light refused with its
line and nothing written. The `refit` example prints the cast, the
drift and the fall.

**Left.** The sampler still takes a frame and its backup copy as two
frames, so a library with backups double-weights them; deduplicating
the sample would move every bar. The curves' own off-data behavior
(flat past the last bin, then pinned to white) is untouched.
