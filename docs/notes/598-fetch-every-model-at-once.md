# §598. Fetch every model at once (2026-11-07)

*2026-11-07*

A user asked, and a review named it: the models come one at a time as
each feature is first used, each behind its own sheet, and someone
setting up before a shoot, or before going offline, wants them all
now. So the Settings sheet gains a Models category: Fetch all at its
top with what is left to fetch beside it, then a line a model
from the registry, its license's name, its size, and where it stands
(in the store, fetching, queued, failed, or skipped with why). Each
wrapping text has its own width, as the keys' rows do, so the
category's height counts a wrapped line and its foot stays in reach. The first picture
opened offers the same once, on the model sheet, Now or As needed, As
needed the default (Escape and a click outside too); the answer is
kept in the settings and never asked again, existing users included.
Nothing is fetched until one of the two is pressed. Fetch all shows
no license sheet per model: each line names its license, the offer's
note names them all, and each full text is written beside its model
in the store, as a single fetch writes it.

What this machine runs is the engine's to say, in the words its own
refusals use, so we wrote no second rule. `ai::Here` reads whether
ONNX Runtime can be had (and the CPU-only runtime that would bring
it), whether the models have WebGPU, whether its shaders take 16-bit
floats, and the card's memory; its `why_not` gives the remove's two
models `FillHaves::why_not`'s answer, the fill's 16-bit refusal (§570,
now a function both share), and the memory refusal `Ai::room` makes
past the whole card, which a 4 or 6 GB card meets at the smallest
removal (6.8 GB with its margin), though only where `room` can
refuse at all, which is where the card's free memory can be read:
shared memory and an integrated GPU's firmware carve-out are no
reason; and it asks `subject::pick` which of the two Subject files
runs here, the other skipped as the copy
this machine does not use. A model already in the store is had
whatever this machine runs. The reason has two forms: the refusal's
own words for the log, a few for a line, with no card named.

`Here::now` probes the WebGPU adapter, a second wgpu instance through
pollster that a slow driver can hold for seconds, so it runs on a
thread of its own, started at launch where there is a store, and the
lines say "checking this machine…" until it is back; the offer waits
for it and is made when it lands, if a picture is up. When the CPU
runtime arrives, or would not load, it is asked again, and an answer
from before that is dropped. With no store nothing is probed.

Where the AI features are off for want of the CPU-only runtime, Fetch
all fetches that first, through the runtime's own fetch, and the
models after it; the lines say "after the runtime" until then. If it
fails, the models it held say so on their lines.

The fetch is the model sheet's own, one at a time: we queue the
runnable set and start the next as any fetch lands, ours or another's
(a model's, the runtime's, the lens profiles'), and hold while the
sheet is up or another fetch is coming, so a Download pressed
meanwhile never fetches a model twice; that Download's line says
"fetching" too. A failure is said on its line, in brief, and the rest
go on; pressing Fetch all again tries it once more. The button is
greyed while anything is coming. `Outcome::Fetching` now names the
file, so a line ticks by the whole model rather than starting over at
each file. There is no stopping a fetch, here or on the sheet; the
queue is left to finish, and a large file cut off by a quit resumes
from its part at the next fetch.

On a Mac, a fetch of every model warms Core ML too: after Sky or a
denoiser tier lands, its thread fetches the tier's fp16 file and
loads the model once, so Core ML's compile is in the cache before the
first mask or denoise asks for it. The warms run one at a time behind
a lock, and none runs for a model Core ML has loaded in this process
already, the worker's or a warm before. The sizes shown on a Mac
count the fp16 files. The other models stay off Core ML and have
nothing to warm. Untested on a Mac in this change.

The registry adds up to 7.7 GB today, not the 3.5 GB first guessed:
the remove's UNet alone is 5.5 GB. A machine without WebGPU fetches
1.7 GB.
