# §610. A learned mask on a card the editor has filled (2026-11-08)

Issue 91, a 4 GB RTX 3050 Ti Laptop on Windows: a Sky or Background
mask is refused with "1.9 GiB free, short of the 2.5 GiB a learned
mask needs; another program may be using it". No other program was.
The card held greycard's own 26 MP base, the local contrast's planes,
the sharpen's working set, and the kept device points, and the guard
(§421's rule carried to masks) read Vulkan's budget after all that
and said no. Lightroom runs its masks on the same card because it
holds less and does not refuse up front.

## The decision

The guard asks a second time, after the editor has given back what
it can. When a learned mask's room check finds the card short, the
engine releases what it holds that a develop can make again: the
working sets of the local contrast and the sharpen, the prefetch's
device bases, and the kept device points of frames other than the
open one; the open frame's own device points stay, since the mask
is made for it. Then the budget is read again. Only a card still
short after that is short, and the message says what was freed.

A card still short does not refuse. The mask runs on the CPU, as the
People model's parts already do on a machine without the room
(`parts_on_cpu`), with the status line saying so and the time it
takes, and the log naming the card's free memory before and after
the release. A mask on the CPU is slow, tens of seconds at 26 MP,
and it is right: a slow mask is a mask, and an error is not.

The number itself is checked: the 2.0 GiB peak plus the margin was
set from a larger model's run, and the mask models' real peak on a
card of this size is measured on this machine's card with the budget
read before and after a run, so the ask is what the model takes and
not a guess with room to spare.

## The mechanism

`Ai::mask_room` (greycard-engine, ai.rs) gains the release-and-ask-
again step through the worker, which owns the context and the kept
points; the release is the engine's existing `Context::release` and
the shelf's and `Kept`'s device drops, never a device destroyed. The
CPU run reuses the provider fallback the parts take. `MASK_PEAK` is
set from the measurement. No UI wording beyond the status line the
parts already use for a CPU run.

## Verified
(To be filled from the build.)
