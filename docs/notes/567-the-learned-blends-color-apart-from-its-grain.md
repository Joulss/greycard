# §567. The learned blend's color, apart from its grain

*2026-11-04*

A tester set the learned denoiser's blend to 0.41 to keep the grain
and found the picture kept 59% of the raw's color noise with it
(issue 70). The blend mixed the network's answer and the plain
demosaic per sample, so luminance and chroma moved together. They
wanted the color clean and the grain left.

The blend now takes two strengths, `learned_strength` for the
luminance (the panel's Luma, Blend until now) and
`learned_chroma_strength` for the chroma. A pixel's
change from plain to model, `d`, is split into its luminance `Y`,
Rec.2020's weights on `d` laid on all three channels as a grey
change, and the rest, `d - Y`, the change's color. The working space
is linear Rec.2020, so the split is exact and costs a dot product a
pixel. The result is `plain + c d + (s - c) Y`.

The chroma's strength is full by default and left out of a sidecar
there: color noise has no look to keep as grain has, so the usual
setting is Chroma full and Luma to taste. An edit from before had
one strength for both, seeded from the ISO on every open, so no
constant could stand for it; schema version 7 writes that blend into
the chroma field of every edit from before whose learned tier is on,
and equal strengths run the old per-sample mix bit for bit, the fast
paths at 0 and 1 included. Those edits develop as they did, with a
develop key that changes once; an edit with the tier off is left as
it was. A 0.9.1 build refuses a version 7 sidecar, as it refuses any
newer one. A sync and a paste carry the strength with the tier, a
preset does not, and the history names it.

The blend runs on the CPU only; there is no GPU blend to check. The
slider's word is in one place on each side (`NoiseWords` in the
Slint, `greycard_edit::LEARNED_CHROMA` in Rust), with a test holding
the two together.
