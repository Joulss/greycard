# §515. The architecture document

*2026-11-01*

`docs/architecture.md` is the map of the code, the one concrete item
of the 2026-10-19 review's contributor readiness: the crates with
what each owns and may depend on, checked against every Cargo.toml;
the develop's stage list with the crate, the CPU reference, the
device twin, and the parity test of each step; the window's `State`
by owner and the worker's handles; the sidecar, the index, the sets,
the model store, and settings.json on disk; and the testing rules.
It is written from the code; the notes stay the why.

**Where it supersedes.** §5's principles predate the ai, render, gpu,
engine, and viewport crates; its CLI as the first front end is §7's
window now, its no learned features at first §34's, and its one render
graph holds for every develop of an edit but not for a camera thumbnail
or `greycard develop`. Against §454: core's `develop::finish` kept its
name for the move into the working space (only its dehaze and sharpen
left), and `greycard develop` still calls core's ops directly, so only
`greycard export` walks the list.

**Kept up.** A crate or a stage that moves moves the page in the same
landing. The camera match's and the measurement runners' move into the
window's library is named there as planned.