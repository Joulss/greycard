# §604. The Develop sections told apart (2026-11-07)

Issue 80 (0.10.1): the Develop panel's sections run into one list of
controls, Look, White Balance, Camera Profile, Light, Color, Curves,
and on, and a tester finds the boundary between them by reading. The
headers are a small semibold title in capitals on the panel's own
surface, with a fold arrow, and nothing marks where a section's
controls end. Lightroom's panel sets each section off with a header
band and a border; the tester asks for some of that structure and
none of the weight.

## The decision

One mark, from the theme's tokens, and no change to the controls or
the spacing: a hairline in `border` along the top of each section.
Each header then sits between the line above it and the rule the
header already draws under itself, so a section reads as a block
with a lid, and the run of folded sections at the panel's end reads
as a list of headers and not as one mass. Spacing stays as it is,
since the tester's one fear is losing the panel's compactness.

We tried a band first, the header on a surface one step up from the
panel's (`raised`), and then one step down (`base`), captured on the
same frame beside the line alone. The lighter band read as a row of
buttons, cheaper than the panel around it; the darker one as a
recess that drew the eye to the headers more than to the controls.
The line alone did the telling apart and left the panel's look.

Every section comes from one component, `Section` (controls.slint),
so the change is made once and reaches the panel's sections, the
Masks tab's, and the left pane's, which share the component. The
tokens are per theme, so Charcoal, Grey, and Light each get their
own line.

## The mechanism

A 1 px `Rectangle` in `Theme.border` at the top of `Section`; the
header's own padding and rule unchanged. The sheets' own headers are
not `Section`s and do not change. The capture harness took the panel
before and after, and with each band, for the record.

## What was built

What was built: in `Section` (controls.slint) the header row sits in a
plain `Rectangle` that also holds a 1 px `Theme.border` hairline at
`y: -root.padding-top`, that is, at the section's top, in the padding
above the header. It is drawn outside the header's box, so it takes no room:
the section's layout (padding, spacing, header, rule, body) is as it was.
No fill. The hairline spans the section's own width; the parent's 16 px
side padding stays clear.

## Verified
- Debug build, headless weston (pixman) with the window on lavapipe, a
  fresh weston and config each run, `--snapshot` at 1500 x 950 (fit to
  1500 x 800), a 45 MP CR3 copied into the worktree: before and after differ
  only in the hairlines; the body rows (Look's list and slider, White
  Balance's rows, the Navigator, the Presets list) are pixel-identical, so
  no spacing moved.
- Charcoal with Light and Color folded (`--panel-scroll 260`): each folded
  section reads as a header between the hairline and its rule, its switch
  as before.
- Light theme, same fold: the hairline shows on the lighter surface.
- The left pane's Navigator, Presets, and Snapshots take the same line.
- First section under the tabs (Look) and under the left pane's buttons
  (Navigator): no line above either already, so the hairline does not
  double one.
- The Masks tab: the pinned list's full-width foot rule already marks where
  the sections start, and with a mask chosen the first section's hairline
  ran 8 px under it, inset. `Section` takes `line-above` (on by default) and
  the Masks tab's first section, the mask's own, sets it off, so the foot
  rule is the one line with a mask chosen and without (none follows then).
  Captured with a mask made by clicks (`--keys`): before-masks.png has the
  foot rule at y 423 and the hairline at y 432; after-masks.png has only the
  foot rule (zooms: before-masks-zoom.png, after-masks-zoom.png); the
  no-mask state (probe-masks.png) shows the foot rule alone.
- The committed hairline renders the same as the earlier hairline-only trial
  (alt-hairline-develop.png): the Develop panel's region is pixel-identical.
- `cargo test -p greycard-ui` (lavapipe env, 4 threads): 1342 + 171 + 3
  passed, 0 failed. `cargo clippy --all-targets -p greycard-ui -p
  greycard-slint` with `-D warnings` clean. `cargo fmt --all --check` clean.

## Not verified
- The Grey theme on screen (border #4e4e4e on surface #3b3b3b).
- The Masks tab, Collections, Folders, History, a superseded (dimmed)
  section, and one with an owner before the title, on screen: the header
  row is unchanged inside its box, so their layout is the same by
  construction, but none was captured.
- Hover and press on screen (unchanged code; the header has no fill).
- The workspace-wide clippy (only greycard-slint's .slint changed).
