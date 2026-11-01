# §507. Structure, a wider and softer local contrast

*2026-11-01*

Asked on pixls.us: a local contrast at a larger scale than Clarity,
soft rather than crunchy, as darktable's local contrast at a wide
detail scale gives. Lightroom has nothing like it; Capture One's
Structure is finer.

**A third band.** Structure sits beside Texture and Clarity in the
Detail section, global and in a mask's Detail alike, -1 to 1. Its
band is Clarity's guided base less a guided base of that at a
twelfth of the long edge (3.3 times Clarity's radius; 500 px at
6000), so the three bands and the last base add up to the low-passed
log and no scale is lifted twice. Its epsilon is a fifth of
Clarity's, chosen by a sweep of a 3-stop step's halo against a
half-stop swell's lift (0.05 stops against 0.16). The band goes
through a soft knee, 0.3 stops times a tanh, so a hard edge is never
pushed further, then Clarity's mid-tone weight, the slider, a gain of
1.5, and the same fade toward the clip. The filter is exact under a
radius of 64, on a grid of 4 to 255, and of 16 beyond, which keeps
the grid radius inside the GPU's 128 at a 16384 edge.

**Measured at the slider's ends.** A 3-stop step edge moves at most
0.085 stops on either side; a 1 px hairline over a flat field moves
0.019 stops, a faint wide lift and not a rim; a slow half-stop swell
is spread 1.58 times wider at +1 and 1.9 times narrower at -1; a
straight ramp is unchanged; a Clarity-sized bump moves 0.15 times as
much as under Clarity; a clipped plateau is untouched. The GPU is
held to the CPU reference within Clarity's tolerances (at most
1.5e-5 against the f64 port), a whole-frame mask equals the global,
and the shader's reading of a mask's band is within two levels of
the CPU finish. At 0 no band is made, no pass runs, the field is left
out of the sidecar, and the develop, its key, and its cost are as
before, Texture and Clarity bit-identical. At 45 MP Structure adds
about 0.14 s on the CPU and 0.3 s on lavapipe.

**Where it lives.** A mask's band rides in the planes' fourth lane on
the GPU and as a second image on the CPU, made only when a mask asks.
`greycard develop` takes `--structure` beside `--texture` and
`--clarity`. The word is one constant on each side, Rust and Slint,
held equal by a test, since it is the one name here the user has not
yet weighed.

**Open.** At +1 it is quiet: 1.2 % RMS on a portrait against
Clarity's 3.3 %, which reads as about Clarity +35. The reviewer would
raise the gain to 2.5 or 3 (a halo of about 0.15 stops), or epsilon
to 0.1 with a gain of 2; the gain is one constant. That is a judgment
to make on real frames.
