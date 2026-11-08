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

## What was built

The mask's guard now asks the card a second time, and only certain jobs
may fall back to the CPU. `Ai::mask_room` (greycard-engine, ai.rs) runs
the old check first: `Ai::room` gives the Ai's own sessions back,
waits out an export's patience, and asks again. If the card is still
short, what happens next depends on `Ai::set_cpu_fallback`.

**Fallback off** (the default). The mask is refused exactly as before:
it is left out, `mask_short` is set, and it is asked for again later.
These keep the refusal:
- a frame's masks made in the background (`Job::MasksFor`);
- a person looked for on another frame (`person_on`);
- a save;
- a develop's masks;
- an export frame's first pass.

`masks_for` and `person_on` now clear the shorts per frame.

**Fallback on.** These jobs may fall back:
- the open frame's Mask, People, and Landscape jobs;
- an export frame tried again after the cross-model give-back;
- any frame of a set whose earlier frame met a short.

For these, the engine first lets go of what a develop makes again (the
release, below) and reads the budget again. A second read that is not
known counts as fitting. The log says what went and the free memory
before and after. If the card is still short, the job's masks run on
the CPU: `mask_providers()` returns `[Cpu]` for the Sky, Subject, SAM,
and SAM 3 loads. A canceled export is refused instead and runs nothing
on the CPU.

**In an export:**
1. The first pass refuses the mask, and the frame fails.
2. The worker gives the card back across its models (the open frame's
   Ai, the set's, People, and the background masks) and tries the frame
   again. On this second pass the mask may run on the CPU.
3. The patient walk keeps the short recorded, so the set's later frames
   ask once (`tries: 1`) and then go straight to the CPU. Without the CPU those
   later frames would fail, since the retry is barred for a short set.
4. After each frame the worker takes the mask short and gives the card
   back across its models.

**The Sky shape's SAM** asks the card before the decode, the way the
Object shape does. It asks only when SAM is cold, the gate has passed,
and SAM is in the store, so a frame with no sky never pays for it. A
refusal leaves the prior's labels as the outline, as before.

**Which sessions were loaded on the CPU by choice** is recorded per
session (`CpuChosen`). It is set at each load, carried with a session
lent by `lend_parts`, and cleared where the sessions go: `drop_sessions`
(and so the give-back), `release_parts`, and `forget_subject`.
- Going to the CPU drops the mask sessions that are on the card, judged
  by each session's own provider.
- Once the card has the room again, the sessions chosen for the CPU and
  still on it are dropped.

**The release** is `greycard_gpu::Releaser` (`Context::releaser`). It
holds the sharpen's and the local contrast's working sets (now behind
an `Arc`), lets them go, waits on the device, and returns their bytes.
- The worker installs it only for the open frame's Mask, People, and
  Landscape jobs (`open_frame_masks`). It clears it, along with the
  fallback switch and the status hook, after each job and after a
  panic, so a save after a panic does not inherit a CPU choice.
- These jobs run on the worker's thread with no develop running. The
  release takes each op's own lock on its set, which a running op
  holds; it never takes the queue's lock.
- The open frame's device points, its upload, and the picture on screen
  stay. The next develop makes the sets again.
- The shelf and the prefetch hold nothing on the device
  (`Kept::shelve`), so the release is the two working sets.

**Status.** Before a CPU run, the worker sends
`Outcome::MaskOnCpu { key, file, status }`. `key` is the mask's, or
none for People and Landscape. `status` is `part_on_cpu`'s line, so
there is no new wording.
- The UI shows it only for the open frame's file, and for a keyed mask
  only if it was asked for and is not quiet.
- After the run, the existing "<shape> found on CPU in N s" line
  follows.
- After Mask, People, and Landscape jobs alike, the worker takes the
  mask short and gives the card back across its models.

**Measurement.** `crates/greycard-ai/examples/mask_peak.rs`, run on
the RTX 5070 Ti on 2026-11-08, each model in a process of its own,
the preview-sized picture, two runs: Sky peaks at 0.80 GiB and holds
0.76 after; Subject (BiRefNet, fp16) 5.92 and 3.24; SAM 2.46 and
1.84; People (SAM 3) 4.40 and 2.65. The 2.0 GiB we had inferred was
a third of the largest. One figure would refuse the Sky on a card
with room for it, so `mask_needs` takes a `MaskModel` and each of
the seven asks names its own: 0.9, 6.0, 2.5, and 4.5 GiB over the
margin.

## Verified

All runs use the fake budget (`free_memory_by_test`); the release adds
the bytes it reports to that figure. Engine tests:
- `a_mask_the_card_is_short_of_runs_on_the_cpu`: short with nothing to
  release goes to the CPU. The worker is told once and the result is
  not a refusal. Within a job it asks once. With room, or with the
  budget unknown, the mask stays on the card. A machine without WebGPU
  never asks.
- `the_engine_lets_go_for_a_mask_before_the_cpu_is_asked`: short, then
  enough after the release, stays on the card. Still short goes to the
  CPU. Room from the start releases nothing. Also checks the log lines.
- `a_mask_that_may_not_fall_to_the_cpu_is_refused`: with the fallback
  off, the mask is refused and nothing is released. Turning the switch
  off clears the CPU choice.
- `an_export_falls_to_the_cpu_only_when_tried_again`: the first pass
  waits its tries and is refused, with the short recorded. Tried again,
  it runs on the CPU with the short kept, the frame not failed, and the
  worker's give-back due. The next frame goes to the CPU after one try
  with no wait. A canceled export runs nothing and releases nothing.
- `a_dropped_cpu_session_clears_the_cpu_choice`: the per-session record
  clears in `drop_sessions`, the give-back, `release_parts`,
  `forget_subject`, and with the room again.
- `the_shorts_are_taken_where_their_job_ends`: unchanged, still refuses.

Worker test on lavapipe, `a_masks_release_keeps_the_open_frames_device_points`:
the Releaser empties both working sets and reports their bytes. The
open frame's PreSharpen point and upload survive (same `Arc`). The next
develop runs on the GPU and remakes the local contrast's planes. It
does not show the driver's budget rising.

greycard-gpu on lavapipe: the releaser covers the local contrast's
kept planes test.

fmt; clippy -D warnings (ai, gpu, engine, ui); engine, gpu, and ai
tests on lavapipe; ui tests.

## Not verified

- That Vulkan's budget actually rises after a release, rather than wgpu
  keeping the memory pooled.
- That the CPU mask matches the GPU mask. These ignored reference tests
  need WebGPU and the store, and were not run:
  `greycard-ai/tests/sky.rs::the_sky_prior_is_the_same_on_the_cpu_and_on_webgpu`
  and `tests/models.rs::the_webgpu_rewrite_answers_as_the_original`.
- The export flow end to end through the worker loop with a real
  learned mask. A worker-level test would need a model store, and the
  worker's Ai reads the user's store. The sequence is tested on the Ai
  alone, the way the worker drives it.
- The second ask does not release the worker's other Ais' sessions;
  the give-back after the job does.
- While the card stays short, each job's give-back drops the CPU
  sessions, so the model reloads every job.
- Windows/DXGI and the reporter's 4 GB card.
- The UI `MaskOnCpu` arm has no test of its own.
- A `MaskOnCpu` line for People stays on the status line if the
  People job then returns early with no press pending; the next
  status replaces it.
- In a patient walk the short kept after a CPU fall also makes a later
  model short in the same frame skip its own give-back; masks come late
  in the walk, so this has not been seen.
- The retry pass waits the full patience again before falling to the
  CPU, though the card was just given back.
