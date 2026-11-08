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
