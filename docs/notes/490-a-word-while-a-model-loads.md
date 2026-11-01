# §490. A word while a model loads

*2026-11-01*

A tester gave up on a 47 s model load with nothing shown (pixls,
2026-10-28), and the remove's UNet took 27 to 40 s at a frame's first
fill on an M5 with no word (§477). Two things, one landing.

**The word.** Every model session opens under a load mark the
runtime counts, ONNX Runtime's own first load and the weights'
mapping (§482) included; the window hears the first start and the
last end through a listener, off the loading thread, handed to the
event loop. Once a load has run 300 ms the status line shows
"preparing...", in the lowercase-and-dots form its other running
words take, over the viewport's, the Library's, and the placeholder's
line without overwriting the words beneath, which come back when it
clears. It comes down 200 ms after the last load is ready or has
failed, so a model of two sessions, SAM's encoder then its decoder,
does not flicker between them; a load under 300 ms never shows it.
The mark is a drop guard, so a panic in a load takes the word down.

**The early load.** Where the remove keeps its UNet open anyway,
unified memory of 16 GB or more or the resident flag (§421), the
UNet lives in one slot a process. Taking the remove's Fill in hand
with Fast or Best chosen, or changing between them with the Fill in
hand, opens it there on a thread of its own, so the load overlaps the
drawing of the hole. A removal that comes while it is still opening
waits for that open rather than start a second; a wrong size or
sampler drops it and opens the right one; an open that panics leaves
the slot free and wakes the waiters. Nothing loads for Heal, Clone,
LaMa's fill, a stroke, a frame opened with fills already on it, or a
machine without WebGPU, and never before the user reaches for the
Fill. A held UNet outlives one removal, so a set's export reuses it
instead of loading it for every frame. The early load is at 1024, the
window not yet known; a small hole that wants 512 waits for it, drops
it, and opens 512, a cost in time only.

Not measured here: the M5's first fill, cold, which the bug line
still asks for before it comes off.
