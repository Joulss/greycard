# §561. The too_many_arguments allows, one by one

*2026-11-05*

The roadmap counted 52; the tree held 54. Each was taken out and
`cargo clippy --all-targets` run to see which still fired. Nothing
changed behavior, and no test's expectation moved.

**Stale, 9.** Seven arguments or fewer, under the lint's threshold:
the panorama test light (`turned`), the blend's `add`, the CA test's
pass votes, `stack_files`, the GPU guided filter, `export_open`, and
three sync calls (`apply_preset_to`, `paste_onto`, `lay_paste`).

**Gathered, 25.** Where the arguments were one thing:

- A size or a point as a tuple, `(width, height)` or `(x, y)`: the
  warp's output size, the à trous level and its row (with the row's
  two outputs and the level's spacing and guard), the bad-pixel fix
  and its median, the X-Trans tile, the test DNG writer, a crop's drag
  on its source, and the GPU CA test's two checks.
- An existing struct, or a tuple of what travels together: the
  overwrite sheet's `Pending`, now
  what `start_set` and `start_or_ask` take beside the policy (a test
  helper, `reveal_in`, gave way to the `Layout` it wrapped); a
  stack's reference as `stack_files` read it, frame, metadata, and
  white; the shelf's file as path and stat; a base swap's leaving and
  coming frames; the box mean's three textures; a step range as a
  `Range`; a sharpen run in the steep-mask check.
- A small new struct: `DevelopLog`, the develop line's eleven, for
  the line and its note; `Resampling`, a panorama warp's step, kernel,
  gain, and margin; `Holding`, a held-out fit's name, label, and test.
- Taken from what was already passed: the DNG container's headroom,
  which its options carry, and its compression with its parameters.
  The dehaze's taps went as `tap`'s triples.

**Kept, 20**, each with its reason in one comment line at the allow:

- A pair spread through a module, where gathering it is the module's
  change, not one function's: the sync's `profiles` and `probe` (3),
  and a row request's frames, scope, words, and action (1).
- A group with a home elsewhere: the four mask rasters' base, image,
  edit, kind, and turn are `worker::masks::On`'s picture, and moving
  `On` into `ai` is a change of its own (4).
- Different things with no grouping: the walk's attempt, the develop
  job, the CLI's develop and its after-the-lens ops, a view's read,
  and the index's pass (6); and the render's pixel, pick, finishes,
  and exports, whose inputs are each a stage's or a plane's, and whose
  pick callers do not hold one `Baked` (6).

**Left.** The eight the module-wide groupings would take: `On` moved
into `ai` for the four rasters, the sync's pair through its module,
and a row request through `rows`. The steep-mask check's four-tuple
and the row kernel's `(mult, inv_sigma2)` are the weakest gatherings,
a test's and a private kernel's.
