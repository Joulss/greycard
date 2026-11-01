# §480. Workgroup memory left unzeroed

*2026-10-31*

§479 timed a Clarity move at 3.9 s on a 102 MP frame on an M5 and put
most of it on the sharpen. The GPU crate's bench (`examples/bench.rs`)
on the same frame said otherwise, and said the GPU path lost to the
CPU outright on this machine:

| 102 MP, M5 | CPU | GPU before |
|---|---|---|
| local contrast, Texture and Clarity at 0.5 | 0.37 s | 4.26 s |
| sharpen, 20 iterations | 1.58 s | 2.25 s |

At 24 MP the local contrast was 0.98 s on the GPU against 0.089 s on
the CPU, where §259 measured a whole Clarity move, sharpen included,
at 59 ms on the GPU it was built on. Keeping the planes between runs
changed nothing, so it was not allocation; Texture alone, at a radius
of 3, took 0.63 s, so it was not the windows' length. Plain copies of
an R32Float plane, sampled to write-only storage or read-write to
read-write, ran twelve full-size dispatches in 6 ms.

The box kernels alone, copied verbatim into a test, found it. The row
pass took 4.6 ms at 24 MP and the column pass 145 ms, at any radius
from 3 to 102. wgpu zeroes a kernel's `var<workgroup>` memory at the
start of every workgroup unless the pipeline says otherwise
(`PipelineCompilationOptions::zero_initialize_workgroup_memory`, on by
default). The column pass's array is 2304 floats, and at 24 MP it runs
94,000 workgroups; with the clearing off it took 3.5 ms.

## The change

`plumbing::pipeline`, through which every compute pipeline in the
crate is made, turns the clearing off. That is safe only where each
kernel writes every element of its workgroup memory before any
invocation reads it, so each of the twelve arrays in the five shaders
was read for it:

- `local_contrast.wgsl`: `box_rows` loads its whole window into `line`
  and `box_cols` into `column` before the barrier, and each reads only
  inside what it loaded.
- `ca.wgsl`: `vote`'s `contrib` is written for every site of a chunk
  before the barrier and read only for those sites.
- `dehaze.wgsl`: all 256 invocations store `wg_sum` and `wg_min`
  before the reduction, which reads under 256.
- `sharpen_tiles.wgsl`: `region` and `rowblur` are filled across
  their whole range before each barrier and read inside it; the
  reductions over `scratch` have every invocation store its slot;
  `wg_flag` is set by one invocation ahead of `workgroupUniformLoad`,
  which carries its own barrier.
- `sharpen_planes.wgsl`: every invocation stores its `scratch` slot
  before each reduction.

The rule is written at the helper for the next kernel. The GPU
crate's tests, which hold each op to the CPU reference, pass, and the
bench's differences from the CPU are the same to the last digit
before and after, as they must be when no kernel read what the
clearing wrote.

## Measured

| on an M5 | before | after |
|---|---|---|
| local contrast, 24 MP | 0.98 s | 0.078 s |
| local contrast, 102 MP | 4.26 s | 0.47 s |
| sharpen, 24 MP | 0.52 s | 0.45 s |
| sharpen, 102 MP | 2.25 s | 1.93 s |
| Clarity move to frame, 102 MP, editor | 3.95 s | 2.19 s |
| Dehaze develop, 102 MP, editor | 2.32 to 2.58 s | 1.99 to 2.19 s |

Two Dehaze moves in that run waited 38 s and 196 s for their frame
after a develop of 2.2 s: the window was not drawn meanwhile, as
§479 found the editor does while it is hidden, so those two are the
window's and not the develop's.

The local contrast is now the CPU's speed on this machine, at 102 MP
and at 24. The clearing costs a discrete card or Vulkan little, which
zeroes through an extension, so the change was not measured there; it
can only save time.

## What is left

A move at 102 MP is now the sharpen: twenty iterations in eight
submissions of about 280 ms. Against §259's 90 ms at 45 MP on its
GPU, that is about the ratio of the two GPUs' arithmetic, so it reads
as the M5's speed and not another fault; the M5's CPU runs it in
1.6 s. Getting a move under a second at that size means sharpening
less per move: at the viewport's scale while a slider is held, or
only what is on screen, the export unchanged. That is a choice about
what the viewport shows during a move (§90's fit view is the export
at rest) and is not made here.

The bench has two things this did not explain: the first local
contrast at 102 MP takes 5.6 s against 0.47 s after, and the sharpen
on the local contrast's output 3.8 s against 1.9 s on the upload.
