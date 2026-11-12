# §650. A fuzz target for the DCP reader (2026-11-12)

A DCP is a file the user picks from disk, so its reader takes bytes
nobody vetted: a TIFF-like directory whose tables carry their own
dimensions. §375's targets covered the text readers and §474's the DNG
opcodes; this is the tenth, `dcp`.

**What it covers.** Each input is read twice: as a file, and laid over
a small synthetic profile (its first two bytes say where, the rest
overwrite from there), so a run explores a well-formed file with no
profile in the corpus. A camera profile is not ours to commit, so
there is no seed file; the synthetic profile is built in code
(`greycard_core::testing::dcp::synthetic`) and carries every tag the
reader knows: two calibrations with forward matrices, a map for each,
an sRGB-encoded look table, a tone curve, an offset, and its names.
The checks live in `greycard_core::testing::dcp` (the fuzz crate turns
on the core's `testing` feature), so the core's own tests run them too.

**What it asserts.** Never a panic. What reads in full reads its header
(`Dcp::info_from_bytes`) and says the same. Every table has the
dimensions it claims, within `MAX_DIVISIONS`, a saturation axis of at
least two, and entries finite and within `MAX_TABLE_ENTRY`. The offset
is finite and within `MAX_OFFSET_STOPS`. A tone curve has at least two
points. The profile writes back (`Dcp::to_bytes`) as a file that reads
the same names, policy, maps, look table, and offset. Then as the
develop runs it: `Dcp::profile`, a white balance solved against it as
an as-shot one is and one asked for by temperature, the maps blended at
six fixed temperatures and at the two the solves give, and the look
table read at seven exposures (Adobe's sort, past `MAX_LOOK_STOPS`
either way, and NaN), each stage run on fourteen fixed pixels (black,
white, grey, the primaries, a highlight, colors outside the space, all
negative, 1e4, and 1e-30). Every pixel comes out finite.

**How long it ran.** cargo-fuzz is not installed on this machine, so
the same checks ran as an ignored test (`mutated_profiles_hold`) over
mutated copies of the synthetic profile and, locally, five real
profiles a tester sent, never committed: bit flips, edge values (sizes,
offsets, and floats at their limits) dropped on a directory entry's
type, count, or offset or anywhere in the file, truncation, and
splices, with what still reads kept to mutate further. Debug build,
overflow checks on, one thread: 1.34 million inputs in 600 s, 0.59
million of them read in full, the slowest 50 ms, nothing found. Before the fix, 228 thousand inputs
in 30 s on the synthetic profile alone.

**What it found.** One defect, inside the first dozen inputs: a table
entry that is finite but huge. A look table's value scale of 1e30
threw a pixel to infinity, and the turn back from HSV made it NaN;
two neighbors near `f32::MAX` overflow the interpolation between them
too. §639 refused an entry that is not a number and left a finite one
as given. Now an entry past `MAX_TABLE_ENTRY` (1e6, either way) is
treated as one that is not a number: a map's refuses the file, so the
develop falls back to the frame's own calibrations, and a look table's
is dropped alone with a warning, on the way in and on the way into the
engine's profile. 1e6 is far above anything real (Adobe's hue shifts
are a few hundred degrees and its scales under 5) and keeps §639's
example of a value scale of 1e6 reading as given; with tables at the
cap itself, every fixed pixel stays finite at every exposure tried. A unit test, `fuzz_a_table_entry_past_the_cap_is_not_read`,
builds the shape synthetically.

**Not covered.** The matrices: a hostile color matrix can still make
the develop's picture nonsense, which is the matrix's job to refuse, in
rawcolor, not the reader's. The tone curve is read and applied by
nothing, so its points are checked only for their count. CI runs the
target through `fuzz.yml` with the others; its timeout went from 120
to 140 minutes so ten targets at 600 s still fit.
