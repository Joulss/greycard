# §537. The recipe kept and the merge made again

*2026-11-02*

Part 1 of §532: the engine, the core, and the command line. The
window's row in the Noise section is part 2.

**The tag.** `merge::Tag` (now in core's `merge/tag.rs`) keeps its
first facts as they were, so the develop's `Facts` reads a merge as
before, and adds three things. After each `frame=` line a `head=`: the
library's content hash (BLAKE3 over the first 64 KB and the size,
§72), so a renamed frame or another file under its name is told from
the one merged. Two frames can share that hash, since uncompressed raws
of one size from one burst may differ only past the head; those frames
also keep a `whole=`, the whole file's BLAKE3 (the index's
`whole_hash`, §216), which the check then reads too. Then the options:
`ca=` (`once`, `per-frame`, `off`), `denoise=` (a tier or `none`), and
for a bracket given its noise, `noise=`. The tier kept is the one that
ran: a tier asked of a merge that stood down for want of its model is
not kept. Then the geometry. A bracket's is each frame's
`exposure=<k> <e>` and, for a frame resampled, `shift=<k>` with its six
affine numbers. A panorama's is `anchor=`, `kernel=`, `projection=`
(`planar`, `cylinder-down`, `cylinder-across`, or `homographies` for a
planar model), `focal=`, a `turn=<k>` a frame, each `correction=` of
the lens as it ran, a `gain=<k>` a frame, and `canvas=` and `crop=`.
Every new fact is optional: a file written before reads with no
recipe. A recipe short of a fact, or with one that does not read, is
dropped whole while the rest of the tag stands, and the tag says which
fact (`Tag::unread`), so a merge made again tells a spoiled recipe
from an old file.

**Why those numbers.** A rotation is a unit quaternion, `w` not
negative: four numbers with one constraint, where three angles bring
an axis order to agree on and a lock where two axes line up, and nine
matrix entries say six things twice. The leveling and the centering
are in the rotations as placed, so they need no fact of their own.
Every number is written in Rust's shortest form that reads back to the
same `f64` (or `f32`), so the text is exact. The quaternion, not the
matrix, is the truth: a matrix through its quaternion and back moves
in its last bit, and again the next time, so a merge places its frames
by the rotations made from the quaternions it writes
(`Placed::model`), and a merge made from the tag gets the same
matrices. The lens is kept too: its distortion is the first term of
each frame's warp, and a lens database updated since, or a Lens section
changed, would otherwise move the frames.

**The anchor.** The frame walked first gives every frame its white and
its CA fit. It is the frame asked, or the middle of the list, as
before. The reference, whose view and exposure scale the panorama is
in and whose EXIF and edit the file takes, is settled after the fit
(below). They are two facts because a merge made again must walk from
the same anchor to get the same white and CA.

**The merge made again.** `merge::again(file, overrides, edit, models,
progress)` reads the tag and finds each frame beside the file by name.
It refuses, saying which: a file with no tag of ours; one written
before the recipe ("this merge was written before its recipe was kept;
merge the frames again"); one whose recipe does not read ("the recipe
in NAME cannot be read: FACT"); a frame named with a path in it; a
frame missing; a frame whose hash has changed; and a learned tier this
machine cannot run, before any frame is walked, since a re-merge in
place without the tier asked would put a plain merge where a denoised
one was. Then the merge runs with the recipe's options as overridden
(today the denoise tier) and its geometry given
(`Options::registration` for a bracket, `Options::geometry` for a
panorama). A bracket takes its exposures and moves and neither
measures nor fits. A panorama walks and spills each frame (the
vignetting from the kept lens), skips the corners, the matches, the
fit, the leveling, the gains, and the largest rectangle, reads no lens
database, and lays the frames on the kept canvas: its seams are found
again on the reduced copies and the frames resampled and blended into
the kept crop. A planar one says it is laid on a plane "as its recipe
says", having read no database to say why. The noise profile and the
headroom are measured again as a merge measures them. The file is
written under the merge's reserved folder (§530) and renamed over the
old one, so its name and its sidecar are untouched; `--out` writes
elsewhere. The edit the frames are walked under is the caller's, as
for a merge: the command line's default edit, as `merge hdr` and
`merge pano` use, and the window the reference's.

**The reference.** With the pairs fitted and no `--reference`, the
reference is the frame whose rotation is nearest the mean of them all
(the largest trace against their sum, the chordal mean), or for a
planar model the frame whose middle lands nearest the mean of the
middles; the frames are turned onto it (each rotation times the
transpose of its, or the inverse homography composed in), which
changes no pair's fit. The gains are on its scale and the file takes
its EXIF. `pano::reference_of` still answers the asked case and names
the anchor. On a synthetic set shot right to left in uneven steps
(yaws 0.5, 0.3, 0, -0.3 rad) the list's middle is the second frame and
the reference the third.

In the window one frame both walks the panorama and gives it its edit.
The walk needs an edit before the reference is known, so the window
walks under the anchor's. When the reference turns out to be another
frame and the two edits walk differently (`pano::walks_alike`: the
demosaic, the camera profile, the Lens section, the learned tier), the
panorama is merged again with that frame asked as the reference, under
its edit, and written over the first. Edits that walk alike, the
usual case, cost nothing more, and the file takes the reference's
edit either way.

## Measured

The three frames of §530 (24 MP, 16 mm, handheld), release build, four
threads, the command line with the lens database.

The reference by geometry is the middle frame, the list's middle too:
the panorama with it and with `--reference 1` are the same file to
the sample (0 of 51,801,600 samples differ) and export the same (0 of
17,267,200 pixels).

| run | total, s | walk, s | features and pairs, s | peak, GB |
|---|---|---|---|---|
| merge pano | 15.4 | 5.5 | 3.3 | 2.20 |
| merge again | 11.2 | 5.3 | 0.05 | 2.20 |
| merge again, in place | 11.1 | 5.3 | 0.04 | 2.21 |
| merge again --denoise balanced | 17.4 | 11.0 | 0.04 | 2.50 |

The re-merge saves the matching and the fit, 4 s, not the half §532
guessed: the walk, the blend (4.0 s), and the write (1.2 s) are most of
a merge and all of a re-merge.

Made again with nothing changed, to another name and in place, the
file is the merge's to the sample: 0 of 51,801,600 samples differ,
and the two exports differ in 0 of 17,267,200 pixels.

Made again with the balanced tier, the frames are where they were: a
translation fit of the denoised merge onto the plain one, on the
reference's scale, finds 0.0011 px (0.0023 px on the exports; the
same fit finds 0.9995 px when one is moved a pixel). The difference is
noise: on the reference's scale it is 7.4e-4 RMS a pixel, 2.7e-4 over
4x4 blocks, and 8.3e-5 over 16x16, against a mean of 0.071, the block
means equal to 1e-5. The file's noise profile says a sixth less shot
noise on the reference's scale (S 1.05e-4 against 1.24e-4). One thing
moved that is not placement: the headroom went from 1.37 to 1.59
stops, since the brightest white-balanced sample, in a clipped
highlight, comes out brighter from the denoiser than from the
demosaic. The develop brightens by the headroom, so the file opens as
bright; the exports' means agree within 0.1%.

## Left

- Part 2: the Noise section's row.
- The edit a merge walks under (demosaic, camera profile, Lens section)
  is not in the recipe. The command line walks under the default edit
  and the window under the reference's, so a window merge made again
  from the command line with an edit off the defaults walks
  differently; the placement holds, the lens being the recipe's.
- A collection's membership tick matches a frame by its content hash
  (ui `panel/collections.rs`), and a merge made again has a new one: a
  collection holding the merged file misses it until it is added
  again.
- The denoiser's brighter highlights: a headroom 0.2 stops higher on
  these frames.
- The window's second merge, when the turn's middle is not the list's
  and the edits walk apart: if it fails, the first file stays with no
  look and no stack, and on Windows the second rename can fail while
  something holds the first file. Writing the first under the
  `.merging` folder would close both.
