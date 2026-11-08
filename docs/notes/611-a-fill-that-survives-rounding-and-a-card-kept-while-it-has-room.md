# §611. A fill that survives rounding, and a card kept while it has room

*2026-11-08*

A tester on a 12 GB card (issue 92) lost the window's device while
the People mask's model loaded for a second time beside the remove's
sessions, and the run that §412 started in its place, with the
engine's ops on the CPU, remade 23 generative removes at 17 to 40 s
each, half of each a reload of the UNet. Three logs show three
things wrong, two of them ours to fix.

## What the logs say

A fill is kept on disk (§425) under a key that includes a print of
the window's pixels round the hole (§566): the means of 16-pixel
blocks at half precision, hashed into the file's name. The print is
there so a fill made under another white balance, lens, or turn is
not served. But the CPU's base and the GPU's differ by rounding, and
half precision has a step of a part in two thousand, so some block
in nearly every window lands on the other side of a step and the
hash misses. The run under `--cpu-ops` found none of its fills. The
same happens to every tester at 0.11.0: the GPU demosaic (§591) moves
the base's pixels by rounding, and every cached fill goes stale at
once.

The card is given back after any fill's model has run
(`give_back_card`, §578): every ONNX Runtime session dropped, since
the WebGPU provider keeps one context for all of them and its cache
frees nothing while any session lives. On a 4 GB card that is what
keeps the window alive. On the tester's 12 GB card it meant the UNet
(10 to 12 s) and the SAM 3 encoder (46 s) loaded again for every
picture that asked, with 8 GiB free.

The thumbnail pool's reduced develops refuse fills (`Fills::Refuse`),
so an edited frame with a fill has no kept edited pictures until the
frame is opened and its fills land, and the strip and the grid show
the camera's preview until then. A People mask with no person picked
is never kept, which is right, and logs three lines a keep, which is
noise.

## The decision

**The print is compared, not hashed.** A fill's slot on disk is named
by the stable parts alone: the file, the patch as `same_fill`
compares it, its seed, the picture's size, the region, the window,
and both engines' models. The print is written into the file after
the header, and a read compares it with the print of the picture now:
the fill is served when every block's mean, per channel, is within a
tolerance of the stored one, relative to the larger of the two and a
floor against black. The tolerance sits between what rounding moves
(the CPU against lavapipe on the same frame) and what the smallest
edit a user makes moves (one mired of white balance, one step of a
lens slider); the builder measures both and writes the figures here.
A fill made under a turn or a crop has another window and misses as
before. The in-memory slot compares the same way. The file's version
goes to 5; the old slot at version 4 is tried once under the exact
print as it always was, and a hit is written again under the new
name, so a machine whose base did not move (ops on the CPU all along)
keeps its fills across the upgrade. Everyone else remakes each fill
once more at 0.11.0, and the changelog says so.

**The card is given back when it is short, not after every fill.**
After a fill's model has run, the worker reads the card's free memory
with the sessions held and gives them back only when it is under a
floor: what the window's own develop may take without asking (its
frame's textures and the GPU ops' working sets on the largest frame
open). Every model run already asks for its room and gives the card
back when short (§578, §610), so the floor only has to cover the
allocations that do not ask. The floor starts at 3 GiB over
`CARD_MARGIN` and is measured on the real card from the log's free
figure before and after a 45 MP develop lands; the builder adds that
log line. On the tester's card the remove's UNet held leaves 1.5 GiB,
so it still goes, and so does everything with it, since the
provider's cache is all or nothing; LaMa, the denoiser, and the mask
models stay on a card that has the room. On a card at 4 GB nothing
changes.

**The reduced develop reads a fill it has.** A new `Fills::Read`
refuses only a fill that is not on disk. The fill is kept at full
size; the reduced develop knows its factor, computes the full-size
region from the patch (`Patch::region` is pure in the patch and the
size), finds the slot, compares the print against the reduced
picture's blocks of 16 over the factor (a factor that does not divide
16 refuses), and takes the region down by the factor with a box
filter before applying it. A fill made since the pictures were kept
is picked up at the next keep, as today.

**A People mask with no person picked** is still not kept, and says
so once a keep.

## Not changed

The SAM 3 encoder's 46 s session build is the provider's shader
compile and is not cached across sessions by ONNX Runtime today; it
is on the roadmap's Speed track. A fill remade because the picture
really changed is right and stays.

## What was built

**The print compared.** A fill's slot is named by the file, the
patch, its seed, the picture's size, the region, the window, and both
engines' models; no print. A slot keeps `BASES_A_SLOT` (3) bases'
fills, as the mask cache does, in files `<slot>-<n>.fill`, read newest
first; a fill is written over the file whose print is its picture's,
else into a file not there, else over the oldest by its time. So
virtual copies of a frame with another white balance each read their
fills in turn, and one with the same base shares the fill. Each file is at version 5: after the header,
the print's block count and its blocks (f16 means per channel), then
the values. A read compares the stored print with the picture's now:
every block within 0.003 of the larger of the two and a floor of
1/256 (the working range's white is 1.0), an empty block matching only
an empty one. The fill in memory compares the same way. On a miss, the
version-4 slot (the exact print hashed into its name) is read once; a
hit is written again at version 5, and the old file removed only once
that write has landed.

**The card kept while it has room.** `give_back_card` reads the card
with the sessions held after a fill's model ran and gives them back
only under `KEEP_FLOOR` (3 GiB, a first figure) plus `CARD_MARGIN`, or
when forced after a short; kept, it logs "{card}: the models'
sessions kept, {free} GiB free". Unknown free memory (unified memory,
lavapipe) is unchanged: nothing given back. The develop's log line
carries the card's free memory before and after a develop that ran on
the device, the figure the floor is to be measured from; it is asked
only once the models' probe of the machine has run, so a develop never
sets off the adapter's probe on the worker.

`KEEP_FLOOR` is a first figure, not measured. Once the sessions are
kept, nothing gives the card back for a bigger frame opened after:
the window's develop does not ask for room, and the card is given back
only when a model next asks for its own and is short.

**The reduced develop reads a fill it has.** `Fills::Refuse` is
replaced by `Fills::Read { factor, full }`: the bin and the full
develop's size (`Facts::binned(1)`, carried rather than worked out
from the reduced size, which can be short of it by up to a bin's
pixels). The walk asks `Models::fill_read`, which is `None` by
default; the thumbnails' pool walks with a `FillReader` taken from its
`Ai` for the frame (its masks root and file), whose `fill_read_only`
finds the slot from the full-size region and window, compares the
stored print with the reduced picture's blocks of 16 over the factor
(a reduced pixel joins the block its center falls in, and is left out
where any full pixel under it is in the hole), and box-filters the
region down by the factor; a factor that does not divide 16 (X-Trans's
3) refuses. The reduced compare is by the median block, within 0.01
(below). A miss names the fill in `without` as before. The pool reads
version 5 only, never the version-4 slot, so after the upgrade a
frame's thumbnail gains its fill once the frame has been opened and
its fills kept again.

**A People mask with no person picked** is said once a keep, however
many of its shapes ask.

## Measured

`walk::tests::the_fill_print_measured` (ignored; `GREYCARD_PRINT_RAW`
names a raw) prints, over five fills' windows on a frame, the largest
relative move of any block's mean and its spread. On lavapipe, a
Canon CR3 and a Nikon NEF, the device running the CA, the demosaic,
and the sharpen:

| | CR3 | NEF |
|---|---|---|
| CPU against the device | 0 | 0.0008 (one f16 step) |
| one mired, largest block | 0.016 | 0.0056 |
| one mired, median | 0.0032 | 0.0030 |
| three mired, median | 0.0096 | 0.0092 |
| one lens distortion step (0.005), largest | 0.95 | 0.83 |
| one lens step, median | 0.080 | 0.056 |
| exposure +0.1 EV | 0 | 0 |
| reduced develop (bin 2) against full, largest | 0.34 | 0.12 |
| reduced against full, median | 0.0041 | 0.0021 |

Exposure is the finish's and never moves the base: a fill stays
served across it, rightly. Lavapipe moves the prints by a step at
most; a real card's FMA and its own demosaic may move more pixels
across a step, but a step is a part in 1024 to 2048, so the full
tolerance is 0.003, a few steps, under one mired's largest block on
both raws. The reduced develop's bin moves single blocks as far as a
lens step, so its compare is by the median: 0.01, over the bin's
0.004 and under three mired's 0.009 plus the bin's. A thumbnail's
fill is told from one under a lens step or several mired of white
balance, not from one under a single mired.

**On the card** (an RTX 5070 Ti, the same five windows, the CA, the
demosaic, and the sharpen on the device): the largest block moved
0.00088 on the Canon frame and 0.00094 on the Nikon between the CPU's
base and the card's, one half-precision step, as on lavapipe; one
mired moved 0.00577 and 0.00563. The tolerance of 0.003 holds on real
hardware with the margin the lavapipe figures gave.

## Verified

- `cargo fmt`, `cargo clippy --all-targets -D warnings` on
  greycard-engine, greycard-edit, and greycard-ui, and `cargo test -p greycard-engine
  -p greycard-edit -p greycard-ui` on lavapipe.
- A print moved under the tolerance is served on disk and in memory,
  over it not; an empty block against a full one is not; the median
  compare passes one far block and refuses a moved picture; a version-4
  file is read once, rewritten at 5, its old slot removed; the slot
  name takes no pixels.
- Sessions kept above the floor, given back under it, given back when
  forced; the worker's give-back counts at room and short.
- Three bases' fills of one patch read back in turn; the same base
  again writes over its own; a fourth evicts the oldest. A version-4
  fill whose version-5 write fails is served and its file left.
- The box filter's region offsets, by a hand-worked case.
- A fill kept at full size is read at factor 2 and 4 within 1e-6 of
  the fill box-filtered; not on disk, another file, a moved picture,
  and factor 3 refuse; end to end, a reduced develop of a linear DNG
  reads the fill a full develop kept and is not refused.

## Not verified

- `KEEP_FLOOR` on a real card: the new develop log line is there to
  measure it from.
- The tolerance against a real card's device develop (only lavapipe,
  which was bit-identical in prints on the CR3).
- The reduced read at factor 2 end to end on a Bayer raw (the
  end-to-end test is a linear DNG, factor 1; factor 2 and 4 are tested
  at the reader).
- That the open frame's path and the thumbnail pool's are the same
  string for one file (the slot hashes the path).
