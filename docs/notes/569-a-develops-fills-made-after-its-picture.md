# §569. A develop's fills made after its picture

*2026-11-04*

A tester's frame with 34 generative fills felt hung (issue 71). Each
base change made every fill again, 11 to 36 s each; one at a time,
the remove opened its UNet for each fill and dropped it after, 4 to
5 s a fill; and a develop showed nothing until its last fill was
made, 270 s for 15. The first is right while a fill is made from the
base under its window (§566); the other two are this note.

## The picture first

The window's develop now walks with a budget of no fills: a fill kept
in memory or on disk goes in, and one the model would have to make is
left as the base has it and listed in `FillReport::pending`. The
picture is delivered at once. The worker then queues its own
`Job::Fill`, behind any develop asked for and the People, Landscape,
and refined-mask asks, ahead of the masks, which a landing fill would
ask for again. Each round drops the kept points past a retouch that
left fills unmade, walks from the base with a budget of one, and
delivers its picture under the develop's own generation, so the
window takes it as that develop's and the fills land one by one. The
count on the status line ("Fill 5, 4 of 15") is carried from round
to round.

A develop asked for is taken before the next round and gives the rest
up; its own first walk lists them again, so a slider moved mid-queue
costs the fill in hand and nothing else. Leaving the editor for the
grid or the cull pauses them, the held UNet let go; showing the frame
again sends no develop, so the worker goes on with them when the
frame is open once more, and a save waiting on them waits on. An
export is taken before the next round, since it makes its own fills
in its one walk. A walk that
starts past the retouch, from a point that left fills for later (a
sharpen move on the device), lists that point's unmade fills that
could be made on the base, so the queue goes on after it. The rounds
end when one leaves no fewer fills to make than the one before, or
makes and fails none: a model that will not load fails the same fill
each round without its failure being kept, and would otherwise go
round for good. Rounds that end with fills left say none to come, so
the outlines and the bar go, and the frame's pictures are not kept as
the edit's. A save of the edit's pictures waits for the last fill, as
for a develop still to land.

The viewport draws each pending patch's edge on any tab, the chosen
patch's stroke at a third of its strength (Slint's Path has no
dashes), until a develop lands with none pending. The status plate's
bar is the camera match's: a 60-pixel track filled to the fill's
steps, `Outcome::FillStep` carrying the remove's UNet calls (24 for
Best, 3 for Fast); LaMa, one call, leaves it empty until it lands.
While a develop is on its way the plate keeps its sweep. No new words.

Each round costs a walk from the base: the retouch with its kept
fills, 0.1 to 0.5 s in the tester's log, then the upload and the
sharpen, about 1 s a fill against 11 to 36 s for the fill.

## The UNet across a queue

One at a time, `Remove::hold_unet` keeps the UNet open after a
removal while more are queued, and the last of the queue runs the one
held and lets it go after; `release_unet` lets it go at once. The
worker holds it while a round has more after it. Any job but the next
fill or a save lets it go, and so does a round whose base is no longer
kept, which runs the learned denoiser again. LaMa may still load
beside it within a round, as it loads beside a removal one at a time.
A UNet held at another size or sampler goes before a removal's encoder
opens.

The hold depends on the card's memory as read (`queued_up_to`): at 512
from a discrete card of 12 GB, at 1024 as well from 14 GB; on unified
memory from 16 and 24 GB, where the mode keeps the UNet anyway; never
where the memory is not known. Measured with `remove_bench`, three removals of one window, seeds 1 to 3, an RTX
5070 Ti on WebGPU, the peak in GiB over the desktop:

| | loads, s | peak |
|---|---|---|
| 1024, one at a time | 5.3, 6.0, 5.7 | 6.6 |
| 1024, UNet held | 3.7, 0.18, 0.23 | 9.9 |
| 512, one at a time | 5.1, 5.7, 3.7 | 5.8 |
| 512, UNet held | 3.6, 0.15, 0.20 | 6.6 |
| 512, UNet held, denoiser run beside | 3.9, 0.15, 0.18 | 7.3 |
| 1024, one at a time, denoiser run beside | 4.9, 4.0, 3.8 | 8.6 |

With the UNet held its buffers stay in ONNX Runtime's cache and the
next encoder's are made beside them: 3.3 GiB more at 1024, which an
11.8 GiB card with a desktop and the editor's textures may not have.
At 512 a held queue asks no more than a 1024 removal asks, the
denoiser beside or not; beside a resident denoiser that is 7.3 GiB,
more than an 8 GB card has with a desktop and the editor's textures,
so the bound is 12 GB. The tester's 512 fills, 11 s each with
4 s of loading, save that 4 s after the first; their 1024 fills, 35 s
with 4.6 s of loading, run as before. A 1024 queue on a card under the
bound runs one at a time (checked: 3.8 and 3.5 s of loads, 6.7 GiB).

## Limits

- A pending patch shows the base under it, not a fill of the same
  patch and window made for another picture, which memory may hold.
- A fill in hand finishes before a develop asked for starts, up to
  36 s at 1024; stopping between UNet calls is where the step count
  already reaches.
- The 14 GB bound for holding at 1024 is the measured 9.9 GiB peak
  with room for the desktop and the editor's textures, not run on a
  card of that size.
- Measured with the bench, not in the editor's window; the rounds
  and the cancellation are tested with fills made by no model.

## A fill carried across a white balance (not built)

White balance is gains in camera space before the matrix (§3), so in
the working space a change of it is a 3x3 map, M′ D′ D⁻¹ M⁻¹. M itself
can move with the white balance: a dual-illuminant DNG interpolates
its ColorMatrix by the white point, so M′ is the new white's matrix,
and the map is still fixed for the two settings. A fill made under the old gains could be carried to the new ones by
that map exactly when every stage between the gains and the retouch
commutes with it, giving the mapped picture for the mapped input, and
the print (§566) checked after it: the block means are linear, so the
old print mapped, compared to the new one at its half precision, says
whether the window's surround moved only as the map says. Commutes
with the map: the DNG warps and the lens (resampling and vignetting
gains), the CA correction once fitted, and the move into the working
space with its turn. Not: the prepare's clip and highlight
reconstruction after the gains, the demosaic (RCD's and AMaZE's
directions read the gained mosaic), the learned denoiser's network
and the profiled denoise (both read a noise model that the gains
scale), the learned denoiser's blend, and the defringe. The blend is
linear but split, `plain + c·d + (s−c)·Y(d)`: it commutes with a 3x3
map only when the luma and chroma strengths are equal, and its
default, chroma full and luma lower, does not.

So the carry is exact only where none of those changed their answer:
a window with nothing clipped, a demosaic whose choices did not turn,
no denoise or a blend at equal strengths. The print check would find
that, and would refuse most windows on a noisy frame. The model itself is not linear either: it
sees the window exposure-normalized and sRGB-encoded, so a new white
balance shows it another picture, and its answer made again would be
another fill, not the old one recolored. Carrying is a choice to keep
the fill's content, which a user who only changed the white balance
likely wants.

The regrain is baked into the kept fill: grain drawn to the frame's
noise as measured after the old develop's denoise. Mapped, it takes
the map's color; the frame's own grain under the new gains is not the
old grain mapped wherever a denoiser read the gains. A carry would
want the fill kept before its regrain and the regrain drawn again,
seeded as now, against the new frame: a second region's values in
memory and on disk, and a noise measure of the window, cheap beside
a fill. Short of an exact carry, the mapped fill is the stand-in a
pending patch should show while its fill is made again.
