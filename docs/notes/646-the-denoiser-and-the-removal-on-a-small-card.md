# §646. The denoiser and the removal on a small card (2026-11-12)

**What was wrong.**

On a 4 GB GTX 1650 (Windows, Vulkan) the card had 3.18 GiB free before the
first develop and 1.95 after it: our own sharpen and local contrast working
sets took 1.2 GiB. The learned denoiser at its tile of 512 (§431) wants
1.95 GiB with the margin, saw 1.8, gave its own sessions back, was still
short, and the engine's own denoise stood in. A removal at 512 wants 6.3 GiB
and at 1024 7.1, more than the whole card; it was refused, the fill left as
it was, with a long line on the status bar.

**Decided.**

The denoiser and a removal, short after their own give-back:
1. The engine lets go of what a develop makes again (the sharpen's and local
   contrast's working sets), as a removal did after its ask since §617, and
   asks again. This now comes before an export's waits: waiting on another
   program while we hold 1.2 GiB of our own made each frame of a set sleep
   its 10 s, since each frame's walk makes the sets again. A short the
   release cures is no short met, so no frame waits; one it does not cure
   waits once a set (below). The window's order is unchanged (it never
   waits). The window's Open and Develop jobs and every export frame set
   the release for their walk; a merge has none to give.
2. For the denoiser, the largest smaller tile that fits, of 512 (from a
   tiling of 1024), 384, and 256, taken before any wait; an export waits
   only when even the smallest is short, at the smallest, and then runs at
   the largest that fits. Measured with `denoise_tiles` on the 5070 Ti, a 24 MP CR3, best
   tier, one tile a process: peak over the loaded tier 1.47, 0.58, and
   0.49 GiB at 512, 384, and 256; 4.7-5.4, 5.2-6.2, and 7.0-8.0 s. At 384
   and 256 the answer was 512's bitwise (the margin of 96 holds the
   network's reach), so a short card shows the same picture, slower.
3. Short at every tile: in the window the engine's own denoise stands in, in
   the same words; an export of the frame is not written, as on master,
   since it would not be the picture the edit asks for.
The disk cache keys the answer by the tiling it ran at; the lookup tries the
tier's tiling and then each smaller one, so an answer a short card made at
384 is read back, not run again.

The remove: a card whose whole memory, read at start, is under the remove's
smallest need (6.3 GiB, `remove_needs(512, false)`) never runs it. Fast and
Best are greyed as on a machine without WebGPU, the tooltip "Needs 6.3 GiB
VRAM, 4.0 available. Use Patch." with the card's figure; a new Fill is Patch;
a Fill already marked Fast or Best is refused at once (`NoFill::Card`, by
`FillHaves::why_not`), before its model is asked for (so a missing remove is
not offered) and before any give-back or release, and said with the card's
total. A card big enough but short now is refused as on master, after the
engine let go and the worker's give-back round; the status line leads with
the same sentence and the free figure, and the long reason goes to the log.
Every develop of the frame works it out afresh (`card_shorts`), one that
starts from a kept point past the retouch or from the shelf too: of the
Fill removals the picture shown left as they were (the walk reports the
kept retouch's `unmade`), one the card can never hold is said with the
card's whole, and one whose refusal still holds (`refused`, which lapses by
its own rule) with that refusal's free figure, unless it is being asked
again this round. A fill held, deleted, made Patch, or in the making says
nothing. Three rounds of a remembered sentence each found a new way for it
to go missing or stale (a reopen from the shelf, a fill shown again from
memory); `refused` is the only memory left. A removal the card can never
hold is not counted as a fill to make, so a slider past the retouch is
served from the kept point. An export on a card too small is not written; on a busy
card it lets go, waits, and is written if the card frees up, and only a
short that lasts is not written. Either failure says the sentence in the
set's finished line (the queue list's status) and logs the reason. Unified
memory and Macs keep §421's bounds: `card_memory` is none there (and on a
CPU adapter), so nothing is greyed. The window reads the card's figure
only once the adapter has been probed elsewhere (the models probe, whose
answer refreshes the panel), so its thread never waits on the probe; not
known is not short. The worker reads it probing, so a launch's first
removal, or a headless export's, is said for the card it is. Exports take the card release as the
window's jobs do, for the denoiser and a removal (an export's masks go to
the CPU as on master): the ops run between the walk's stages, so letting go of
their sets changes no pixel.

A set short of the card waits once, not once a frame: a short the walk met
after its waits (`hit_short`; a smaller tile that fits is taken before any
wait and meets nothing) has the set's later frames ask once; one that failed (`short_failed`: a refusal, or a frame tried
again) also sends a mask to the CPU at once and bars another retry.

**Built and cut.**

We first built two more rungs: the denoiser on the CPU as a last resort
(26.6 s at 512 on the 24 MP frame), and LaMa standing in for a removal the
card could not hold. Both are gone. The LaMa stand-in broke "an export is
the picture the window showed" several ways (a fill made by one engine in
the window and another in an export, a canceled export writing it, a kept
fill outliving the card's shortage) and hid from the user that a model they
had not chosen made their fill. The CPU denoise changed the picture twice
(the engine's denoise first, the learned one half a minute later) and held
edits with nothing on screen to say why. The user picks Patch; the window
says why.

**Tests.**

The ladder (release, each smaller tile, refusal in the old words, a canceled
export letting nothing go), an export's smaller tile met once, the cache's
smaller-tile lookup, the run tiling, `FillHaves` greying and the Patch
default, the panel's greyed buttons and tooltip (`set_test_card`), a
Fast-marked removal on a short card failing with its figures, the status
line's sentence once and again on a second develop, a removal past the whole
card refused at once and before a missing model (no download offered), a set
short by the engine's own sets waiting not at all (in the worker, the sets
made again each frame, and at the denoiser and a removal), and a refused
set's waits counted across its frames (2, 2, 2).
