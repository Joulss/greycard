# §617. A removal on a card the editor has filled (2026-11-09)

Issue 104, an 8 GB RTX 5060 Laptop on Windows: a frame with twelve
fills opens with three of them refused, every time, with "5.9 GiB
free, short of the 6.3 GiB a removal at 512 needs; another program
may be using it". No other program was. The nine fills LaMa makes
came back from the fill cache; the three marked Remove want the
UNet, and the guard (§421's rule) read Vulkan's budget after the
editor's own base, the local contrast's planes, and the sharpen's
working set were on the card, and said no. The tester had made those
three fills on this machine once.

## The decision

The removal's guard asks a second time, as the learned mask's does
since §610. When a Remove fill's room check finds the card short,
the engine releases what it holds that a develop makes again, the
local contrast's and the sharpen's working sets, reads the budget
again, and begins if it now fits. The log names what was given back
and the free memory before and after, so the next Windows log says
whether the margin is the wall or the editor's own holdings were.

A card still short after that is refused as before: the fill is left
as the base has it and asked for again later, and the message says
what was freed. A removal does not fall to the CPU, which takes
minutes a fill, and does not fall to LaMa, which the user did not
choose; a hole the user can see is answered by the log, a quiet
change of model is not.

The peak itself stays: 5.8 GiB at 512 is what `remove_bench` read on
a 16 GB card, and nothing here can measure an 8 GB one. If the
tester's next log shows the card short by less than the half-GiB
margin after the give-back, the margin is the next question.

## Also in the log

The thumbnail pool refuses fills, so the frame's Library thumbnail
shows the camera's preview (the pool's own line says "not kept: the
fill Fill 2; ..."); issue 92 found the same. And the window at rest
drew 660 to 1085 frames a minute, all for a scope readback, nothing
busy: the Windows log discussion 51's roadmap line waited for.

## What was built

`Ai::room_for_remove` (greycard-engine, ai.rs) asks a second time, as
`Ai::mask_room` does. `Ai::room` runs first, its own give-back as
before. Still short, the engine lets go of what the release holds
(`Ai::release_card`), reads the budget again, and asks once more, the
UNet's need found afresh:
- fits, or the second read is not known: the log says "a removal at
  N: begun once the engine let go of ..., X GiB free before and Y
  after", and the short `room` recorded is put back;
- short still: refused as before, its message the first refusal's
  with what was let go and the two readings, "short still";
- no release set, or nothing in it: refused with the message
  unchanged;
- a canceled export lets nothing go.

The worker installs the release (`card_release`) around the open
frame's `Job::Fill` round, cleared after it and by the job's panic
arm. Open, Develop, and DevelopAside run a first round with no fill
budget, so they ask no card. Saves, exports' sets, the background
masks, and the prefetch get no release, as on the mask path. The
release runs inside the fill's walk at its retouch, between the ops'
runs, so no op holds its set's lock; the walk makes the sets again
past the retouch, so a queue's later fills may let go once more, and
the log says so each time.

## Verified

- A cold removal at 512 on 5.9 GiB free begins once 0.6 GiB is let
  go, and is refused with the freed set named when 0.2 is; with no
  release, or one with nothing in it, the old message.
- A fill job sets the release, asks it once across three removals,
  and leaves none set between jobs.
- fmt, clippy, and greycard-engine's tests on lavapipe (476).

## Not verified

- On a real card: that the budget rises by what is let go, and that
  the 8 GB laptop's frame now makes its three removals. After an
  admitted removal the walk makes the sets again while the UNet is
  still held for the queue, so the pressure may move to the ops
  after the fill; the tester's next log says.
