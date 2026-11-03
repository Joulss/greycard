//! Merge to HDR and Merge to Panorama, in the frame menu beside Stack
//! bursts: the frames chosen, a tripod bracket or a panorama's frames,
//! merged by the engine's merge (the one `greycard merge hdr` or
//! `greycard merge pano` runs) into a linear DNG beside the first of
//! them, `<first>-HDR.dng` or `<first>-pano.dng`.
//!
//! The merge reads every frame whole, seconds a frame, so it runs on a
//! thread of its own, saying each frame as it comes in the status line;
//! a second ask waits for the first and then runs. What it made lands on
//! the window's thread: the file given to the indexer, its edit the
//! reference frame's less the exposure, and the file on top of a stack
//! with its sources when they are of one folder, as a stack is. A
//! session that writes no sidecars holds the merged file's edit in
//! memory, as it holds an edit made there, until the window is closed.
//!
//! A merge that keeps its recipe and has its frames beside it is made
//! again from the Noise section's row (the engine's `merge::again`), its
//! frames denoised at the tier chosen there, on the same thread and
//! queue. The new file takes the old one's name, so its edit and its
//! sidecar stay; what the window keeps of the old file's picture goes.

#[cfg(test)]
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use greycard_edit::{Edit, Learned, Line, Placement, Sidecar};
use greycard_engine::merge::{self, Options, Step, pano};
use greycard_library::Rehash;
use greycard_library::collections::Reference;
use slint::ComponentHandle;

use crate::panel::browser::chosen_frames;
use crate::panel::collections::frames_of;
use crate::worker::Worker;
use crate::{App, Shared, State};

/// The merges asked and not yet run, and whether one is running.
/// Shared by the window's thread and the merge's own; a test's are its
/// thread's alone.
#[derive(Default)]
struct Queue {
    running: bool,
    waiting: VecDeque<Job>,
}

#[cfg(not(test))]
fn with_queue<R>(f: impl FnOnce(&mut Queue) -> R) -> R {
    static QUEUE: std::sync::Mutex<Option<Queue>> = std::sync::Mutex::new(None);
    let mut q = QUEUE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(q.get_or_insert_with(Queue::default))
}

#[cfg(test)]
fn with_queue<R>(f: impl FnOnce(&mut Queue) -> R) -> R {
    thread_local! {
        static QUEUE: RefCell<Queue> = RefCell::new(Queue::default());
    }
    QUEUE.with(|q| f(&mut q.borrow_mut()))
}

/// Which merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Hdr,
    Pano,
}

/// One merge asked: of frames chosen, or a merged file made again.
enum Job {
    Merge(Merge),
    Again(Remerge),
}

impl Job {
    /// How many frames it merges.
    fn frames(&self) -> usize {
        match self {
            Job::Merge(m) => m.files.len(),
            Job::Again(a) => a.frames,
        }
    }

    /// The merge, on whatever thread runs it; `said` takes each
    /// progress line.
    fn run(self, said: &dyn Fn(String)) -> Done {
        match self {
            Job::Merge(m) => m.run(said),
            Job::Again(a) => a.run(said),
        }
    }
}

/// Frames chosen merged: the files in the list's order, their edits
/// (the reference's is the merged file's start), their references for
/// the stack, and where a sidecar goes.
struct Merge {
    kind: Kind,
    files: Vec<PathBuf>,
    /// Each frame's edit as the window holds it, where the window has
    /// read its sidecar; none for one read from disk on the thread.
    edits: Vec<Option<Edit>>,
    /// The sources as a stack keeps them, when they are of one folder.
    stack: Option<Vec<Reference>>,
    placement: Placement,
    write_sidecars: bool,
}

/// A merged file made again from its recipe, over itself.
struct Remerge {
    file: PathBuf,
    /// The frames its recipe names, for the first status line.
    frames: usize,
    /// The reference frame, whose edit the frames are walked under, as
    /// the window holds it; none for one read from disk on the thread.
    reference: PathBuf,
    edit: Option<Box<Edit>>,
    /// The learned tier the frames are denoised at; none for none.
    tier: Option<String>,
}

/// What a merge made.
struct Done {
    frames: usize,
    made: anyhow::Result<Made>,
    lands: Lands,
}

/// Where what a merge made lands.
enum Lands {
    /// A new file beside the frames: on top of a stack of them, when
    /// they are of one folder.
    Beside { stack: Option<Vec<Reference>> },
    /// Over the merged file it was made again from: the old file's
    /// thumbnail key, its hash and stamp, read before it was replaced;
    /// none when it would not read.
    Over { old: Option<(String, u64)> },
}

struct Made {
    out: PathBuf,
    /// Its content hash, for the stack; none when it would not read.
    hash: Option<String>,
    /// Its edit, when no sidecar was written for it.
    edit: Option<Edit>,
}

/// The frame menu's Merge to HDR or Merge to Panorama: the frames
/// chosen, each file once.
pub(crate) fn ask(st: &mut State, app: &App, worker: &Worker, kind: Kind) {
    let mut cells: Vec<usize> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in chosen_frames(st) {
        let f = st.file_of(c);
        if seen.insert(f) {
            cells.push(c);
        }
    }
    if cells.len() < 2 {
        app.set_status("choose two frames or more to merge".into());
        return;
    }
    let files: Vec<PathBuf> = cells
        .iter()
        .map(|&c| st.files[st.file_of(c)].clone())
        .collect();
    // A frame whose sidecar the window has read is taken as the window
    // holds it (the one open, and every edit of a session that writes
    // no sidecars); the rest are read from disk on the merge's thread.
    let edits: Vec<Option<Edit>> = cells
        .iter()
        .map(|&c| {
            let f = st.file_of(c);
            crate::rows::file_is_loaded(st, f)
                .then(|| st.sidecars.get(f).map(|s| s.current.clone()))
                .flatten()
        })
        .collect();
    // A stack is one folder's, and a collection's list makes none.
    let folders: std::collections::HashSet<Option<PathBuf>> = cells
        .iter()
        .map(|&c| crate::stacks::folder_of(st, st.file_of(c)))
        .collect();
    let stack = (st.views.collection_frames.is_none() && folders.len() == 1)
        .then(|| {
            let found = frames_of(st, &cells);
            (found.len() == cells.len()).then(|| {
                found
                    .into_iter()
                    .zip(&cells)
                    .filter_map(|((hash, _, _), &c)| {
                        Some(Reference {
                            hash,
                            hint: crate::stacks::hint_of_file(st, st.file_of(c))?,
                            version: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
        })
        .flatten();
    let job = Job::Merge(Merge {
        kind,
        files,
        edits,
        stack,
        placement: st.window.placement,
        write_sidecars: st.window.write_sidecars,
    });
    queue(st, app, worker, job);
}

/// The Noise section's row: the open merged file made again from its
/// recipe, its frames denoised at the row's tier: the window's, never
/// written to the edit, which a merge clears of the learned tier on
/// purpose.
pub(crate) fn ask_again(st: &mut State, app: &App, worker: &Worker) {
    if !st.window.write_sidecars {
        app.set_merge_again_shown(false);
        return;
    }
    let (Some(c), Some(tag)) = (
        st.current,
        st.develop
            .frame
            .as_ref()
            .and_then(|(f, _)| f.dng.merged.clone()),
    ) else {
        return;
    };
    let file = st.path_of(c).clone();
    let Some(name) = tag.frames.get(tag.reference) else {
        return;
    };
    let reference = file.with_file_name(name);
    // As the window holds it when it has read its sidecar, as a merge
    // takes it.
    let edit = st
        .file_by_path(&reference)
        .filter(|&f| crate::rows::file_is_loaded(st, f))
        .and_then(|f| st.sidecars.get(f))
        .map(|s| Box::new(s.current.clone()));
    let tier = Learned::from_name(&app.get_merge_again_tier())
        .and_then(Learned::tier)
        .map(str::to_string);
    let job = Job::Again(Remerge {
        file,
        frames: tag.frames.len(),
        reference,
        edit,
        tier,
    });
    queue(st, app, worker, job);
}

/// The worker said whether the merge of ours open now can be made
/// again with its frames denoised: the Noise section's row shown for it
/// in a session that writes sidecars, and not on a frame of the export
/// queue's view. Its tier starts at the one the recipe ran, else off.
pub(crate) fn opened(st: &State, app: &App, can: bool) {
    let shown = can && st.window.write_sidecars && !crate::panel::queue_view::frozen(st);
    app.set_merge_again_shown(shown);
    if shown {
        let ran = st
            .develop
            .frame
            .as_ref()
            .and_then(|(f, _)| f.dng.merged.as_ref()?.recipe.as_ref()?.denoise.clone());
        let tier = ran.unwrap_or_else(|| Learned::default().name().to_string());
        app.set_merge_again_tier(tier.into());
    }
}

/// Another frame is opened: the row is the worker's to show.
pub(crate) fn left(app: &App) {
    app.set_merge_again_shown(false);
}

/// `job` run now, or after the merge in hand.
fn queue(st: &mut State, app: &App, worker: &Worker, job: Job) {
    app.set_merge_running(true);
    let start = with_queue(|q| {
        if q.running {
            q.waiting.push_back(job);
            None
        } else {
            q.running = true;
            Some(job)
        }
    });
    match start {
        Some(job) => send_off(st, app, worker, job),
        None => app.set_status(QUEUED.into()),
    }
}

/// The status line for a merge asked while another runs.
const QUEUED: &str = "these frames are merged once the merge in hand is done";

/// A merge's progress as the status line says it.
fn step_line(step: Step) -> String {
    match step {
        Step::Frame { index, of } => format!("merging frame {index} of {of}"),
        Step::Placing => "merging: placing the frames".to_string(),
        Step::Blending { index, of } => format!("merging: blending frame {index} of {of}"),
        Step::Writing => "merging: writing the file".to_string(),
    }
}

impl Merge {
    fn run(self, said: &dyn Fn(String)) -> Done {
        let frames = self.files.len();
        let made = (|| -> anyhow::Result<Made> {
            let reference = match self.kind {
                Kind::Hdr => merge::reference_of(&self.files, None)?,
                Kind::Pano => pano::reference_of(self.files.len(), None)?,
            };
            let edit_of = |k: usize| {
                self.edits[k].clone().unwrap_or_else(|| {
                    if self.write_sidecars {
                        greycard_edit_io::on_disk::read_sidecar_said(&self.files[k])
                            .0
                            .current
                            .clone()
                    } else {
                        greycard_edit::Edit::default()
                    }
                })
            };
            let edit = &edit_of(reference);
            let progress = |step: Step| said(step_line(step));
            let (out, headroom_stops, seconds, warnings, merged_edit) = match self.kind {
                Kind::Hdr => {
                    let r = merge::hdr(
                        &self.files,
                        edit,
                        &Options::default(),
                        &mut greycard_engine::walk::NoModels,
                        &progress,
                    )?;
                    (
                        r.out,
                        r.headroom_stops,
                        r.seconds,
                        r.warnings,
                        merge::edit_for_merge(edit),
                    )
                }
                Kind::Pano => {
                    // Under `edit`, the learned denoiser on each frame
                    // when it has it; `again`, walked from that frame
                    // and written over that file.
                    let run = |edit: &Edit, again: Option<(usize, PathBuf)>| {
                        let denoiser = edit.noise.tier().and_then(greycard_ai::denoiser);
                        let options = pano::Options {
                            denoiser,
                            reference: again.as_ref().map(|(k, _)| *k),
                            on_exists: match again {
                                Some(_) => greycard_core::output::OnExists::Overwrite,
                                None => Default::default(),
                            },
                            out: again.map(|(_, out)| out),
                            ..pano::Options::default()
                        };
                        match denoiser {
                            Some(_) => pano::pano(
                                &self.files,
                                edit,
                                &options,
                                &mut merge::DenoiseModels::new(),
                                &progress,
                            ),
                            None => pano::pano(
                                &self.files,
                                edit,
                                &options,
                                &mut greycard_engine::walk::NoModels,
                                &progress,
                            ),
                        }
                    };
                    let mut r = run(edit, None)?;
                    // The reference is the middle of the turn, which the
                    // list's middle need not be. One frame walks the
                    // panorama and gives it its edit: when the two
                    // frames' edits walk differently, it is merged
                    // again from the reference, under its edit.
                    let mut theirs = None;
                    if r.reference != reference {
                        let other = edit_of(r.reference);
                        if !pano::walks_alike(edit, &other) {
                            r = run(&other, Some((r.reference, r.out.clone())))?;
                        }
                        theirs = Some(other);
                    }
                    let look = pano::edit_for_pano(theirs.as_ref().unwrap_or(edit));
                    (r.out, r.headroom_stops, r.seconds, r.warnings, look)
                }
            };
            for w in &warnings {
                tracing::warn!("merge: {w}");
            }
            tracing::info!(
                "merge: {} written from {frames} frames, {headroom_stops:.2} stops of headroom, \
                 in {:.1} s",
                out.display(),
                seconds
                    .iter()
                    .find(|(n, _)| *n == "total")
                    .map_or(0.0, |(_, s)| *s)
            );
            let kept = if self.write_sidecars {
                let mut sidecar = Sidecar {
                    line: Line::of(merged_edit),
                    ..Sidecar::default()
                };
                if let Err(e) = sidecar.save_in(&out, self.placement) {
                    tracing::warn!("merge: {}: the edit not written: {e}", out.display());
                }
                None
            } else {
                Some(merged_edit)
            };
            let hash = greycard_library::hash_file(&out)
                .map_err(|e| tracing::warn!("merge: {}: {e}", out.display()))
                .ok();
            Ok(Made {
                out,
                hash,
                edit: kept,
            })
        })();
        Done {
            frames,
            made,
            lands: Lands::Beside { stack: self.stack },
        }
    }
}

impl Remerge {
    fn run(self, said: &dyn Fn(String)) -> Done {
        // Read before the file is replaced, since it cannot be after.
        let old = crate::worker::thumb_key(&self.file);
        let mut frames = self.frames;
        let made = (|| -> anyhow::Result<Made> {
            let edit = match &self.edit {
                Some(edit) => (**edit).clone(),
                None => greycard_edit_io::on_disk::read_sidecar_said(&self.reference)
                    .0
                    .current
                    .clone(),
            };
            let overrides = merge::Overrides {
                denoise: Some(self.tier.clone()),
                out: None,
            };
            let progress = |step: Step| said(step_line(step));
            let again = match self.tier {
                Some(_) => merge::again(
                    &self.file,
                    &overrides,
                    &edit,
                    &mut denoise_models(),
                    &progress,
                ),
                None => merge::again(
                    &self.file,
                    &overrides,
                    &edit,
                    &mut greycard_engine::walk::NoModels,
                    &progress,
                ),
            }?;
            let (out, warnings, seconds) = match again {
                merge::Again::Hdr(r) => {
                    frames = r.frames.len();
                    (r.out, r.warnings, r.seconds)
                }
                merge::Again::Pano(r) => {
                    frames = r.frames.len();
                    (r.out, r.warnings, r.seconds)
                }
            };
            for w in &warnings {
                tracing::warn!("merge: {w}");
            }
            tracing::info!(
                "merge: {} made again from {frames} frames, the learned tier {}, in {:.1} s",
                out.display(),
                self.tier.as_deref().unwrap_or("off"),
                seconds
                    .iter()
                    .find(|(n, _)| *n == "total")
                    .map_or(0.0, |(_, s)| *s)
            );
            let hash = greycard_library::hash_file(&out)
                .map_err(|e| tracing::warn!("merge: {}: {e}", out.display()))
                .ok();
            Ok(Made {
                out,
                hash,
                edit: None,
            })
        })();
        Done {
            frames,
            made,
            lands: Lands::Over { old },
        }
    }
}

/// The models a merge made again with a learned tier runs: the user's
/// store. A test's network is the plain demosaic.
#[cfg(not(test))]
fn denoise_models() -> merge::DenoiseModels {
    merge::DenoiseModels::new()
}

#[cfg(test)]
fn denoise_models() -> greycard_engine::testing::PlainDenoiser {
    greycard_engine::testing::PlainDenoiser
}

/// The merges on a thread of their own, each landed on the window's as
/// it is done, the next asked run after it.
#[cfg(not(test))]
fn send_off(_st: &mut State, app: &App, _worker: &Worker, job: Job) {
    app.set_status(format!("merging frame 1 of {}", job.frames()).into());
    let app_weak = app.as_weak();
    let spawned = std::thread::Builder::new()
        .name("greycard merge".into())
        .spawn(move || {
            let mut job = Some(job);
            while let Some(j) = job.take() {
                let weak = app_weak.clone();
                let said = move |line: String| {
                    let weak = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(line.into());
                        }
                    });
                };
                let done = j.run(&said);
                let weak = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let (Some(app), Some(state), Some(worker)) = (
                        weak.upgrade(),
                        crate::STATE.with(|s| s.borrow().clone()),
                        crate::WORKER.with(|w| w.borrow().clone()),
                    ) else {
                        return;
                    };
                    land(&state, &app, &worker, done);
                });
                job = with_queue(|q| {
                    let next = q.waiting.pop_front();
                    q.running = next.is_some();
                    next
                });
            }
            // After the last landing: the row is free again, unless a
            // merge asked since has started one of its own.
            let weak = app_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_merge_running(with_queue(|q| q.running));
                }
            });
        });
    if let Err(e) = spawned {
        tracing::warn!("merge: {e}");
        with_queue(|q| {
            q.running = false;
            q.waiting.clear();
        });
        app.set_merge_running(false);
    }
}

/// In a test the merge runs where it is asked, and any waiting after
/// it: there is no event loop to land it on.
#[cfg(test)]
fn send_off(st: &mut State, app: &App, _worker: &Worker, job: Job) {
    app.set_status(format!("merging frame 1 of {}", job.frames()).into());
    let done = job.run(&|line| app.set_status(line.into()));
    LANDED.with(|l| l.borrow_mut().push(done));
    let _ = st;
    with_queue(|q| {
        q.running = false;
    });
    app.set_merge_running(false);
}

#[cfg(test)]
thread_local! {
    /// What a test's merges made, for the test to land with the state
    /// let go.
    static LANDED: RefCell<Vec<Done>> = const { RefCell::new(Vec::new()) };
}

/// A merge done: the file given to the indexer and stacked on top of
/// its sources, the list read again, the status said.
fn land(state: &Rc<Shared>, app: &App, worker: &Rc<Worker>, done: Done) {
    let made = match done.made {
        Ok(made) => made,
        Err(e) => {
            tracing::warn!("merge: {e:#}");
            app.set_status(format!("not merged: {e:#}").into());
            return;
        }
    };
    let name = made
        .out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let said = format!(
        "merged {} into {name}",
        greycard_engine::queue::frames(done.frames)
    );
    let stack = match done.lands {
        Lands::Beside { stack } => stack,
        Lands::Over { old } => {
            let mut st = state.borrow_mut();
            let reopened = swapped(&mut st, app, worker, &made, old);
            // The reopen's develop says its own line, and in culling
            // the loupe's: this one goes before it.
            if reopened {
                st.develop.status_after_develop = Some((st.develop.generation, said.clone()));
            } else if let Some(c) = st.current
                && st.path_at(c) == Some(&made.out)
                && let Some(cull) = st.culling.cull.as_mut()
            {
                cull.said = Some(said.clone());
            }
            app.set_status(said.into());
            return;
        }
    };
    {
        let mut held = state.borrow_mut();
        let st: &mut State = &mut held;
        if let Some(edit) = made.edit {
            st.work.merge_edits.insert(made.out.clone(), edit);
        }
        if let Some(index) = &st.views.index {
            index.file(made.out.clone());
            st.work.merge_relist = true;
        }
        if let (Some(sources), Some(hash)) = (stack, made.hash) {
            let mut refs = vec![Reference {
                hash,
                hint: crate::stacks::hint_of_path(st, &made.out),
                version: None,
            }];
            refs.extend(sources);
            st.views.stacks.make(refs);
            crate::panel::stacks::settle(st, app, worker);
        }
    }
    crate::roots::refresh_view(state, app, worker);
    held(&mut state.borrow_mut());
    app.set_status(said.into());
}

/// A merge made again, renamed over its file: what the window kept of
/// the old file's picture let go, and nothing of its edit. Its
/// thumbnails and previews in the cache, under the old file's hash and
/// time, are taken out, and the pictures of its edit under that hash,
/// each of its cells asking for its picture again; its stack's
/// reference takes the new hash; the indexer is given it; and when it
/// is open it is opened again, its edit and its sidecar as they were.
/// Whether it was.
fn swapped(
    st: &mut State,
    app: &App,
    worker: &Worker,
    made: &Made,
    old: Option<(String, u64)>,
) -> bool {
    let path = &made.out;
    if let Some((hash, stamp)) = &old {
        let cache = worker.thumb_cache();
        if let Some(cache) = cache.lock().expect("thumbnail cache").as_mut() {
            cache.remove(hash, *stamp);
        }
    }
    let file = st.file_by_path(path);
    if let (Some(f), Some((hash, _))) = (file, &old) {
        crate::edited::rewritten(st, f, hash);
    }
    if let (Some((old, _)), Some(new)) = (&old, &made.hash)
        && old != new
    {
        let hint = file
            .and_then(|f| crate::stacks::hint_of_file(st, f))
            .unwrap_or_else(|| crate::stacks::hint_of_path(st, path));
        let rehashed = [Rehash {
            path: PathBuf::from(hint),
            old: old.clone(),
            new: new.clone(),
        }];
        if st.views.stacks.heal(crate::stacks::Found {
            moved: &[],
            folders: &[],
            rehashed: &rehashed,
        }) {
            crate::panel::stacks::settle(st, app, worker);
        }
    }
    if let Some(index) = &st.views.index {
        index.file(path.clone());
        st.work.merge_relist = true;
    }
    if let Some(c) = st.current
        && st.culling.cull.is_none()
        && st.path_at(c) == Some(path)
    {
        crate::panel::browser::open_frame(st, app, worker, c);
        return true;
    }
    false
}

/// The merged files' edits held in memory, each given to its file's
/// sidecar once the file is listed and its sidecar read; a file still
/// standing in from its row takes it when it is read.
pub(crate) fn held(st: &mut State) {
    if st.work.merge_edits.is_empty() {
        return;
    }
    let listed: Vec<(PathBuf, usize)> = st
        .work
        .merge_edits
        .keys()
        .filter_map(|p| Some((p.clone(), st.file_by_path(p)?)))
        .filter(|(_, f)| st.from_row.get(*f).is_none_or(|r| r.read))
        .collect();
    for (path, f) in listed {
        if let Some(edit) = st.work.merge_edits.remove(&path) {
            st.sidecars[f].line = Line::of(edit);
        }
    }
}

/// The indexer has written rows: a merged file among them is listed
/// now.
pub(crate) fn indexed(state: &Rc<Shared>, app: &App) {
    let relist = std::mem::take(&mut state.borrow_mut().work.merge_relist);
    if relist && let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) {
        crate::roots::refresh_view(state, app, &worker);
    }
}

pub(crate) fn install(app: &App, state: &Rc<Shared>, worker: &Rc<Worker>) {
    let (state, worker, app_weak) = (state.clone(), worker.clone(), app.as_weak());
    for kind in [Kind::Hdr, Kind::Pano] {
        let (state, worker, app_weak) = (state.clone(), worker.clone(), app_weak.clone());
        let asked = move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            ask(&mut state.borrow_mut(), &app, &worker, kind);
            #[cfg(test)]
            for done in LANDED.with(|l| std::mem::take(&mut *l.borrow_mut())) {
                land(&state, &app, &worker, done);
            }
        };
        match kind {
            Kind::Hdr => app.on_merge_hdr(asked),
            Kind::Pano => app.on_merge_pano(asked),
        }
    }
    app.on_merge_again(move || {
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        ask_again(&mut state.borrow_mut(), &app, &worker);
        #[cfg(test)]
        for done in LANDED.with(|l| std::mem::take(&mut *l.borrow_mut())) {
            land(&state, &app, &worker, done);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::roots::{View, land_sent, open_view};
    use crate::testing::{state_for, window};
    use greycard_core::merge::Noise;
    use greycard_core::merge::testing::{mosaic, write_dng};
    use slint::Model;

    /// A shoot of three bracketed frames, IMG_0 to IMG_2 (x0.26, x1,
    /// x3.8), the middle one edited (an exposure, a demosaic), indexed,
    /// on a window with the grid up over the root.
    #[test]
    fn a_bracket_is_merged_named_stacked_and_given_its_reference_edit() {
        let dir = crate::testing::scratch_dir("merge-hdr");
        let root = dir.join("photos");
        let shoot = root.join("shoot");
        std::fs::create_dir_all(&shoot).unwrap();
        let (w, h) = (192, 128);
        let gains = [1.0 / 3.8, 1.0, 3.8];
        let shutters = [(1, 400), (1, 100), (1, 25)];
        let files: Vec<PathBuf> = (0..3)
            .map(|k| {
                let p = shoot.join(format!("IMG_{k}.dng"));
                write_dng(
                    &p,
                    mosaic(w, h, gains[k], Noise::ASSUMED, 9 + k as u64),
                    w,
                    h,
                    shutters[k],
                    "Bracket",
                );
                p
            })
            .collect();
        let mut edited = Edit::default();
        edited.light.exposure = 1.5;
        edited.demosaic = greycard_edit::Demosaic::Bilinear;
        Sidecar {
            line: Line::of(edited),
            ..Sidecar::default()
        }
        .save(&files[1])
        .unwrap();
        let db = dir.join("index").join("library.sqlite");
        let mut writer = greycard_library::Library::open(&db).unwrap();
        writer.index_tree(&root, &mut |_| {}).unwrap();
        let app = window(0);
        app.window()
            .set_size(slint::LogicalSize::new(1400.0, 900.0));
        app.set_grid_open(true);
        let (state, worker) = state_for(&app, Vec::new());
        {
            let mut st = state.borrow_mut();
            // Sidecars read and written, as a user's window has them.
            st.window.write_sidecars = true;
            st.views.index_reader = Some(greycard_library::Library::open_read_only(&db).unwrap());
            st.views.sort = crate::sort::Kept::by_name();
            st.views.library.roots.add(&root).unwrap();
            let collections = dir.join(crate::collections::DIR);
            st.views.collections.dir = Some(collections.clone());
            st.views.stacks =
                crate::stacks::Store::load(greycard_library::stacks::dir_in(&collections));
        }
        open_view(&state, &app, &worker, View::Roots(None));
        land_sent(&state, &app, &worker);
        assert_eq!(app.get_thumbs().row_count(), 3);

        // One frame chosen is no bracket.
        let cell = |i: usize| {
            let st = state.borrow();
            let f = st.file_by_path(&files[i]).unwrap();
            st.own_cell(f).unwrap()
        };
        let row = |c: usize| crate::panel::browser::row_of(&state.borrow(), c).unwrap() as i32;
        app.invoke_select(row(cell(0)));
        state.borrow_mut().views.picked = vec![cell(0)];
        app.invoke_merge_hdr();
        assert_eq!(app.get_status(), "choose two frames or more to merge");

        // All three.
        let all = vec![cell(0), cell(1), cell(2)];
        state.borrow_mut().views.picked = all;
        app.invoke_merge_hdr();
        let out = shoot.join("IMG_0-HDR.dng");
        assert_eq!(app.get_status(), "merged 3 frames into IMG_0-HDR.dng");
        assert!(out.is_file());
        assert!(!with_queue(|q| q.running));

        // Its edit: the reference's, less the exposure.
        let sidecar = Sidecar::load(&out).unwrap().expect("a sidecar beside it");
        assert_eq!(sidecar.current.light.exposure, 0.0);
        assert_eq!(sidecar.current.demosaic, greycard_edit::Demosaic::Bilinear);

        // On top of a stack of its sources, in the list's order.
        let stacks = state.borrow().views.stacks.list.clone();
        assert_eq!(stacks.len(), 1);
        let names: Vec<String> = stacks[0]
            .refs
            .iter()
            .map(|r| {
                Path::new(&r.hint)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            ["IMG_0-HDR.dng", "IMG_0.dng", "IMG_1.dng", "IMG_2.dng"]
        );

        // The stack is closed, its top the merge: the list shows it
        // alone.
        assert_eq!(app.get_thumbs().row_count(), 1);

        // Opened, and merged again: beside it, numbered, and stacked
        // anew, the sources leaving the first stack.
        {
            let mut st = state.borrow_mut();
            st.views.stacks.set_open(&stacks[0].id, true);
            crate::panel::stacks::settle(&mut st, &app, &worker);
        }
        state.borrow_mut().views.picked = vec![cell(0), cell(1), cell(2)];
        app.invoke_merge_hdr();
        assert_eq!(app.get_status(), "merged 3 frames into IMG_0-HDR-1.dng");
        assert!(shoot.join("IMG_0-HDR-1.dng").is_file());
        let stacks = state.borrow().views.stacks.list.clone();
        assert_eq!(stacks.len(), 1);
        assert!(stacks[0].refs[0].hint.ends_with("IMG_0-HDR-1.dng"));

        // Asked while another runs: it waits, and says so.
        {
            let mut st = state.borrow_mut();
            st.views.stacks.set_open(&stacks[0].id, true);
            crate::panel::stacks::settle(&mut st, &app, &worker);
        }
        with_queue(|q| q.running = true);
        state.borrow_mut().views.picked = vec![cell(0), cell(1)];
        app.invoke_merge_hdr();
        assert_eq!(app.get_status(), QUEUED);
        with_queue(|q| {
            assert_eq!(q.waiting.len(), 1);
            q.waiting.clear();
            q.running = false;
        });

        // A session that writes no sidecars: none beside the file, its
        // edit held in memory and given to it once it is read.
        let all = vec![cell(0), cell(1), cell(2)];
        {
            let mut st = state.borrow_mut();
            st.window.write_sidecars = false;
            st.views.picked = all;
            // The reference edited in the session, in memory alone.
            let f = st.file_by_path(&files[1]).unwrap();
            assert!(crate::rows::load_file(&mut st, &app, f));
            st.sidecars[f].line = Line::of(Edit {
                demosaic: greycard_edit::Demosaic::Vng4,
                ..Edit::default()
            });
        }
        app.invoke_merge_hdr();
        let third = shoot.join("IMG_0-HDR-2.dng");
        assert_eq!(app.get_status(), "merged 3 frames into IMG_0-HDR-2.dng");
        assert!(Sidecar::load(&third).unwrap().is_none());
        assert!(state.borrow().work.merge_edits.contains_key(&third));
        writer.index_tree(&root, &mut |_| {}).unwrap();
        open_view(&state, &app, &worker, View::Roots(None));
        land_sent(&state, &app, &worker);
        let mut st = state.borrow_mut();
        let f = st.file_by_path(&third).expect("listed");
        assert!(crate::rows::load_file(&mut st, &app, f));
        assert!(st.work.merge_edits.is_empty());
        assert_eq!(st.sidecars[f].current.light.exposure, 0.0);
        assert_eq!(
            st.sidecars[f].current.demosaic,
            greycard_edit::Demosaic::Vng4
        );
        drop(st);

        drop(writer);
        drop(app);
        drop(worker);
        crate::testing::remove_scratch(state, &dir);
    }

    /// Two frames of a camera turned about its lens, the second chosen
    /// first and the reference edited: Merge to Panorama names the file
    /// after the first in the list, stacks it on top of its sources,
    /// and gives it the reference's edit less the exposure and the crop.
    #[test]
    fn a_panorama_is_merged_named_stacked_and_given_its_reference_edit() {
        use greycard_core::merge::testing::{mosaic_of, turned, write_dng_shot};
        let dir = crate::testing::scratch_dir("merge-pano");
        let root = dir.join("photos");
        let shoot = root.join("shoot");
        std::fs::create_dir_all(&shoot).unwrap();
        let (w, h) = (960, 640);
        let files: Vec<PathBuf> = [-0.2, 0.2]
            .iter()
            .enumerate()
            .map(|(k, &yaw)| {
                let p = shoot.join(format!("IMG_{k}.dng"));
                let samples = mosaic_of(w, h, 1.0, Noise::ASSUMED, 70 + k as u64, |x, y, c| {
                    turned(x, y, c, w, h, 800.0, yaw)
                });
                write_dng_shot(&p, samples, w, h, (1, 100), 100, "Pano", Some(30));
                p
            })
            .collect();
        // The reference of two is the first.
        let mut edited = Edit::default();
        edited.light.exposure = 0.7;
        edited.demosaic = greycard_edit::Demosaic::Bilinear;
        edited.geometry.angle = 2.0;
        Sidecar {
            line: Line::of(edited),
            ..Sidecar::default()
        }
        .save(&files[0])
        .unwrap();
        let db = dir.join("index").join("library.sqlite");
        let mut writer = greycard_library::Library::open(&db).unwrap();
        writer.index_tree(&root, &mut |_| {}).unwrap();
        let app = window(0);
        app.window()
            .set_size(slint::LogicalSize::new(1400.0, 900.0));
        app.set_grid_open(true);
        let (state, worker) = state_for(&app, Vec::new());
        {
            let mut st = state.borrow_mut();
            st.window.write_sidecars = true;
            st.views.index_reader = Some(greycard_library::Library::open_read_only(&db).unwrap());
            st.views.sort = crate::sort::Kept::by_name();
            st.views.library.roots.add(&root).unwrap();
            let collections = dir.join(crate::collections::DIR);
            st.views.collections.dir = Some(collections.clone());
            st.views.stacks =
                crate::stacks::Store::load(greycard_library::stacks::dir_in(&collections));
        }
        open_view(&state, &app, &worker, View::Roots(None));
        land_sent(&state, &app, &worker);
        assert_eq!(app.get_thumbs().row_count(), 2);
        let cell = |i: usize| {
            let st = state.borrow();
            let f = st.file_by_path(&files[i]).unwrap();
            st.own_cell(f).unwrap()
        };
        let row = |c: usize| crate::panel::browser::row_of(&state.borrow(), c).unwrap() as i32;
        app.invoke_select(row(cell(0)));
        state.borrow_mut().views.picked = vec![cell(0), cell(1)];
        app.invoke_merge_pano();
        let out = shoot.join("IMG_0-pano.dng");
        assert_eq!(app.get_status(), "merged 2 frames into IMG_0-pano.dng");
        assert!(out.is_file());
        let sidecar = Sidecar::load(&out).unwrap().expect("a sidecar beside it");
        assert_eq!(sidecar.current.light.exposure, 0.0);
        assert_eq!(sidecar.current.demosaic, greycard_edit::Demosaic::Bilinear);
        assert_eq!(sidecar.current.geometry, Default::default());
        let stacks = state.borrow().views.stacks.list.clone();
        assert_eq!(stacks.len(), 1);
        let names: Vec<String> = stacks[0]
            .refs
            .iter()
            .map(|r| {
                Path::new(&r.hint)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["IMG_0-pano.dng", "IMG_0.dng", "IMG_1.dng"]);
        assert_eq!(app.get_thumbs().row_count(), 1);
        drop(writer);
        drop(app);
        drop(worker);
        crate::testing::remove_scratch(state, &dir);
    }

    /// A bracket of three merged, its file and sidecar beside the
    /// frames and stacked on them, indexed and listed, on a window
    /// whose worker opens frames for real and says so to the test, and
    /// a merge of the old kind beside it, with no recipe.
    struct Rig {
        dir: PathBuf,
        root: PathBuf,
        shoot: PathBuf,
        files: Vec<PathBuf>,
        out: PathBuf,
        old: PathBuf,
        writer: greycard_library::Library,
        app: App,
        state: Rc<Shared>,
        worker: Rc<Worker>,
        heard: std::sync::mpsc::Receiver<crate::worker::Outcome>,
    }

    fn rig(name: &str) -> Rig {
        let dir = crate::testing::scratch_dir(name);
        let root = dir.join("photos");
        let shoot = root.join("shoot");
        std::fs::create_dir_all(&shoot).unwrap();
        let (w, h) = (192, 128);
        let gains = [1.0 / 3.8, 1.0, 3.8];
        let shutters = [(1, 400), (1, 100), (1, 25)];
        let files: Vec<PathBuf> = (0..3)
            .map(|k| {
                let p = shoot.join(format!("IMG_{k}.dng"));
                write_dng(
                    &p,
                    mosaic(w, h, gains[k], Noise::ASSUMED, 9 + k as u64),
                    w,
                    h,
                    shutters[k],
                    "Bracket",
                );
                p
            })
            .collect();
        let old = shoot.join("OLD-HDR.dng");
        greycard_core::merge::testing::write_merged(&files[0], &old);
        let db = dir.join("index").join("library.sqlite");
        let mut writer = greycard_library::Library::open(&db).unwrap();
        writer.index_tree(&root, &mut |_| {}).unwrap();
        let app = window(0);
        app.window()
            .set_size(slint::LogicalSize::new(1400.0, 900.0));
        app.set_grid_open(true);
        let (tx, heard) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let worker = Rc::new(Worker::new(move |o| {
            let _ = tx.lock().unwrap().send(o);
        }));
        worker.set_thumb_cache(Some(greycard_library::thumbs::Thumbs::at(
            dir.join("thumbs"),
            greycard_library::thumbs::DEFAULT_CAP,
        )));
        let state = crate::testing::state_with(&app, Vec::new(), worker.clone());
        {
            let mut st = state.borrow_mut();
            st.window.write_sidecars = true;
            st.views.index_reader = Some(greycard_library::Library::open_read_only(&db).unwrap());
            st.views.sort = crate::sort::Kept::by_name();
            st.views.library.roots.add(&root).unwrap();
            let collections = dir.join(crate::collections::DIR);
            st.views.collections.dir = Some(collections.clone());
            st.views.stacks =
                crate::stacks::Store::load(greycard_library::stacks::dir_in(&collections));
        }
        open_view(&state, &app, &worker, View::Roots(None));
        land_sent(&state, &app, &worker);
        let cells: Vec<usize> = {
            let st = state.borrow();
            files
                .iter()
                .map(|p| st.own_cell(st.file_by_path(p).unwrap()).unwrap())
                .collect()
        };
        let row = crate::panel::browser::row_of(&state.borrow(), cells[0]).unwrap() as i32;
        app.invoke_select(row);
        state.borrow_mut().views.picked = cells;
        app.invoke_merge_hdr();
        app.set_grid_open(false);
        let out = shoot.join("IMG_0-HDR.dng");
        assert_eq!(app.get_status(), "merged 3 frames into IMG_0-HDR.dng");
        writer.index_tree(&root, &mut |_| {}).unwrap();
        open_view(&state, &app, &worker, View::Roots(None));
        land_sent(&state, &app, &worker);
        Rig {
            dir,
            root,
            shoot,
            files,
            out,
            old,
            writer,
            app,
            state,
            worker,
            heard,
        }
    }

    impl Rig {
        /// File `path` opened, its sidecar read, and the worker's open
        /// of it taken in as the window takes it.
        fn open(&self, path: &Path) {
            let c = {
                let mut st = self.state.borrow_mut();
                let f = st.file_by_path(path).expect("listed");
                assert!(crate::rows::load_file(&mut st, &self.app, f));
                st.own_cell(f).expect("a cell of its own")
            };
            crate::panel::browser::open_frame(
                &mut self.state.borrow_mut(),
                &self.app,
                &self.worker,
                c,
            );
            assert!(!self.app.get_merge_again_shown(), "not until it is read");
            self.opened();
        }

        /// The worker's open of the frame the window opened last taken
        /// in as the window takes it: its open, its develop, and for a
        /// merge that keeps a recipe the word on making it again after
        /// them. What else it says is not this test's.
        fn opened(&self) {
            use crate::worker::Outcome;
            let (generation, path) = {
                let st = self.state.borrow();
                (
                    st.develop.generation,
                    st.path_at(st.current.unwrap()).unwrap().clone(),
                )
            };
            let asked = greycard_core::decode::merge_tag_path(&path)
                .ok()
                .flatten()
                .is_some_and(|t| t.recipe.is_some());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                assert!(std::time::Instant::now() < deadline, "waiting for the open");
                let Ok(outcome) = self
                    .heard
                    .recv_timeout(std::time::Duration::from_millis(100))
                else {
                    continue;
                };
                let last = match &outcome {
                    Outcome::Opened { generation: g, .. } if *g == generation => false,
                    Outcome::Developed { generation: g, .. } if *g == generation => !asked,
                    Outcome::MergeAgain { path: p, .. } if *p == path => true,
                    Outcome::Failed { message, .. } => panic!("{message}"),
                    _ => continue,
                };
                crate::panel::deliver::deliver(&self.app, outcome);
                if last {
                    return;
                }
            }
        }

        fn done(self) {
            crate::WORKER.with(|w| *w.borrow_mut() = None);
            crate::STATE.with(|s| *s.borrow_mut() = None);
            self.worker.stop();
            drop(self.writer);
            drop(self.app);
            drop(self.worker);
            crate::testing::remove_scratch(self.state, &self.dir);
        }
    }

    /// The Noise section's row is offered on a merge that keeps its
    /// recipe, its frames beside it, in a session that writes sidecars,
    /// once the worker has said so after the develop; and on nothing
    /// else: a merge written before the recipe, the same merge with
    /// sidecars off, or with a frame gone.
    #[test]
    fn the_row_is_offered_on_a_merge_it_can_make_again() {
        let r = rig("merge-again-row");
        let row = "Denoise the frames and merge again";
        r.app.set_collapsed_noise(false);
        r.open(&r.out);
        assert!(r.app.get_merge_again_shown());
        // On the panel, below the learned tier's Blend.
        let (at, _) = crate::testing::labeled(&r.app, row);
        let (blend, _) = crate::testing::labeled(&r.app, "Blend");
        assert!(at.y > blend.y, "{at:?} under {blend:?}");

        r.open(&r.old);
        assert!(!r.app.get_merge_again_shown());

        r.state.borrow_mut().window.write_sidecars = false;
        r.open(&r.out);
        assert!(!r.app.get_merge_again_shown());
        r.state.borrow_mut().window.write_sidecars = true;

        let aside = r.shoot.join("aside.dng");
        greycard_core::testing::rename_retry(&r.files[2], &aside);
        r.open(&r.out);
        assert!(!r.app.get_merge_again_shown());
        greycard_core::testing::rename_retry(&aside, &r.files[2]);
        r.open(&r.out);
        assert!(r.app.get_merge_again_shown());
        let _ = &r.root;
        r.done();
    }

    /// Pressed, the row makes the open merge again with its frames
    /// denoised (the network the plain demosaic), over its file: the
    /// status says the merge's lines, the file keeps its name, its
    /// sidecar, and its place on top of its stack, the old file's
    /// pictures leave the cache, and the frame is opened again under
    /// the edit it had, the done line kept before its develop's. The
    /// row's tier is the window's: the edit keeps no learned tier, and
    /// the develop has no denoise to complain of. With a frame gone it
    /// is refused in the engine's words and nothing is written.
    #[test]
    fn the_row_makes_the_merge_again_and_the_picture_reloads() {
        let r = rig("merge-again-press");
        {
            let mut sidecar = Sidecar::load(&r.out).unwrap().unwrap();
            let mut edit = sidecar.current.clone();
            edit.light.exposure = 0.4;
            sidecar.line = Line::of(edit);
            sidecar.save(&r.out).unwrap();
        }
        let sidecar = Sidecar::path_for(&r.out);
        let sidecar_before = std::fs::read(&sidecar).unwrap();
        r.open(&r.out);
        assert!(r.app.get_merge_again_shown());
        assert_eq!(r.app.get_merge_again_tier(), "off", "the recipe ran none");
        let generation = r.state.borrow().develop.generation;

        // A picture of the old file in the cache, under its key.
        let (old_hash, old_stamp) = crate::worker::thumb_key(&r.out).unwrap();
        let tag = greycard_library::thumbs::Tag {
            recipe: crate::worker::THUMB_RECIPE,
            stamp: old_stamp,
        };
        let thumb = greycard_library::thumbs::Thumb {
            width: 1,
            height: 1,
            rgb: vec![9, 9, 9],
        };
        let has = |hash: &str| {
            r.worker
                .thumb_cache()
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .has(hash, 77, tag)
        };
        r.worker
            .thumb_cache()
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .put(&old_hash, 77, tag, &thumb)
            .unwrap();
        assert!(has(&old_hash));

        r.app.set_merge_again_tier("balanced".into());
        r.app.invoke_merge_again();
        assert_eq!(r.app.get_status(), "merged 3 frames into IMG_0-HDR.dng");
        assert!(!r.app.get_merge_running());
        let made = greycard_core::decode::merge_tag_path(&r.out)
            .unwrap()
            .expect("our tag");
        assert!(made.denoised);
        assert_eq!(
            made.recipe.expect("a recipe").denoise.as_deref(),
            Some("balanced")
        );
        let names: Vec<String> = std::fs::read_dir(&r.shoot)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.') || n.contains("-HDR-"))
            .collect();
        assert!(names.is_empty(), "{names:?}");
        assert_eq!(std::fs::read(&sidecar).unwrap(), sidecar_before);
        assert!(!has(&old_hash));

        // On top of its stack under its new hash.
        let new_hash = greycard_library::hash_file(&r.out).unwrap();
        assert_ne!(new_hash, old_hash);
        let stacks = r.state.borrow().views.stacks.list.clone();
        assert_eq!(stacks.len(), 1);
        assert!(stacks[0].refs[0].hint.ends_with("IMG_0-HDR.dng"));
        assert_eq!(stacks[0].refs[0].hash, new_hash);

        // Opened again, under the edit it had.
        assert!(r.state.borrow().develop.generation > generation);
        r.opened();
        assert!(r.app.get_merge_again_shown());
        let status = r.app.get_status();
        assert!(
            status.starts_with("merged 3 frames into IMG_0-HDR.dng; ")
                && status.contains("developed in")
                && !status.contains("denoise"),
            "{status}"
        );
        assert_eq!(r.app.get_merge_again_tier(), "balanced", "the recipe's");
        assert_eq!(
            Sidecar::load(&r.out)
                .unwrap()
                .unwrap()
                .current
                .noise
                .learned,
            Learned::Off
        );
        assert_eq!(r.app.get_denoise_learned(), "off");
        {
            let st = r.state.borrow();
            assert_eq!(st.develop.edit.light.exposure, 0.4);
            assert_eq!(
                st.path_at(st.current.unwrap()).unwrap().as_path(),
                r.out.as_path()
            );
        }
        assert_eq!(std::fs::read(&sidecar).unwrap(), sidecar_before);

        // A frame gone: refused, and the file as it was.
        let before = std::fs::read(&r.out).unwrap();
        let aside = r.shoot.join("aside.dng");
        greycard_core::testing::rename_retry(&r.files[0], &aside);
        r.app.invoke_merge_again();
        assert_eq!(
            r.app.get_status(),
            format!(
                "not merged: IMG_0.dng is not beside {}: a merge is made again from its \
                 frames, in its folder",
                r.out.display()
            )
        );
        assert!(!r.app.get_merge_running());
        assert_eq!(std::fs::read(&r.out).unwrap(), before);
        greycard_core::testing::rename_retry(&aside, &r.files[0]);
        r.done();
    }
}
