# §651. A check that the catalogs match the source (2026-11-12)

`scripts/translations-check.py`, Python 3 and nothing else, reads the
source and the catalogs in two seconds, with no build and no gettext
tool. Run it bare for errors and counts, `--list` for every reported
string, `--self-test` for its readers on small cases. Each error says
what to do.

**What it checks.**

1. *The template against the source* (an error). Every string marked
   in the source has its entry in `translations/greycard.pot`, and
   every entry is still marked somewhere, by context, English, and
   plural: `@tr("s")`, `@tr("ctx" => "s")`, and `@tr("one" | "many" %
   n)` in the Slint files; `tr!` in the same three shapes and
   `marked::tr!` in greycard-ui's Rust, comments and test-only names
   (`shown!`) aside, and an `@tr` inside a Slint string's `\{...}`
   included. Its reading matches slint-tr-extractor's and
   xtr's string for string on master, but for the one file xtr skips.
2. *Each language against the template.* Missing, empty, fuzzy, and
   obsolete entries are a report, since a catalog behind the source is
   normal. A translation whose placeholders differ from its English's
   is an error, read as `i18n::fill` reads them: `{}` by place, `{0}`
   by index (so `{} of {}` may be `{1} sur {0}`), `{name}` by name,
   `{{` a brace. A plural's form may leave out the count `{n}`, as
   English's "One frame" does; any other loss, or a name the English
   lacks, fails. The same slip in an entry the template no longer has
   is never read, so it is reported, not failed.
3. *Words nothing marks* (a report). In the Slint files, a worded
   string bound outside `@tr` to a property the existing scan
   (`tests/untranslated.rs`, which fails on its own) does not read; in
   the Rust, one handed to a `set_` call or `SharedString::from`
   outside `tr!`, the log, and the tests. Keys are left out: strings
   compared or matched anywhere, those in an `options` list, and
   properties bound to a `current` or `options`, with their Rust
   setters. `translations/untranslated.txt` lists the rest that are
   data or keys; the Slint scan's own list is read too.

CI runs it as a job of its own, on Linux, its self-test first.
`scripts/translations.sh` runs it after a refresh and fails with it.

**What it found on master.** Direction 1: 6 strings marked with no
entry, all in `crates/greycard-ui/src/archive/rejects.rs`, the reasons
the rejects queue gives for a frame left or an entry dropped. xtr resolves `archive.rs`'s `mod rejects` to
`src/rejects.rs` and never reads `src/archive/rejects.rs`; the
template took `src/rejects.rs`'s strings twice instead. No stale entry,
no plural mismatch. Direction 2, French: 1844 of 1856 translated, 12
missing (the white balance presets and §629's six reworded warnings),
none empty or fuzzy, 18 obsolete without `#~`, no placeholder
mismatch. Direction 3: 2 candidates, both Slint window defaults.

**What we fixed.** The six entries added to the template by hand,
after `archive.rs`'s, in source order; nothing renumbered or reordered.
`scripts/translations.sh` now reads, as a root of its own, each file
with a `tr!` that the walks from lib.rs and main.rs did not reach, so a
refresh keeps them (and the next file xtr misses the same way). On
master with this merged, 1864 entries and 1864 marked strings, no
error; French 1844 of 1864, 20 missing (Midtones and the VRAM line
among them), 18 obsolete; 2 unmarked candidates.

**Left for the maintainer.**

- French's 18: a translator's, through a pull request. Six of them
  are §629's warnings without their leading ", "; the old forms are
  among French's stale entries.
- The two defaults, `adjustment-names: ["Global"]` and
  `lens-choice-labels: ["From the file"]`: wrapping them adds entries.
- Rust words that reach the window through a helper of ours, not a
  `set_` call, are beyond the scan; review still catches them.
