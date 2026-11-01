# §479. The editor timed on an M5

*2026-10-31*

Three roadmap items waited on a Mac's numbers: the first develop, for
whether a proxy develop is worth showing (§90, §134); the GPU's memory
on unified memory (§259, §269); and, with them, how the GPU path feels
there. Measured on an M5 with 24 GB (macOS 27.2) in the release
editor, the window on screen, from `-v` logs, `/usr/bin/time -l`'s
peak memory footprint, which on Apple silicon counts the GPU's
allocations, and `sample`. The frames to hand were a 24 MP Canon EOS R6
Mark II CR3 and a 102 MP Fujifilm GFX 100S II RAF with no edit; there
was no 45 MP one, so the 45 MP figures elsewhere are not repeated.

## The first develop

The GFX frame at its first open, `--snapshot`, three runs:

| 102 MP, first open | |
|---|---|
| decode | 0.25 to 0.36 s |
| CA on the GPU | 0.38 to 0.42 s |
| prepare | 0.48 to 0.58 s |
| demosaic | 2.75 to 3.12 s |
| lens on the GPU | 0.29 to 0.32 s |
| finish, guide, upload | 0.31 s |
| develop in all | 6.5 to 7.0 s |

The 24 MP CR3's parts came to about 1.2 s, its demosaic 0.76 s (that
frame carried two fills, whose 88 s is §477's). So the camera's JPEG
stands in for about a second at 24 MP and for seven at 102 MP. The
demosaic is the largest part at both sizes, as §378 found at 45 MP on
eight threads.

The library's one thumbnail of that folder came 10.4 s after the
folder opened, with 0.04 s of decoding: it waited behind the develop.

## A slider move

`--time-clarity` and `--time-dehaze`, ten moves each on the GFX frame:

| 102 MP | move to frame | peak footprint |
|---|---|---|
| opened only | | 9.55 GB |
| Clarity | 3.95 s mean (3.88 to 4.38) | 12.09 GB |
| Dehaze | 2.40 s mean (2.32 to 2.58) | 11.95 GB |

§259 measured a Clarity move at 59 ms at 24 MP and 118 ms at 45 MP on
the GPU it was built on; scaled to 102 MP, that is about 270 ms. The
M5 takes fifteen times that. A symbolized build sampled through the
moves puts the whole of the worker's time in `greycard_gpu`'s
`sharpen`, waiting on Metal: half in `sharpen_into`'s wait after its
submissions, half in `tile_indices`, whose `read_buffer` waits for the
queue to drain mid-pipeline. The debug log has the sharpen's 20
iterations in 10 submissions, the longest about 0.3 s (once 1.3 s), so
some 3 s of the 3.9 is the capture sharpen run again after every
Clarity move, since it comes after the local contrast. The local
contrast's own share is the smaller one.

## The GPU's memory

The local contrast's planes are already made for each run on unified
memory and kept only on a discrete GPU (`Context`'s
`keep = DiscreteGpu`), which is what the roadmap's item proposed if
memory pressed; the log says "the local contrast's planes made per
run" on the M5. A Clarity move at 102 MP takes the footprint from 9.55
to 12.09 GB, a Dehaze move to 11.95. On a 24 GB machine that is room
to spare. An 8 GB Mac was not to hand: at 45 MP the pictures are 44
percent of these, and the footprint carries fixed costs beside them,
so what presses there is still to measure on one.

## What follows

- The sharpen on every move is the Mac's slow part at large sizes: a
  move waits for 20 iterations over 102 MP. Running it at the
  viewport's scale while a slider is held, or holding the
  `tile_indices` readback off the path of a move, are the two places
  to look; neither is decided.
- Whether a proxy develop earns its place is a judgment on those seven
  seconds behind the camera's JPEG at 102 MP, and one second at 24.

## What was not measured

- 45 MP, and an 8 GB Mac.
- When the camera's JPEG reached the screen: the log does not say, and
  a snapshot waits for the develop.
- A run with the window hidden: the editor draws nothing then, so a
  `--snapshot` waits for good. The develop finishes and the main thread
  sits in AppKit's event loop with no redraw asked for.
