# §532. The merge recipe and the re-merge

*2026-11-02*

A merged file is linear RGB with no mosaic, so the learned denoiser,
which takes a mosaic, stands down on it (§523): the only place it can
act on a bracket or a panorama is on each frame before the merge. Today
the window carries it into the merge when the reference frame's edit
holds a tier at merge time, and the command line never does. A frame
straight off the card is merged undenoised, and once the file is
written the chance is gone. We make it come back.

**The recipe.** The file's tag (`merge::Tag`, in `DNGPrivateData`)
already names the frames, the reference, whether they were denoised,
the headroom, and the lens. It gains what made the picture: the
options (the CA fit, the kernel, the denoise tier), and the geometry
the fit found (each frame's rotation, the focal length, the
projection and the leveling, the gains, the canvas, and the crop). A
bracket's geometry is its registration. A hash of each frame's head
tells a frame from a rename.

**The re-merge.** A merged file whose frames are still beside it can
be made again from its recipe. The geometry is taken from the tag, not
fitted, so the re-merge walks, resamples, and blends only: about half
the time of a panorama (8 s against 15 on three 24 MP frames), and the
placement is the old one to the pixel. That is the point of keeping
it: denoised frames would move a fresh fit by a fraction of a pixel,
and every mask and crop drawn on the merge would drift. The new file
is written beside the old one through the merge queue, with its status
line, and swaps in with one rename. The edit and its sidecar are
untouched, since the file's name and geometry are the same.

**The control.** The learned tier on a merged file is a row of its own
in the Noise section, offered only when the file carries a recipe and
its frames are present: it names the tier and runs the re-merge. The
existing tier control is left alone, since a slider that rewrites a
file on disk and takes ten seconds would be unlike every other one.
The wording is the user's. From the command line, `greycard merge
again <file> --denoise <tier>`, which also takes any other option in
the recipe. When a frame is missing or renamed, the row says which,
and the tier stays off.

**The reference.** The "middle" frame is the middle of the list as
given, in the strip's order from the window; a sweep shot right to
left, or with a frame reshot, puts an end frame there. With the pairs
fitted, the reference can be the frame whose rotation is nearest the
mean of them all: the middle of the stitch however it was shot. The
frames are turned onto it after the fit, which the fit allows.
`--reference` still overrides.

**The default.** The merge takes the tier from the reference's edit
as now. At base ISO the Balanced tier took a quarter off the noise
variance of three frames for 4 s more (§530); a handheld night
panorama at a high ISO is where it matters, and that test decides
whether the merge turns it on by itself.
