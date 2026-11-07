# §591. A quick demosaic on the GPU, and the default behind it (2026-11-07)

Frame switching is the complaint testers repeat (issue 30, issue 71,
discussion 59). The shelf (§378) made the return to a frame cheap and
the prefetch (§379) the walk forward, but the first open of a frame
still pays a fresh base, and the demosaic is most of it: 1.2 s of
1.9 s at 45 MP on eight threads, 2.8 s of 6.5 to 7.0 s at 102 MP on
an M5 (§479), now the lens and the chromatic aberration run on the
GPU. The camera's JPEG or the edit's picture stands in meanwhile
(§134, §270, §580), so what the user sees is a picture that is not
the develop for a second or seven.

The default demosaic for a Bayer frame is AMaZE where there is
detail and VNG4 where there is only noise, the threshold from the
frame's measured noise (§13; `dual.rs`). It scored above AMaZE alone
at every noise level, clean included, for about twice the time, and
that decision stands. AMaZE is the one demosaic that resists a GPU:
long, branchy, and many passes over shared neighborhoods; darktable
never ported it to OpenCL either. RCD, VNG4, and the dual's blend
mask are each a few passes of local arithmetic, the shape every op
in `greycard-gpu` already has.

## The decision

Two routes, both taken, one as the build and one as a trial.

**The build.** When the method is the AMaZE dual and the walk has a
device, the viewport's develop runs the RCD and VNG4 dual on the GPU
as its demosaic stage, with the threshold the CPU dual would have
used (the noise model's thresholds by lightness, or the automatic
one, both computed on the CPU as today from the reference), so the
picture lands in a fraction of the time it takes now. The worker then
makes the default's base on the CPU as its next job, ahead of the
prefetch and behind the user's own asks, and lands the develop again
from it. At the fit view the two are indistinguishable, since the
two duals differ at single pixels along edges; at 1:1 the default is
there within the time it takes today. A slider dragged across the
swap sees those pixels change, which we accept.

A base made from the quick demosaic is marked quick. The shelf keeps
a quick base until the default replaces it, and a quick base taken
from the shelf is brought up the same way. A new open drops a
pending default; the prefetch, which makes the default directly,
brings it later if the frame is a neighbor. Export, the headless
command line, the reduced pictures, the mask bases, and the prefetch
never run the quick stage: their walks have no stand-in to shorten,
and the export stays exactly what it is. RCD chosen by hand runs on
the GPU too and needs no second pass. X-Trans keeps Markesteijn on
the CPU; a quick route for it is a decision of its own.

No setting and no UI text. The log says `demosaic on the GPU (quick)
0.08 s` on the first landing and `base: the default's demosaic 1.9 s`
on the second, so a tester's log shows both.

**The trial.** AMaZE itself on the GPU, in a branch, checked against
the CPU port and timed. If it comes in equal to the reference and
quick, the viewport's walk runs it in the demosaic stage and the
second pass above goes away for machines with a GPU; if it does not,
the branch is kept as the record of why.

## The shaders

RCD, VNG4, and the blend in `greycard-gpu`, each a module and a WGSL
file beside the lens and the chromatic aberration, Bayer only, with
the dual's `blend_mask` on the GPU from the detail result's lightness
as `dual.rs` computes it. Each is checked against its CPU reference
on the same mosaics in `crates/greycard-gpu/tests`, on NVIDIA and
on lavapipe (§221), to the tolerance the other ops hold. The
reference stays the CPU's: the GPU implements it and is compared
against it, never the other way about.

## Measured first

Before the build the dual's time is split, AMaZE against VNG4
against the blend, at 45 MP, and the two duals compared at the fit
view's scale on a clean frame and a noisy one, so the claim that
they are indistinguishable there is a number and not a sentence.
