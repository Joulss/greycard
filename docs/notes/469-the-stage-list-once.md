# §469. The stage list, once

*2026-10-31*

The design pass §454 asked for before part 2: the type of the list
of stages a develop walks, derived once from the edit and the
purpose, compared for caching, placed for the GPU, and walked by one
function. The draft that led here is in the captures; this is what
we decided.

**The purpose.** All that varies between the callers is one enum:
Viewport (GPU where lent, ends at the shader), Export with its
settings (the CPU reference, ends at the file), Reduced (binned,
masks from the cache, 2048 sRGB), ModelsSee (Reduced cut at the
base, no retouch), and Prefetch (ends at the base, for the shelf).

**The steps.** A `Vec<Step>`, each an `Op(Stage)`, a `Tap`, or a
`Keep`. A stage is one transform with its options resolved from the
edit, so lists compare by `==`: Prepare, Ca, Demosaic or Bin(n) or
Learned, Denoise, IntoWorking, Blend, Orient and Reduce for a
picture input, Lens, Defringe, Retouch with its fills run or
refused, LocalContrast, Dehaze, Sharpen, and Finish. A tap is a
product read off the picture where it stands, not a transform: the
guide plane, the detail planes, the masks' rasters. A keep marks
where a walk leaves what it has for the next to start from: the
pair, the base, the patched picture, the picture before the
sharpen, the developed one. Two functions derive it: `needs` before
the decode (models, look) and `stages` after it, from the edit, the
purpose, the decode's facts (input kind, lens identity, turn), and
the lens database; two to keep in step, in return for lists that
compare resolved. A stage the purpose leaves out of what the edit
asks goes in `skipped` with its reason, and a list with any is kept
under the reduced recipe.

**The name.** Core's pre-demosaic stage is `prepare` and the denoise
cache keys on `Prepared`, so the post-demosaic stage §454 called
prepare is `IntoWorking`, for what it does; §454 reads so from here.

**Caching as data.** At each keep passed, the walk stores the
buffer, its meta, the taps so far, the steps up to there, and where
each ran. A walk starts from the last kept point whose prefix equals
its own and whose places its plan allows. That one rule retires the
four hand-kept predicates on the edit (`same_learned`, `same_base`,
`same_patched`, `same_develop`), the GPU flags on the base and the
last result, and the export's redevelop test: an export's plan is
all CPU, so a base the device touched never matches. It rekeys
itself when a lens database arrives mid-session. The cost is
`PartialEq` on the lens correction and pointer equality for a DCP.
The shelf holds a kept base by file key, as now.

**The GPU as data.** A stage says whether it has a device version
(CA, the lens when it moves, local contrast, dehaze, the sharpen),
and a plan places each step: all CPU for Export, Reduced, ModelsSee,
and the command line; GPU where possible for Viewport and Prefetch.
Contiguous GPU steps are one run: one upload at its head, a device
buffer between, and a run ending at the shader's finish writes the
viewport texture. The failure rules live once in the walker: an
unsupported stage reruns its run on the CPU from the run's input,
context kept; any other error drops the context and replans the rest
on the CPU. Runs keep today's grain; per-stage declines would cost a
download and an upload nothing measured asks for. The prefetch's
give-way is asked between steps for every walk. A kept prefix that
differs only in the orientation, with nothing after it that does not
commute with a turn, is permuted rather than redeveloped.

**The reduced develop.** The same list under Reduced: Bin(n) for the
demosaic, the defringe scaled by 1/n, no sharpen, no denoisers, the
fills refused, masks from the cache only, the planes inline, each
skip named. ModelsSee is that list cut at the base. The reduced
develop, the masks' pictures, and the person test each become a walk
of the list; there is no second pipeline.

**One finish.** The viewport and an export end at the same step; only
its place differs. The shader's finish takes what the walker yields,
one `FinishInput` where today the outcome and the edit's finish each
carry half; the CPU finish is the render crate's, then the border and
the mark. The parity tests that hold the shader to the CPU finish now
hold it to the stage. Core's `finish` loses the dehaze and the
sharpen, their stats, and their settings; it keeps the matrix, the
DCP map, and the orientation. Core's develop becomes prepare,
demosaic, into-working, what every caller wanted.

**The command line.** `greycard export` is the one develop that
matches the editor, walking the list under Export with the sheet,
the presets, and the overrides read by one settings subset reader.
`greycard develop` stays a measurement tool with its own settings,
walking the list's front; the DNG's demosaic stays off the list. A
frame with no sidecar exported from the command line seeds its blend
from the ISO as the editor does. The camera thumbnail is the one
develop that is not an edit's and stays outside the list. Progress
words mid-walk go through a small trait the worker implements with
its delivery, so the command line owes the queue nothing. The
sharpen's blend mask rides as a field of the walk's result, for the
viewport alone.

**After part 2, as their own set.** Two choices change pixels and
would muddy part 2's bar of byte-equal exports. The models see the
base everywhere, where today the Library's reduced develop hands
them the picture after local contrast and dehaze and a frame with
strong Clarity can carry a mask made from a picture other than the
one it gets on open; the raster pipeline version rises once. And
the edit's kept pictures come from a CPU walk of the kept patched
picture on the keeping thread, where today a save during a GPU
viewport reads the texture back, within the parity tolerance but not
the reference; a few hundred milliseconds a save at 45 megapixels.
