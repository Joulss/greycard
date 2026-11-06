# §574. The grain's line drawn past the texture

*2026-11-05*

On a 45 MP frame, one Best fill over lettering, a window of 1176x928,
came back with grain 4.5 times its ring's high-pass energy, where the
same frame's small cuts read 0.8 to 1.3. The log said why: the
surround's luma sigma at the hole's level, 0.04, read 0.0179; the
small cuts on the frame read 0.0014 to 0.0030.

The bins were not wrong. Each level bin's quietest quarter of blocks
read the grain: 0.0010 at 0.006 rising to 0.0046 at 0.13, a clean shot
noise line, from 80 to 735 blocks a bin. Above that four bins held
nothing but the lettering, 17, 17, 11, and 8 blocks, reading 0.014 to
0.125: 10 to 1,000 times the grain in variance. The line through the
bins was weighted by each bin's blocks alone, so those four, a few
dozen blocks, tilted it until it read texture at every level, the
hole's included. A small window rarely holds a level bin all
texture; a big one over a sign does.

A variance read from blocks scatters in proportion to itself, so the
line is now weighted by each bin's blocks over its variance squared,
and fitted again a few times by the blocks over the line's own value
squared, until the bins kept stop changing. Each time it leaves out
the bins past 3 times the line, since texture only adds variance, and
those under a sixteenth of it, crushed or clipped shadows, which
relative error would otherwise let pull the line's foot down: on a
synthetic window a third crushed to a twentieth of its grain, the
shadows above them read 0.73 of their grain without that. The first
weights hold a bin far under the rest (a smooth stretch amid grain) to
a sixteenth of the bins' median, so it cannot pin the start. This is
the picture estimate's second fit only. Its first fit, which sets the
floor for made-up highlights, stays weighted by blocks: weighted by
relative error, a few smooth bins near the peak hold it down to their
own, and the floor lets them through. The mosaic estimate the develop's
denoise reads is unchanged.

The quiet blocks per bin were already there and right, a spread
robust within a block cannot help where no block holds grain alone,
and the parked mosaic cap needs the demosaic, the matrix, and the
learned denoiser modeled before it binds; the line needed none of it.

The grain is kept in the fill, so the fill cache's version goes to 4
and fills kept on disk are made again in the same slots.

### Before and after

The spread of a 3x3 high-pass in the hole (within 0.8 of the radius)
over that in a ring round it (1.3 to 2.5 of the radius), luma and
chroma. The remove's patches; LaMa's at their own pixels have no
grain put back and do not move. The overcast sky is §434's harness
(ISO 500, −1 EV, learned Best, ring kept to the sky), the per-block
median at each distance.

| case | luma before | luma after | chroma before | chroma after |
|---|---|---|---|---|
| big Best fill, 1176x928 over lettering | 4.47 | 0.93 | 3.45 | 1.18 |
| first frame, 5 small removes, median [range] | 0.99 [0.92, 1.12] | 1.00 [0.93, 1.13] | 0.97 [0.76, 1.06] | 0.96 [0.77, 1.05] |
| second frame, 34 small removes, median [range] | 0.98 [0.62, 1.34] | 0.98 [0.62, 1.32] | 1.00 [0.84, 1.29] | 1.00 [0.84, 1.29] |

| overcast sky, Best | d=1 | d=2 | d=4 | d=8 |
|---|---|---|---|---|
| before | 1.39 | 1.40 | 1.31 | 1.30 |
| after | 0.98 | 1.20 | 1.12 | 1.14 |

The surround's luma sigma at the hole's level, from the log: the big
fill 0.0179 to 0.0023; the second frame's 34 fills moved by a median
of nothing, the most by 0.74 times; the first frame's 7 regrained
fills, its 5 removes and 2 LaMa fills taken down, by under 7
percent. The overcast sky moved too: its window holds sea and rock
at the sky's level, §434's open item, and the excess at two fell from 1.40
to 1.20. The second frame's before is its export at triage, on the
same estimator; a fresh one ran out of the card's memory beside an open
editor. The exports were made before the shadows were left out and the
loop stopped early; the line as it stands, run over the same windows,
reads the same sigma at each hole's level to within 4 percent.

### Not done

- The overcast sky's remaining 1.2: the bins there are not all
  texture, so none is left out, and the quiet quarter of a mixed bin
  still reads high on a denoised frame.
- A bin of fine, even texture at the grain's own scale, read within
  3 times the line, is still read as grain.
- The mosaic estimate, which the develop's denoise and sharpening
  read, keeps its block-weighted line; whether it has the same fault
  on a frame of lettering is unmeasured.
