# §578. A model run the card cannot hold is not begun, and the card given back

*2026-11-05*

The editor died during a Best fill at 1024: Vulkan reported
`vkAllocateMemory failed` (VK_ERROR_OUT_OF_DEVICE_MEMORY) through
Dawn, and three seconds later the process segfaulted inside the
NVIDIA driver under Dawn's command submission, called from ONNX
Runtime's WebGPU provider on the worker thread. The 16 GB card was
shared with two other processes running the same model. Tester issue
73, a crash during an export of 35 files while another editor used
the card, is the same kind. A second crash came through the learned
denoiser: after the remove's held UNet was let go, the process still
held 12.2 GiB of the card, the denoiser's run met the same error five
times inside ONNX Runtime, its answer was refused as implausible, and
the process aborted in glibc's realloc half a minute later (§431's
abort again). ONNX Runtime does not fail cleanly when the device runs
out of memory, so the only defense is not to begin a run the card
cannot hold.

## The card's free memory

`runtime::webgpu_free_memory` asks the adapter the WebGPU probe keeps,
at each call: on Vulkan, `vkGetPhysicalDeviceMemoryProperties2` with
`VkPhysicalDeviceMemoryBudgetPropertiesEXT` chained, the budget less
this process's use of the largest device-local heap (a second one,
the 256 MB BAR window, is a view into the same memory); on DX12,
DXGI's `QueryVideoMemoryInfo` for the local segment, the budget less
the current use. Each budget is the driver's estimate of what this
process may have, other processes' use taken out. It is a query of
the physical device, no device made. It says nothing on unified
memory (an integrated GPU's device-local heap is the firmware's
carve-out, and §569's bounds stand there), on a CPU adapter
(lavapipe), where the device lacks `VK_EXT_memory_budget` (wgpu
leaves it out on RADV), or where the instance lacks
`VK_KHR_get_physical_device_properties2`. The first figure goes to
the log. On the 5070 Ti it read 2.15 GiB free with the editor running
beside two other processes, where `nvidia-smi` said 2199 MiB.

The probe's instance and adapter now live for the process; on a
laptop with two GPUs that may keep the discrete one awake.

## The checks

A removal's need is its peak at the size the window runs at (§569):
5.8 GiB at 512 and 6.6 at 1024 one at a time. When that size's UNet is
open already, its 4.7 GiB of weights come off the held peaks, 6.6 at
512 and 9.9 at 1024. Half a GiB of margin goes on every figure. So a
fill at 1024 wants 7.1 GiB free, one at 512 6.3, and the next of a 512
queue 2.4. The need is worked out first and the card read after, so
the figure is as fresh as it can be.

Only the run asks. Counting the fills ahead does not: a fill the card
is short of is still counted and queued, so its run asks, gives back,
and may refuse it; a count that left it out would never queue it, and
nothing would ask again. The run asks once the walk is sure to make
the fill now, before anything loads: there a UNet being opened ahead
is waited for, up to two minutes (a Mac's takes tens of seconds), and
then counted. A fill left for a later round asks nothing. The UNet
opened ahead of a removal, where the memory mode keeps it, is not
opened on a card without room for its weights.

A refused fill is remembered by its patch and window with the free
figure then. A develop neither counts it nor asks the card for it
again until something changes: a give-back that let anything go, more
memory free than at the refusal, or the patch or its window changed.
Without that, a fill the card stays short of would bring a fill round,
and a give-back that drops and reloads every model, on every slider
move. An export always asks.

The learned denoiser stays loaded between develops, so its check sits
before its run, on a disk-cache miss. Measured on the 5070 Ti, the
best tier, the largest, holds 3.86 GiB after a run at a tile of 1024
and 1.49 at 512, of which its weights, on the card from the load, are
0.05; a second run asks nothing more. It wants 4.35 GiB free at 1024
and 1.95 at 512, past 1024 by the tile's area.

A learned mask's model is checked before it loads or runs its encoder
cold, and loaded after the check, since a short's give-back drops it: SAM and its embedding (for a Sky, once the gate has passed),
SAM 3, its people, and a Part on a picture it has not encoded,
Subject's matte, and the sky model's scene. A mask drawn from a kept
embedding, encoding, scene, or matte, or a SAM decode alone, is not
checked, so it does not drop out of the viewport on a full card. The figure is one, 2.0 GiB and the
margin, the largest of them (SAM 3) inferred from its weights, not
measured.

## Refused

Our own sessions are the first thing a short card holds. A 12 GB card
with the denoiser kept has 6.9 GiB free; a 1024 fill wants 7.1. So on
a short right before a run, before anything is refused, the models
that asked give the card back (below: every session they hold dropped, the UNet let go,
the heap trimmed) and the card is asked once more; the freed memory
shows in the budget at once (7.47 to 11.72 GiB, measured). A 512
queue's held UNet goes the same way when a 1024 fill follows it.

Still short, a refusal names the card, the free figure, and the need.
It is not remembered as a failure.

- A fill is `NoFill::Failed`; the worker's "fill X: the fill model
  failed (...)" line and the status carry it, the patch is left as the
  base has it, and it stays among the fills ahead. It ran nothing, so
  its place in the round goes to the next fill, which may fit: a big
  fill the card is short of lets the small ones after it by. A queue's
  UNet at the other size is kept through the refusal's give-back: the
  small fill after it would open it again, and beside the others'
  sessions letting it go gives nothing back. A need past the whole
  card gives nothing back and waits for nothing.
- The denoiser is `NoLearned::Short`; in a develop the engine's own
  path stands in for that develop.
- A mask is left out as a failed model's is.

After a develop or a fill's round in which a fill was refused, the
worker gives the card back across all its models (its own and those
kept for People, an export's other frames, and the masks of frames
not open). When that let anything go, the queue tries the fill again,
in its next round or, the queue ended, in one asked for again once a
develop, with as many fills left as the queue had, without waiting
for the user to touch anything. When it let nothing go, the fill is
remembered as above and the queue ends without it. A develop's first
round makes no fill, so only the fills' own rounds ask. A mask refused gives the card back the same way, for the next mask. The
flags that say a run was refused are cleared at each develop round and
mask, so one never outlives its job.

An export or a merge does not stand in, which would mix learned and
engine output in one file or one merge. It asks the card three times
in all, five seconds apart (ten seconds), the export's cancel looked
at between, and then fails the item as it fails any. The first short
stops the frame's walk asking: its other fills, masks, and denoiser
are not waited on again. The first frame of a set to fail so is tried
once more with the card given back across all the worker's models, the
editor's own included; later frames of that set are not. Once a set has met a short it did not recover from, its
later frames ask once; the set is held weakly, so one done or canceled
is forgotten. A merge has no cancel to look at between its asks, and
its own models never give the card back: one on its own thread, they
would drop sessions under the worker's run.

## No 1024 UNet held on a card

§569 let a discrete card of 14 GB or more hold the 1024 UNet across a
queue, a peak of 9.9 GiB against 6.6 one at a time. Held, it is the
largest single thing the editor keeps on the card, and a card is
shared: another process can take the rest at any moment, and the
next removal is the one that dies. A discrete card now holds a queue's
UNet at 512 only, from 12 GB, as before; a 1024 queue runs one at a
time, 4 to 5 s of loading a fill more. Unified memory keeps its
bounds.

## Giving the card back

Measured with a bench on the 5070 Ti, synthetic windows, the editor
closed: the process's own use on the card (`nvidia-smi` per process)
and the free figure.

| | process GiB | free GiB |
|---|---|---|
| alone: after a removal at 512, at 1024 | 0.00 | 11.8 |
| alone: a 512 removal, UNet held | 6.67 | 5.1 |
| alone: after `release_unet` | 0.00 | 11.9 |
| denoiser run once, kept | 3.86 | 7.9 |
| with it: a removal at 512 | 3.78 | 8.0 |
| with it: a removal at 1024 | 4.31 | 7.5 |
| with it: a 512 removal, UNet held | 7.75 | 4.0 |
| with it: after `release_unet` | 7.75 | 4.0 |
| with it: then a small denoiser run | 10.68 | 1.05 |
| every WebGPU session dropped | 0.00 | 11.5 |

ONNX Runtime's WebGPU provider keeps one context a process for every
session, and its storage buffers in a bucket cache: a released buffer
whose size has a bucket with room is kept for reuse as long as the
context lives, the rest freed at a later run's flush. Alone, the
remove's sessions are the context's last, and letting them go
destroys it and gives everything back. Beside a kept denoiser,
letting the UNet go gives nothing back, and the denoiser's next run
makes buffers of its own beside the remove's cached ones: 2.9 GiB
more, the second crash's path. A run of another model to flush the
cache does the opposite of what it is for.

So when a queue of fills ends, a queue of one included, the worker
drops every ONNX Runtime session it holds in each of its models (its
own and those kept for People, an export's other frames, and the
masks of frames not open): the remove's, LaMa's, the learned
denoiser's, and the masks' models'. It lets go of the UNet held
between removals and calls `malloc_trim(0)` (Linux with glibc; it
does nothing elsewhere). What the models made stays: SAM's
embeddings, the scenes, the mattes, the people, and the disk caches,
so only the sessions load again, at their next use; the denoiser's
tiling is kept with it, so a disk-cache hit after a drop loads
nothing. It happens on the worker, after the last fill's picture is
delivered, or when a queue is given up for another job, before that
job loads a model: never mid-walk, and never under the queue's lock
(the drop took 0.14 s and the trim 0.22 s, measured, and every send
from the window waits on that lock). It happens only once a fill's
model has run, or a UNet was opened ahead for a fill since undone, so
other jobs pay nothing. An export frame whose fills ran a model gives
the card back before the next frame. One info line says it: "the
card's N GiB given back".

Not on unified memory, where the free figure is not read: a Mac's
UNet takes tens of seconds to load and stays across queues as before,
and SAM and Subject were not measured there. Not while a merge runs:
ONNX Runtime's WebGPU context and its buffer cache have no lock of
their own (in its 1.23 source, only the factory that hands contexts
out has one), so a session dropped on the worker while the merge's
denoiser runs on its own thread would put buffers back into the cache
that run takes from. The drop waits for the merge to end: a merge
holds a shared lock for its life, and a give-back takes the lock
exclusively, or gives nothing back, and holds it across the drop. A worker
develop running beside a merge shares the context the same way; that
was so before this note.

After a removal at 1024 beside a kept denoiser, the drop took the
card from 7.62 GiB free to 11.80, and the host's resident memory from
5.29 GiB to 0.32. Each model cold after a drop:

| | load, s | first run, s |
|---|---|---|
| denoiser, best tier, tile 1024, 2048 mosaic | 0.12 | 0.46 (second 0.46) |
| Sky | 0.60 | 0.08 |
| Subject | 3.58 | 0.18 |
| SAM | 1.43 | 0.06 |

The denoiser comes back in 0.6 s, so the drop stands after every
queue, not only after one that held the UNet or ran at 1024. Subject's
and SAM's loads are paid only by a mask not kept, at its next use.


## Limits

- The figure is a snapshot. Another process can take the memory
  between the check and the run, or during the run's 11 to 36 s; the
  check narrows the window for the crash, it does not close it.
- The peaks are one card's, on WebGPU; Dawn on another driver, or
  DX12, may ask more or less. The masks' figure is inferred.
- The free figure counts ONNX Runtime's own cache as used, so a run
  the process could serve from its cache may be refused.
- A merge on its own thread holds a denoiser of its own, which the
  worker's drop does not reach; while it runs the context stays, and
  nothing is given back.
- The fills asked for again after a give-back are asked once; still
  short, they wait for the next develop.
- Reload times are one card's, the models' files in the page cache.
