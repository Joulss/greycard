# §653. A kept point bound to its frame (2026-11-12)

**Found.** An outside review (2026-11-11) noted that the walk's resume
cache, `walk::Kept`, matched a resume on steps and places alone. Two
frames of a burst with a synced edit have equal steps, so only each
caller's habit kept one frame from resuming on another's pixels.

**Latent.** No path crossed frames. Five callers walk into a fresh
`Kept`; the worker's open frame has its base taken out by `swap_base`
on every open, before the input changes. `develop_round`'s check on
the frame's facts would not have caught a cross: the facts are equal
across a burst. The shelf is the one place a `Kept` meets another
allocation of the same frame, handed back by its file key to a fresh
decode.

**The identity.** An input carries no id and no hash, and hashing
samples on every develop is ruled out. We bind to the input's own
allocation: `Kept` holds a `Weak` of the raw frame, or of a picture's
image, and `walk_held` compares pointers before anything else. A
mismatch lets go of everything held: points, device buffers, the
upload, the planes, the sharpen guide. The `Weak` keeps the allocation
(not the samples) from being freed, so no other input can take its
address while the `Kept` lives. One pointer compare a walk.

**Adopt.** A base taken off the shelf is the same frame read again, a
new allocation. `Kept::adopt` lets the caller vouch for it, and
`swap_base` does so where the file key's match is proven, so the
shelf still resumes.

**Tests.** A frame given another's points of equal steps does not
resume and equals a fresh walk to the bit, on the CPU and for an
export's device points; a frame read again resumes only once adopted;
the shelf's take-back tests now re-read the frame and fail without
the adoption.

**Left.** `Kept`'s readers outside a walk (`picture`, `patched`,
`planes_for`) still trust the caller's pairing. They read the last
walk's output and resume nothing.
