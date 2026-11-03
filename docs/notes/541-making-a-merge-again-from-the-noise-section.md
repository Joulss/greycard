# §541. Making a merge again from the Noise section

*2026-11-03*

Part 2 of §532, after §537. On a merged file, the Noise section's
learned tiers give way to a row of their own: a tier picker over
**Denoise the frames and merge again**, which runs `merge::again` on
the merge queue's thread with the merges' status lines, done line, and
refusals. The row's tier is the window's, starting at the recipe's tier
or off, and is never written to the edit. A merge clears the learned
tier from its edit on purpose: kept there, every develop of the file
would say the denoiser needs a mosaic, the profiled Denoise would be
held off, and `greycard export` of it would fail. So one picker shows,
the row's, and the edit's tier, which steers nothing on a merge, is
hidden.

**When it shows.** The engine decides (`merge::can_denoise_again`).
The recipe has to read, each frame has to be beside the file and the
one merged, by its hash, and each has to be a 2x2 Bayer mosaic. The
worker asks after the frame's develop has been delivered, so the
heads it reads never hold up the picture. The button is hidden with
sidecars off and on the export queue's view. Asked for a tier on
frames it cannot denoise, `again` refuses before walking any frame,
where the walk would otherwise skip the denoiser and write the file
undenoised.

**The swap.** The file keeps its name, so its edit and sidecar stay.
What was keyed by the old file goes: its thumbnails and previews (by
head hash and time), its edit's pictures (by head hash and develop
key), the loupe's copies, and the open file's hash in the masks'
`Ai`. The stack reference takes the new hash, and the indexer is
given the file. The worker's shelf and the masks' disk slots key by
the file's stat, so they miss the new file without our help. An open
frame is opened again, and the done line goes in front of its
develop's line, or of the loupe's in culling. The worker's word on the
frames is matched to the open file by its path, not by the develop's
generation, which a slider moved during the first develop moves on.

## Left

- The rename over an open file is untried on Windows.
