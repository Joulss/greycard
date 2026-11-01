# §478. The engine, part two: the worker below the window

*2026-10-31*

The second step of §454's part 2, after §476: the worker and what
it holds now live in `greycard-engine`, with their tests. Twenty-two
modules moved as renames, the window names them at the paths it
always had, and the 28 exports of §476's bar are byte-equal before
and after, the nondeterministic Nikon TIFF included this time.

**What moved.** The worker and its loop, the reduced develop, the
thumbnail pool, the previews, the engine half of the edit's pictures,
the version key, the AI side, the queue, and the export sheet's
settings. What they reached up to the window for came down with
them: the camera's picture of a file, the made thumbnail sizes, and a
path made safe. The three helpers §476 copied into `stages.rs` are
gone; the originals serve. The walker's `State` is now `Carried`,
since the engine has a state of its own coming.

**The cfg hazard, by design.** A module's `cfg(test)` applies only
to its own crate's tests, so once a module lives in a library the
window's tests lose the hooks they shared with it (§467 met the
first form of this). The hooks the window's tests need sit behind
`any(test, feature = "testing")`, and the window's dev-dependency
turns the feature on. The gate is wider than the window needs today,
which is harmless since it only reaches test builds; nothing under
`cfg(test)` alone is reached from outside a test, and the release
build carries the engine with no features.

**The background, held.** The worker's pool and its reduced develops
are a `Background` the worker holds through the `Hold` trait of
§476, with the thread limits moved on the same edges as before. The
headless export builds a worker without one, so an export from the
command line spawns neither the pool nor the reducer's threads.

**What stayed.** Four tests that read the window's loupe or its
prefetcher stay in the window, on the engine's test rig. The source
test names are the same set before and after, 3362 of them.

**What is left of part 2.** The three develops as walks of the list,
in three landings: the reduced develop and the masks' pictures under
Reduced and ModelsSee; the headless export and the sheet's export
under Export; then the main develop, its last frame, and the shelf
onto the kept points, the four `same_*` predicates retired, and the
kept points forgotten where the engine loop takes a new device. Each
checked by the bar; the last also by the shelf and prefetch tests and
a comparison of cache hits, since retiring the predicates can change
hits without changing bytes. The walker's imports from the worker's
modules, an upward dependency inside the crate, are sorted out there.
