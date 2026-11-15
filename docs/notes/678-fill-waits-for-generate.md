# §678. Fill waits for Generate (2026-11-14)

Painting with Fill used to make a fill at every release of the button,
so an area took as many generations as strokes, each one a wait nobody
asked for. Now only Enter or Generate makes a fill, with no exception:
no way out of the tool makes or saves anything.

## The behavior

With Fill in hand a stroke is painted, not made: drawn over the picture
as a repair being drawn is (the chosen patch's edge and feather paths,
same colors), and kept in the session only (`Develop::painted`, module
`panel/painted.rs`). Nothing goes into the edit, the history or the
sidecar, and no develop is asked. Heal and Clone are as they were.

Only Enter or **Generate** makes a fill. Generate is a full-width
`PanelButton` under the Fill's Patch/Fast/Best row, shown with the Fill
in hand, enabled and primary only while strokes wait on the open frame
and are drawn there.
It has no tooltip: a button with words has none, and no tooltip in the
window shows a key to copy. Enter acts only while the picture's keys
have the focus and no field is open, so Enter in a number field, or on
a slider holding the keys, is theirs. Enter, Esc and the keys' undo
reach the strokes only where they are drawn, the Develop picture's
Retouch tab: one rule in the window, `fill-view`, which the keys read
and `painted::undo_last` asks for. In the grid, the lightbox, the crop,
and culling they keep their own meaning, the undo a mark's or the
edit's. A Generate is one history step,
written, and asks the develop that makes the fills; the tool stays in
hand.

## Strokes wait on their frame

Strokes are kept per frame, by key (path and version id), for the
session. Leave the tool, the tab, the crop, culling, or the frame, and
come back to it with Fill in hand: they are there, drawn, and Generate
is lit. They are drawn, and Enter, Esc and Generate act on them, only
while Fill is in hand on their frame (`painted::waiting`, set on every
tool change, frame open, and frame drawn).

Nothing else makes them, and nothing writes them: the tool's button,
Heal, Clone, a tab, R, the crop, the grid, culling, Cull drawn
developed, another frame, a folder change, a version made or swapped,
an export, Edit in, Add to queue, and the quit all leave them waiting
and the edit as it was. An export reads the edit, so they are not in
it. The quit drops them; nothing is in the sidecar.

A frame turned carries its waiting strokes round with the same
`Turned` as its patches (`browser::turn_frames`, every version of the
file); with the picture's shape unknown they stay put, as the patches
do. A file followed to a new path (`browser::file_moved`) takes its
strokes along, again at each move; strokes still kept at the new path
were a gone file's and are dropped. A version swapped with the original
keeps its strokes on the line they were painted over: the swap
exchanges the two keys. A file gone from the list (`cull::drop_files`,
or a refresh that no longer lists it, `kept_up::merge`) and a version
deleted drop their strokes with a log line, so a file that later comes
to the old path has none. A renamed version keeps its key. Strokes are
found by exact key, so they never show or apply on another frame. At most 32 frames keep strokes; painting on a
33rd drops the frame painted longest ago, with a log line.

## Esc and undo

Esc with strokes waiting drops them, the stroke under the pointer too,
and keeps the tool; the next Esc lets go of a chosen repair, the one
after puts the tool down. The keys' undo (Ctrl+Z and anything bound to
`undo`) with Fill in hand: a stroke under the pointer makes it do
nothing; with strokes waiting, the last goes; with none, what the last
Esc dropped comes back, one level, while neither the panel's edit nor
the frame's history has changed since: the Esc keeps both, so an edit
changed and undone back to the same value, which has moved the history,
ends the offer. A held Esc that lets strokes go is one press: its
repeats are eaten (`fill-esc-held`, ended by any other press), so the
tool stays in hand for the undo. Every other Esc, held, goes on as it
did: a chosen repair let go, then the tool put down.
Otherwise it is the edit's undo. The history panel's buttons stay the
edit's alone; redo is the edit's.

## The list's choice

With a Fill chosen in the list, strokes that touch it, directly or
through other waiting strokes, are added to it at Generate and it is
made again as one area. Areas that do not reach it, and every area when
no Fill is chosen, are new Fills at the end of the list, on top of what
they overlap. The chosen row stays lit while painting.

## Sizing a chosen Fill

With any repair tool in hand and a Fill chosen, the Size, Feather and
Opacity sliders, the wheel, and the bracket keys set the brush for the
next stroke and never the Fill; the sliders show the brush, not the
Fill. To resize a Fill, put the tool down and choose it: its sliders
show with no tool in hand, edit it, and it is made again when the
slider is let go (`EditSlider::moving` falling, `patch-settled`), or at
once for a typed value. A slider's drag begun on its track marks the
drag before the jump sets the value, so the jump does not make the Fill
at once; one drag makes it once, at the release. Wherever the tool goes
down (its button, a tab, a mode), the sliders are shown again for what
they now edit. A chosen Heal or Clone is edited as before. A
notch or a key never starts a generation. The center pin of a chosen
Fill still moves it (its hit area is 20 px); a stroke started outside
it paints.

## How a repair's area is stored, and the format

A `Patch` was one polyline (`points`, `radius`). Version 9 adds `more:
Vec<Stroke>`, each `{ points, radius }`, `#[serde(default)]` (a missing
radius reads as a patch's default), left out when empty. The area is
the union: cover the max of the strokes', the ring outside every stroke
and within some stroke's 1.7 r; feather and opacity are the repair's.
A repair of one stroke writes byte-identical JSON, has a bit-identical
region, and keeps its fill slot and print. Heal and Clone are never
given `more`; one read with it is the union too.

`region` measures each stroke over its own window and max-merges into
the union window, so a Fill of many strokes costs about as much as as
many Fills of one (tested by counting the pixels measured, which equal
the strokes' own; the timing is an ignored measurement): 30 strokes of 20 points take 21 ms at 1500 wide and
301 ms at 6000 wide in release (2.93 s at 1500 before). A stroke off the
picture does not widen the window; when every stroke is off it, the
first's window stands, as a single patch's always did.

Two strokes touch when their covers overlap: polyline distance less
than the sum of their radii. Rings meeting is not enough.

The history names a Generate by the existing rules, with one change: a
step that adds one repair and changes another is "Retouch", not the new
one's name, so growing a Fill alongside a new one is not hidden. A
Fill grown alone is "Fill 1 moved"; several new, "Retouch".

Version bump: `VERSION` 9, the migration doc, the preset golden header,
and the future-version test. A v8 build refuses a v9 sidecar. An
unchanged old sidecar is not rewritten on open, and nothing is written
for waiting strokes (tested on disk).

## Tested

`panel::painted::tests` (29): Enter makes, no develop or save before;
Generate greyed then as Enter; Esc then Esc; an undo right after Esc
brings the strokes back, not over an edit change; no way out (13 of
them) makes or writes anything; strokes wait through a trip away and
back (tool, tab, crop, culling, frame) and are not shown or made on
another frame; a frame gone drops them; the 32-frame bound; a turn
carries them with the patches; Enter in a field or on a slider makes
nothing; a chosen Fill is resized only with no tool and on release;
a stroke beside the pin paints; an undo mid-stroke does nothing; a
chosen fill grows; nothing chosen makes a new one; apart and touching;
the split; one step; undo pops a stroke; painting while a fill is made;
an old sidecar not rewritten, through waiting strokes and a quit;
Enter and Esc in the grid, the lightbox, its loupe, the crop, and
culling, and the undo there a mark's, neither taking a stroke nor
bringing back what an Esc let go; a held Esc, and a held Esc with Heal
going on as before; an edit changed and undone ends the restore; the
tool put down by a tab or culling shows the chosen Fill's sliders; a
move and a double move, and another file at either old path; a swap; a
drag from the track makes the Fill once. `roots::tests::lists`: a file
gone from a refreshed list takes its strokes.
greycard-edit: the union, the one-stroke region against the old code,
the cost against strokes alone, an off-picture stroke, a Heal with
`more` and a stroke without a radius read from a file, touching, v8
JSON byte-identical, the history name. greycard-engine: a one-stroke
fill's slot and print unchanged.

## Open

- Wording still to settle, wrong while strokes wait: the Retouch hint
  "click or drag to repair; scroll for size; drag pins to move; Esc to
  finish" (`panel/retouch.rs`): with Fill a press paints, and the first
  Esc drops the strokes; "put the tool down first (Esc), then sync" and
  "…, then paste" (`panel/sync.rs`, four places): with strokes waiting
  the first Esc does not put the tool down.
- Strokes on a frame of a folder not listed now are not dropped when its
  file goes from outside; a file that later comes to that path, with
  the folder opened again, shows them. Telling would need the file's
  identity kept with the strokes.
- Storing waiting strokes across sessions is on the roadmap as a maybe.
