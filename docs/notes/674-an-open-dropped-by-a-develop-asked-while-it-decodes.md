# §674. An open dropped by a develop asked while it decodes (2026-11-14)

## The invariant

The window's develop generation says which picture it waits on: a
develop lands only at the generation waited on, so a picture of an edit
or a frame left since is never shown. What an open says of the frame
(its kind and mosaic, its As shot white, its lens report and options,
the profile list, the camera, the shot, the capture time, the learned
blend) is a fact about the file, not about the edit. It belongs to the
visit: taken whenever it is for the frame chosen now (the same file and
version) and from the last open sent in this visit, whatever develop was
asked since; refused for a frame left, for an earlier open of the same
frame (chosen again, its file written again under the window), and in
culling, which waits on the open its leaving sends.

A second invariant, the same class: no picture of one file is ever
shown as another's. The worker develops only the file it holds; the
window asks a develop only of a frame whose file the worker was sent.

## The cause

The worker folded a develop into an open only while the open still
waited in its slot (`handle::into_slot`). Once the worker had taken the
open, a develop of that frame asked during the decode (a slider's 300 ms
debounce, the Original key, an undo, a turn) moved the window's
generation on, and the window refused the open's `Opened` for not being
the current generation (`deliver.rs`). Everything the open reports was
lost for that visit: As shot and the slider resets did nothing, Auto and
the dropper read the last frame's camera, the lens panel, the profiles,
the mosaic menu, the shot line, `raw_input`, the noise section, and the
picture's offset ranges were the last frame's or empty. The reviewer's
probe (`opened_gen=2 now_gen=3`) is this.

The same class, found on the way and in review:

- An open that fails or panics: its `Failed` was refused the same way,
  and the develop asked during the decode then ran on the file the
  worker still had in hand, the last frame's, and was shown over the
  frame that failed. Without the race too: any slider moved on a frame
  whose open had failed or panicked developed the last frame's file.
- A frame chosen without an open (a pick still reading its sidecar, a
  frame under an offline root): the panel's controls asked develops of
  it, which the worker made from the file it had, another frame's, and
  the window showed as this one's.

## The fix

- The worker, once the open has read its file and before it says what
  it read, takes a develop of that frame (file and version) waiting in
  the slot into the open, under its edit, generation, and turn, as
  `into_slot` does for an open still waiting (`Queue::take_develop_of`,
  sharing `handle::develops` and `Queue::open_folded` with it). The open
  then lands at the generation the window waits on, one develop runs
  under the newer edit, its learned blend seeded as the open's would be,
  and the neighbors and the open the worker says landed follow it. A
  panic after the fold is blamed on the new generation; a failed open
  fails at that generation too.
- An open that panics is a failed open in every respect. A panic in the
  read (the decoder) is caught round `open_told` and becomes the read's
  error, in the words every panicked job fails in ("the worker panicked:
  ..."), so the fold and the failure remembered follow as for an error.
  A panic after the read and before the file is the worker's (its
  profile, its base) goes to the job's panic handler as before, which
  now also remembers the open's file as failed when it never became the
  file in hand. One report each: the caught panic is the open's Failed,
  the handler's is the blame's. The worker lives on as before.
- The worker remembers the file whose open failed last and its message
  until an open succeeds; a develop or an aside of that frame, while
  another file is in hand, is answered with that failure. A develop of
  any other file than the one in hand makes nothing (`handle::holds`,
  logged). No new text.
- The window asks a develop only of the frame of the last open it sent
  in this visit, under the path it sent it by (`edit::develops_here`, at
  every sender: the sliders' debounce, the timing runs, undo and redo,
  the turn, the Original, a model arrived, a blend seeded). Not of a
  frame standing in from its row (a pick still reading its sidecar, a
  frame under an offline root, one whose read was given up on), whose
  file the worker was never sent. So the worker is never asked a develop
  it declines: a develop the window sends names the file of the last
  open it sent, which the worker holds (that open succeeded), is opening
  (the open ahead of it in the slot, or folded with it), or failed to
  open (answered with the failure). Nothing is left at "developing..."
  waiting on an answer that will not come.
- A frame whose file moved under the window (found moved, followed to an
  archive's copy) is opened again under its new path
  (`browser::reopen_moved`, from both follows, through `reread_open`).
  The worker is told nothing of the move: it holds the file by the path
  it was sent, and the one open under the new path sets every path it
  keeps of the frame in one job. Until then no develop is asked (the
  open sent named the old path, `open_sent` ends with the move). We
  first told the worker of the rename instead; review found it left the
  learned masks refused and a job already queued under the old path
  answered with nothing, and every path-holding place on both sides
  would have had to follow in the same step. The open again costs one
  decode of a file found moved, which is rare. Followed to an archive's
  copy, the open again reads the copy; if that read fails, the frame
  reports a failed open (and its develops that failure) until it is
  chosen again, where before it went on developing the bytes in hand.
- The window records the last open it sent, its generation and frame
  (`open_sent`, set in `open_job` and `reread_open`, every open the
  panel sends), and ends that record whenever the chosen frame changes
  to another (`browser::current_moved`), so a return without an open of
  its own takes no word of an earlier visit. An `Opened` is taken when
  it is the generation waited on (as before), or, outside culling, when
  it is at or after the recorded open's generation and that open's frame
  is still the one chosen (`browser::opened_is_current`). Taken behind a
  develop, the Original's re-read is left (the develop asked since is
  what the Original gives way to), and a learned blend it seeded has the
  edit developed again, said as "developing...", since the develop in
  flight was sent without it; not while the Original is on show, nor
  with the learned denoiser off, which leaves the blend unused.

The window's acceptance is still needed with the worker's fold: a
develop asked after the worker says `Opened` but before the window takes
it (a timer firing in the same turn of the event loop) still moves the
generation.

The failure remembered lasts for the visit: after a failure that would
not recur (a network read that timed out), every develop of that frame
answers the old failure until an open of it is sent again, by choosing
it again or by its file changing on disk (the re-read). We accept that:
a frame that failed to open has nothing to develop until it is read
again, and the failure is what the status line should keep saying.

## What holds the open frame's path, and how each follows a move

`State::move_file` is the one place a listed file's path is written;
`browser::file_moved` does the rest in the same step.

Window, renamed: the list (`files`), its companions, a collection's
frames, the panel stand-in (by the follows), the frame's stance and
mosaic (`stances`, `mosaics`), the size its develop lands at
(`opened_sizes`), the mosaic the panel shows (`panel_mosaic`), the file
the learned masks are of (`learned_of`) and their failures'
(`masks_failed`), the lens frame (`lens_frame`, else the lens filter
would reset), a crop fit due (`crop_fit_due`), the walk's last step
(`walk.from`, else the next step would count as a jump), the custom
white As shot replaced (`before_as_shot`, keyed by the frame, else As
shot off after a move gives nothing back), the way back from a middle
click's 200% (`zoom_back.frame`, else it goes to fit), and the command
line's overrides (`launch.overridden`).

Window, ended and set again by the open again: the open sent
(`open_sent`); the masks asked and failed (cleared and asked again by
`learned_anew`, the rasters kept drawn); the Original's pictures
(`original::rewritten`); a person's pick (`parts::picture_changed`); the
neighbors sent (`neighbors_sent`).

Window, translated: what a job the worker was running on an earlier
path says (a mask, a mask on the CPU, People, Landscape, a merge's
denoise again, a mosaic) is taken as the frame's while the frame is the
one moved in this visit. Every earlier path is kept for the visit
(`moved_from`: the path now and the earlier ones, cleared when another
frame is chosen), so two moves in a row (A to B to C, or A to B and back
to A) resolve a job begun on any of them (`browser::is_open_file`,
`was_at`). People and Landscape look at the file before they take the
press waiting, so an answer for another file leaves the press for its
own.

Window, left: `pick_pending`. The read it waits on is itself keyed by
the old path (a request resolves its frames by path, `rows::resolve`),
so a pick whose file moves before its sidecar lands lands as gone, and
that ends the pick only while `pick_pending` still names the path its
request captured; followed, the pick would hang. It is also compared
with the frame's path when another frame is chosen (`pick_pending`,
`open_frame`: the outgoing panel is saved unless it was a pick's) and
in a refusal's words (`rows::refused`): after a move those see the pick
as not one, which costs nothing, since a stand-in's panel is refused
when saved (`rows::panel_is_frames`; the stand-in follows the move) and
its read has indeed not come in. The
Original's kept
frame key (dropped by the open again); culling's caches by frame (a miss,
read again); the history of frames visited and the recent list (a
navigation list, where a gone path is skipped as any gone file). A cache
keyed by a path a move left can only miss, never hit a wrong file,
unless another file later takes that path, as it could before.

Worker, set by the open again in one job: the file in hand
(`opened_path`, `input`, `metadata`), the frame left (`left_path`), its
shelf key (`opened_file`), the landed open (`landed`), the failed open
(cleared by a good open), the fold (`folded`, none at a new open), the
open frame for the pool (`edited.set_open`), the masks' models
(`ai.forget(path)`).

Worker, jobs naming the old path:
- waiting in the slot (a develop, an aside, or an open of the frame):
  the open again replaces it, the slot holding the newest; nobody waits
  on it, the window waiting on the open again;
- running (an open, a develop): it ends on what it read and answers at
  its own generation, which the window, waiting on the open again,
  refuses; a mask, People, or Landscape run answers under the old path,
  which the window translates;
- a save (`Keep`) queued before the open again runs first, on the old
  path still in hand; one after, before the open is taken, finds another
  file in hand and leaves its pictures to the pool's reduced develop, as
  any save of a frame not in hand;
- a fill and the mask picture are keyed by generation; the masks queued
  and the People and Landscape asks name no file and run on the file in
  hand when taken, after the open again;
- an export of the frame under the new path, before the open again
  lands, is made on the side (`export_other`), as any frame not in hand;
- thumbnails, the reduced develops, the prefetch, and a person looked
  for on other frames name other frames or are asked again by the list
  after the move (`ask_owed`).

## Considered and rejected

- The worker's fold alone: leaves the event-loop race above.
- The window's acceptance alone: the open's develop of the stale edit
  still runs to the end before the one asked (a wasted develop of 0.3 to
  2 s on a big raw), the develop asked runs without the seeded blend,
  and a failed open still shows the last frame.
- Carrying the path and version in `Opened`: the window's record of the
  open it sent says the same without changing the outcome every test
  and the headless export build.
- Only the worker refusing develops of a file not in hand: the window
  would sit at "developing..." with nothing coming. The window not
  asking is the fix; the worker refusing is the guard.
- Telling the worker of a move (`Worker::file_moved`, tried in the
  last round): every path the window and the worker keep, every queued
  job, and every outcome in flight would have to follow in the same
  step; the open again does that by construction.
- In culling, taking an open's word: culling never took it before (its
  entry moves the generation), and the model offer the open's landing
  makes would come up over culling.

## Tested

Worker (`worker/tests.rs`, with an open held once it has read its file,
and panics placed by a test):
- a develop asked during the read is that open (Opened and Developed at
  its generation, its turn, the picture its edit's, the neighbors and the
  landed open retagged);
- a failed open with a develop asked during the read fails at the
  develop's generation, a develop or an aside after it is the same
  failure, then a good open develops as before;
- a decoder that panics with a develop asked fails at the develop's
  generation in the panic's words, and a develop after is that failure;
  without the race, the same; a panic after the read (the fold done)
  likewise, and the worker opens the next frame;
- a frame whose open failed, its file written good and opened again,
  develops;
- a develop of a file with no open, asked while another opens, is not
  folded and makes nothing;
- two opens in quick succession land each at its own; another version
  of the file is not folded.

Window (`panel/deliver.rs`, `roots/tests/picks.rs`, `roots/tests/lists.rs`,
`sync.rs`):
- step to a frame and nudge a slider before its open lands, then the
  worker's order: the camera, As shot white, lens, shot, and raw input
  taken; the open's develop not shown, the nudge's shown;
- a frame left (two steps, with and without a develop since), the first
  open of a frame chosen again, and a return without an open (the visit
  over) take nothing; the same frame renumbered keeps its visit;
- another version of the file is not the frame;
- culling entered during the decode takes nothing;
- a frame whose file moved keeps its open and still asks develops;
- a frame whose open failed is read again on the re-read path;
- a blend seeded behind a develop develops again ("developing..."), and
  with the learned denoiser off does not;
- an offline frame and a pick reading its sidecar ask no develop (a
  slider, the Original key, a turn);
- a frame whose file moved asks no develop until it is opened again,
  which the follow does (both follows driven end to end); the old open's
  late word is not taken; what a job running on the old path says is;
- a learned mask asked before the move and made on the old path, landing
  after it, is taken, as is one made under the new path; another file's
  is not;
- two moves in a row and back (A to B to C to A): a mask made on A or B,
  a mosaic read on A, and a merge's answer on A are the frame's; another
  frame chosen, none is;
- a Part and a Landscape press asked before two moves (A to B to C, and
  A to B to A) are made and offered from an earlier path's answer;
  another file's answer leaves the press waiting;
- a move in culling: the open again is refused there, no develop is
  asked, and leaving culling sends the open under the new path;
- Cull's Move back of the frame on screen leaves no move behind for the
  frame chosen after it;
- As shot switched off after a move gives the custom white back; the way
  back from 200% follows the move.

Worker, the slot's own rules, which the open again relies on (these
pass with or without the move fix and are not its tests): a develop
waiting when the file moves, an open waiting, and an open running each
give way to, or are followed by, the open again under the new path, the
file renamed on disk; nothing is left unanswered and the frame develops
under the new path after.

On real raws (two R6 Mark II CR3s copied out of the sample folder into
a scratch folder, `--no-sidecars`, private XDG dirs, a private headless
weston): `--keys "right ]"` steps to the next frame and turns it 200 ms
later, during its 1.5 to 2.5 s decode. Before (switched back by a
temporary probe): `opened_gen=2 now_gen=3`, refused; two develops (the
open's unturned, then the turn's); the panel kept the last frame's
5606 K and no shot lines. After: the open landed at 3 and was taken,
one develop ran, turned; the panel says 6124 K and the frame's camera,
lens, exposure, and time. With the worker's fold alone switched off,
the window took `opened_gen=2 now_gen=3`. The sample folder's listing
was unchanged after.
