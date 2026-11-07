# §600. The command line's export on the GPU (2026-11-07)

Roadmap line: "The command line's export under a device when the
machine has one, so it and the editor's export run the same demosaic,
CA, and lens on the GPU and match again; a flag forces the CPU."

Since §236 the headless export asked for no device, because the export
was the CPU reference and nothing in it would have used one. §591 put
the demosaic on the GPU for the viewport and the prefetch and left the
export on the CPU, so the command line still rendered what the editor
exported. That kept parity, but the editor's export of the open frame
could not reuse the viewport's base, whose CA and demosaic were the
device's, so it made the base again on the CPU: 0.6 s of AMaZE dual at
45 MP on 32 threads, over two seconds on a tester's eight.

## The rule

`walk::plan` is still the one place that decides. Every walk lent a
device runs on it the stages that take a picture in memory and hand
one back: the CA, the lens when it moves a pixel, and a demosaic with
a device version (`Stage::in_memory_on_device`). These are the
viewport's, the prefetch's, and now the export's. The runs that end at
the viewport's texture, which are the Detail section, the dehaze, and
the sharpen, stay with the viewport and the prefetch, along with the
shader. Their device versions write half floats for the screen, and
the export finishes in full floats on the CPU. Taking them over would
need a download path and a sharpen that writes floats, and is a
separate item. The reduced pictures, the models' pictures, and a
merge's frames stay on the CPU.

So the editor's export of the open frame resumes from the viewport's
base. The one walk on one device gives the editor's export and the
command line's export as the same file, which a test checks pixel for
pixel on a mosaic with a lateral CA (`headless::tests`). A set frame
that is not open runs on the worker's device the same way.

There is one place where the two can still differ. Suppose a device
error left the open frame with a base made on the CPU. That base still
serves an editor export of the frame, while a fresh command-line export
demosaics on the GPU. There the two agree only to the rounding that
§591 bounds.

An export walks under a hold of its own (`ExportHold`), as the
prefetch does. A device error on its frame, out of memory included,
runs that frame on the CPU and keeps the context. Without the hold,
one large file beside the open frame's kept device points could put
the viewport on the CPU until a restart. A lost device lets the
context go, as everywhere. After an export, the worker forgets the open
frame's device points if the walk dropped the context, as it does after
a prefetch. A panic that says the device was lost counts as a loss; the
frame it hit fails and the run exits 1.

The learned denoiser had a catch. When its model is missing, the walk
swaps it for the demosaic and placed the stand-in again from whether
the old plan held anything on the device. An export with no CA or lens
held nothing, so its stand-in would have run on the CPU. Now the plan
gives the denoiser's step the place its stand-in demosaic would take,
and the walk carries the plan over step for step. The denoiser itself
still runs on its own providers.

## The device

The engine is still lent a device and never makes one (§454). The
consumers make it: `greycard export`, and the editor's `--export`
alias, call `greycard_gpu::Context::for_export`, which makes a context
on a device of its own and watches that device for its loss. They pass
the result to `headless::run`, as a context or as the reason there is
none. The run hands the context to its worker whole (`set_context`),
so the pipelines are built once. Because the context is built first,
the first line is true when it is said.

The adapter is the one the window would take: the one
`WGPU_ADAPTER_NAME` names, else the one `WGPU_POWER_PREF` asks for,
high performance by default (§392). A software adapter is declined
unless it was asked for by hand. That means `WGPU_ADAPTER_NAME`, or a
`SLINT_WGPU_CPU` that no greycard set, which is the window's rule,
now one function (`greycard_gpu::asked_by_hand`) that both read. The
reason for declining is speed: lavapipe or WARP shaders run slower than
the CPU runs the same stages (§374). The engine's tests take lavapipe
under `GREYCARD_REQUIRE_GPU` in the tests' own code, so the production
rule has no test bypass.

`greycard export --cpu` lends no device, for a render that is the same
on every machine. The editor's alias takes its existing `--cpu-ops`
for this. The CPU reference itself stays where §591 left it, in the
tests. The run's first log line says where it develops: `export: on
NVIDIA GeForce RTX 5070 Ti`, or `export: on the CPU (--cpu)`, `(no
GPU)`, or `(software adapter declined)`.

The cost is the device's startup, about the adapter, the device, and
the pipelines (AMaZE's built on first use), paid once per run. A run
of one small frame on a machine whose CPU is quick may come out no
faster. Nothing new reaches the UI.
