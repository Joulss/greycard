# §554. The sharpen's threshold to 200

*2026-11-05*

The manual Threshold stopped at 50%, and a tester there still saw
flat areas sharpened (pixls, 2026-10-25). It now runs 0 to 200%,
RawTherapee's range.

**Same units, so nothing moves.** Ours was always RawTherapee's: the
panel's percentage over a hundred, against a contrast taken in L* over
sixteen, which is its `0.0625 / 327.68` on L scaled to 32768. A test
pins our blend to its formula at 0, 50, and 200 percent; the sidecar
holds the fraction, so a file at 50% reads and develops as before. The
default stays automatic, and 10% when fixed.

**On an older build.** The edit never clamped the field, so 0.9.x
reads 120% and develops at 120%, and shows "120%" with the thumb at
the track's end. The slider clamps on its first touch: a key step or
a wheel notch to 50%, a drag to wherever it lands in the range. So
the clamp comes with the first touch, not on reading.

**CPU and GPU.** The parity test gains cases at 50 and 200: at 200
the pictures are 9.5e-7 apart and the masks 7.7e-7, the stats equal.
The random sweep now draws over 0 to 2.

**Measured** on three noisy dusk frames, ISO 500 to 1600, 24 MP,
release build. The share of the picture blended (the mask's mean):

| frame | auto | 50% | 100% | 200% |
|---|---|---|---|---|
| ISO 1600 | 30% (at 19%) | 5% | not run | under 0.5% |
| ISO 1250 | 32% (at 17%) | 13% | 8% | 4% |
| ISO 500 | 36% (at 13%) | 10% | 3% | 1% |

In a box of open sky no pixel moved by 0.5% at 50 or at 200 on any of
the three (automatic: 0.05 to 0.07%). Strong edges keep their sharpen
at 200, lighter: building edges on the ISO 500 frame had 23% of their
pixels moved by over 0.5% at 50, 2.4% at 200, and the high-pass
energy there rose 18% and 11%. On the ISO 1600 frame almost nothing
passes 200, which there is near off. The skies here were
left alone at 50 already; the tester's frame is not ours, and we
guess it a brighter or a noisier one.
