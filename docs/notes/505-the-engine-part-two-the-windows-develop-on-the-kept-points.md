# §505. The engine, part two: the window's develop on the kept points

*2026-11-01*

The second of §478's last three landings: the window's own develop
is a walk of its stage list over kept points, as the reduced develop
(§488) and the set export (§496) already were.

**What goes.** The open frame keeps a Kept of the window's list, a
FrameKept, and the base, the learned pair, the turn, the picture
before the sharpen, the patched picture's upload, the dehaze's copy,
and the masks' bands are its points and taps. Base, LearnedBase,
turn_base, PreSharpen, and KeptPlanes go, and with them Edit's
same_base, same_learned, and same_patched: what a develop reuses is
what its list shares with the kept points, so a Dehaze added over a
kept local contrast now resumes from that picture where the old
predicates reran it, bit-exact with a fresh develop. Edit's
same_develop stays until the open file's export moves too (3c), and
the browser keeps a small field comparison of its own, since it has
no frame facts. A scripted session (open, five moves, white balance,
a turn, a prefetch, a second frame, back from the shelf, an export,
two more moves) hits the same keep as before at every develop, and
the bar is 40 of 40 byte-equal.

**The shelf and the prefetch hold the same FrameKept.** The prefetch
is a walk of its own list that gives way before any stage. Short of
a lost device, a device error in a prefetch takes that frame to the
CPU and leaves the session its device, as before: a transient
out-of-memory on a speculative neighbor must not cost the session
its GPU ops. A device taken or lost forgets what the points held on
it. A prefetched base carries no guide plane; the window makes it,
20 ms, when it resumes.

**What a point carries, continued (§501).** The window's list keeps
no developed picture, and its picture before the sharpen only on the
device; after a declined device run the CPU's copy is read for the
refined masks' picture and then let go, so the largest frames hold
no extra picture. A turn of a kept base takes the learned pair and
the masks' tap with it, so a walk resumed from it still makes its
masks. A walk resumed past the masks' bands takes the bands kept
(the review's one visible regression: a mask's Texture vanished on
the first sharpen move on the device, since the device run lands its
bands after its keeps). The upload is matched by the picture it was
made from, not its steps, since an export's base made with the CA on
the CPU has the same steps, and goes when no point holds that
picture. The refined masks' picture (§503) travels with the base:
tapped on the CPU and kept for the next walk, read back from the
device's picture before the sharpen.

**Left for 3c.** The open file's export still walks the window's
FrameKept with no GPU, so its resume can drop the window's device
points; a read-only resume or a borrowed copy. same_develop retired
with it.
