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

#[cfg(test)]
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use greycard_edit::{Edit, Line, Placement, Sidecar};
use greycard_engine::merge::{self, Options, Step, pano};
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

/// One merge asked: the files in the list's order, their edits (the
/// reference's is the merged file's start), their references for the
/// stack, and where a sidecar goes.
struct Job {
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

/// What a merge made.
struct Done {
    frames: usize,
    made: anyhow::Result<Made>,
    stack: Option<Vec<Reference>>,
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
    let job = Job {
        kind,
        files,
        edits,
        stack,
        placement: st.window.placement,
        write_sidecars: st.window.write_sidecars,
    };
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

impl Job {
    /// The merge, on whatever thread runs it; `said` takes each
    /// progress line.
    fn run(self, said: &dyn Fn(String)) -> Done {
        let frames = self.files.len();
        let made = (|| -> anyhow::Result<Made> {
            let reference = match self.kind {
                Kind::Hdr => merge::reference_of(&self.files, None)?,
                Kind::Pano => pano::reference_of(self.files.len(), None)?,
            };
            let edit = &self.edits[reference].clone().unwrap_or_else(|| {
                greycard_edit_io::on_disk::load_sidecar_said(
                    &self.files[reference],
                    self.write_sidecars,
                )
                .0
                .current
                .clone()
            });
            let progress = |step: Step| {
                said(match step {
                    Step::Frame { index, of } => format!("merging frame {index} of {of}"),
                    Step::Placing => "merging: placing the frames".to_string(),
                    Step::Blending { index, of } => {
                        format!("merging: blending frame {index} of {of}")
                    }
                    Step::Writing => "merging: writing the file".to_string(),
                })
            };
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
                    // The learned denoiser on each frame, when the
                    // reference's edit has it.
                    let denoiser = edit.noise.tier().and_then(greycard_ai::denoiser);
                    let options = pano::Options {
                        denoiser,
                        ..pano::Options::default()
                    };
                    let r = match denoiser {
                        Some(_) => pano::pano(
                            &self.files,
                            edit,
                            &options,
                            &mut merge::DenoiseModels::new(),
                            &progress,
                        )?,
                        None => pano::pano(
                            &self.files,
                            edit,
                            &options,
                            &mut greycard_engine::walk::NoModels,
                            &progress,
                        )?,
                    };
                    (
                        r.out,
                        r.headroom_stops,
                        r.seconds,
                        r.warnings,
                        pano::edit_for_pano(edit),
                    )
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
            stack: self.stack,
        }
    }
}

/// The merges on a thread of their own, each landed on the window's as
/// it is done, the next asked run after it.
#[cfg(not(test))]
fn send_off(_st: &mut State, app: &App, _worker: &Worker, job: Job) {
    app.set_status(format!("merging frame 1 of {}", job.files.len()).into());
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
        });
    if let Err(e) = spawned {
        tracing::warn!("merge: {e}");
        with_queue(|q| {
            q.running = false;
            q.waiting.clear();
        });
    }
}

/// In a test the merge runs where it is asked, and any waiting after
/// it: there is no event loop to land it on.
#[cfg(test)]
fn send_off(st: &mut State, app: &App, _worker: &Worker, job: Job) {
    let done = job.run(&|line| app.set_status(line.into()));
    LANDED.with(|l| l.borrow_mut().push(done));
    let _ = st;
    with_queue(|q| {
        q.running = false;
    });
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
        if let (Some(sources), Some(hash)) = (done.stack, made.hash) {
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
    app.set_status(
        format!(
            "merged {} into {name}",
            greycard_engine::queue::frames(done.frames)
        )
        .into(),
    );
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
}
