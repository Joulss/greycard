# §573. Merge to Focus stack

*2026-11-05*

The focus stack joins the HDR merge and the panorama on the frame
menu, and moves out of the command line into the engine's merge
module, `merge::focus`, so `greycard stack` and the window run one
implementation.

**What moved.** The frames' read, their short develop, the core's
`stack_camera`, and the DNG's write. `focus::frames` decodes each
frame, develops it the way `register` does (bilinear, hot pixels
repaired, no CA, highlights clipped) and stops at the demosaic, at
camera RGB with the gains divided back out, then merges. Each frame's
samples are let go once it is demosaiced; the CLI used to hold every
raw to the end for the reference's tags. `focus::stack` holds the
name, merges, writes under the hidden `.<name>.merging` folder, and
renames at the end, as every merge does: a stack cut short leaves no
half file, and a frame is never written over. The CLI's DNG goes
through it too, so `-o` naming one of the frames is refused under any
`--on-exists`, where before it truncated the frame. The CLI keeps its
options, its report, and its TIFF, which it writes itself from
`focus::frames`.

**The memory.** The HDR merge streams its frames into sums and the
panorama spills them to its folder; the stack holds every frame at
once, and stacks of 15 to 50 frames are usual. A stack the machine's
memory will not hold would take the editor down with it, so it is
refused, from the first frame's size before a second is read: frames
times pixels times 40 bytes (the demosaiced frame's 12, and the core's
pyramids, sharpness maps, and output over it) against the machine's
physical memory (`greycard_ai::ram::physical`), in the failure line.
Physical, not available, memory: that is the reader we have.

**Not the walk.** The HDR merge and the panorama read their frames
through the stage list; the stack keeps the clipped prepare and plain
bilinear the CLI has always used. Moving it onto the walk would change
every stack the CLI writes, a decision of its own.

**The action.** Merge to Focus stack in the frame menu after Merge to
Panorama, offered with more than one frame chosen; the word is one
string in `frame-menu.slint`. The same job, queue, thread, and
landing as the other merges: each frame said as it is read, then
"placing the frames" while the core registers and merges, then the
write; a failure says "not merged:" and why. The window asks for
nothing and takes the CLI's defaults: the middle frame as reference,
the core's selectivity and residual, lossless samples, and a name
there already numbered. The file is `<first> stack.dng` beside the
first frame, as the CLI names it, given to the indexer and put on top
of a stack with its sources as an HDR merge is. Its edit is the
reference's, found after the merge (the stack picks its own
reference), less the learned denoiser; its exposure stays, since a
stack has no headroom and sits at the reference's brightness. A frame
the stack leaves out is said in the log.

**No merge again.** The stack writes no tag of ours and no recipe, so
"Denoise the frames and merge again" is not offered on it. It wants a
stack geometry in the core's `Recipe` and the frames walked with the
learned denoiser, which waits on the walk above.

## Left

- The stack's frames through the walk, and with it the learned
  denoiser per frame and merge again.
- A `NoiseProfile` on the stack's file: the stack writes none, where
  the HDR merge does.
- Progress inside the core's merge: the status line sits on "placing"
  for the whole of it.
- The memory check against available rather than physical memory.
