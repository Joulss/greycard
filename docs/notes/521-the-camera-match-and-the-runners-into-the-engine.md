# §521. The camera match and the runners, into the engine

*2026-11-01*

The engine track's last small item: the camera match's runner and the
measurement runners out of the window's binary, now that the frame
develop they call is the engine's (§506).

**Into the engine, not the library.** Every piece calls
`worker::FrameDevelop`: the fit run develops each sampled frame, and
the compare harness, the baseline, and the Lightroom measurements
each develop and finish a frame. greycard-library sits below the
engine (the engine reads its index and cache), so it cannot call the
develop without a cycle; and no windowless piece is free of the
develop, since the survey and the plan exist only to feed the run.
The roadmap's line said the window's library target; that would keep
11,000 lines in the window's crate, compiled with it, when nothing in
them names the window. So the engine, beside the develop:
`camera_match` (with its test-only `cover` and `sweep`),
`match_compare`, `measure_baseline`, `measure_crop`,
`measure_lightroom`, and `fit_lightroom`, as they were. The engine
depends on greycard-match now, and on blake3 for the records' keys,
which the library already brought. The window keeps greycard-match for
the fit version the Look section checks a table against.

**What the window keeps.** The sheet (`panel/camera_match.rs`), its
progress, and the calls. The five `headless(cli, …)` entry points took
the window's clap struct; each is now `headless(&Options)` in the
engine, and a new window module, `runners.rs`, reads the flags into
the options as each did (the library defaulting to the user's, the
`--scale` and `--fit-scale` pairs parsed). Visibility widened only
where the window reaches in: the survey, the plan, the run and its
results, the scope, two refusal texts, `Frame` and `MIN_FRAMES` for
the sheet's tests, and each runner's `Options`.

**Imports.** The moved files had no glob import; they named the
window's crate by path (`crate::Cli`, `crate::export`,
`crate::finish`, `crate::testing`). Now none names it: render's
`export` and `finish` are imported as such, the engine's own
`testing` has the same two helpers. The sheet traded `use crate::*`
for explicit imports. Glob imports of the crate root in greycard-ui:
50 before, 49 after; every glob in it but the tests' `super::*`: 58
before, 52 after (the four rayon preludes moved, and the sheet's).

**Tests.** The 72 tests move with their code (36 camera match, 6
cover, 1 sweep, 10 compare, 4 baseline, 1 crop, 8 Lightroom, 6 fit),
names unchanged. One, the ignored `the_samples_run`, printed the
sheet's plan lines through the window's `group_line` and `summary`;
the engine's prints each plan by `Debug`, and the window gains an
ignored test of the same name that prints the sheet's lines and the
summary over the same survey.

**Held equal.** `--match-compare` over 20 frames of one body and
style (four raws copied into five folders, an index of their own),
from a release build of master and of this branch: all 20 frames'
records and the report are identical but for the build line (the
binary's hash) and the timing fields. Every test passes: the engine's
347 (16 ignored), the window's library 170 and binary 1235 (4
ignored), with clippy and fmt clean.

`docs/architecture.md` moves with it: the engine's row owns the
runners and depends on match, and the planned paragraph is now the
paragraph of what is.
