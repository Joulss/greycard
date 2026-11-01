//! Stack bursts, in the frame menu beside the stacks' own actions: the
//! bursts of each folder the list shows (`greycard_library::bursts`),
//! frames seconds apart with near-identical previews, made a stack
//! each through the stacks' store, the sharpest frame on top, closed.
//! A frame already in a stack is left where it is, and splits a run it
//! stands in: the user's own grouping wins over a guess.
//!
//! The looks a folder lacks are made from its files' previews, seconds
//! for a card of raws, so the finding runs on a thread of its own with
//! a connection of its own that keeps the looks; what it found lands
//! on the window's thread, where the stacks are made from the list as
//! it is then.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use greycard_library::collections::{Reference, hint_of};

use crate::stacks::{folder_of, hint_of_file, listed};
use crate::*;

/// The threads a folder's previews are read on: a preview is a JPEG
/// decode, and four make a card of raws a few seconds' work while
/// leaving the rest of the machine to the window and its thumbnails.
const THREADS: usize = 4;

/// How long a run may go without reading a preview before a second ask
/// takes it for stuck (a share that stopped answering mid-read) and
/// starts afresh: a preview is a fraction of a second, a large one over
/// a slow share a few, and none takes this long.
const STALL: Duration = Duration::from_secs(30);

/// The run under way: its number, and when it last read a preview.
#[derive(Clone, Copy)]
struct Run {
    id: u64,
    beat: Instant,
}

/// The run under way, none for none. Shared by the window's thread and
/// the run's own; a test's run is its thread's alone, so tests running
/// side by side do not see each other's.
#[cfg(not(test))]
fn with_run<R>(f: impl FnOnce(&mut Option<Run>, &mut u64) -> R) -> R {
    static RUN: std::sync::Mutex<(Option<Run>, u64)> = std::sync::Mutex::new((None, 0));
    let mut run = RUN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (run, last) = &mut *run;
    f(run, last)
}

#[cfg(test)]
fn with_run<R>(f: impl FnOnce(&mut Option<Run>, &mut u64) -> R) -> R {
    thread_local! {
        static RUN: RefCell<(Option<Run>, u64)> = const { RefCell::new((None, 0)) };
    }
    RUN.with(|r| {
        let (run, last) = &mut *r.borrow_mut();
        f(run, last)
    })
}

/// A run held: what makes a second ask wait rather than read the same
/// previews twice. Dropped however the run ends, landed, failed, or
/// panicked, it lets the next one start; a run stuck past [`STALL`] is
/// passed over by the next ask, and its guard, when it does drop, leaves
/// the newer run's mark alone.
struct Running {
    id: u64,
}

impl Running {
    /// The run, unless another is under way and not stuck.
    fn take() -> Option<Running> {
        with_run(|run, last| {
            if let Some(r) = run {
                if r.beat.elapsed() < STALL {
                    return None;
                }
                tracing::warn!(
                    "stack bursts: the last run read nothing for {} s; starting afresh",
                    r.beat.elapsed().as_secs()
                );
            }
            *last += 1;
            *run = Some(Run {
                id: *last,
                beat: Instant::now(),
            });
            Some(Running { id: *last })
        })
    }

    /// A preview read: the run is alive.
    fn beat(&self) {
        with_run(|run, _| {
            if let Some(r) = run.as_mut().filter(|r| r.id == self.id) {
                r.beat = Instant::now();
            }
        });
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        with_run(|run, _| {
            if run.is_some_and(|r| r.id == self.id) {
                *run = None;
            }
        });
    }
}

/// Whether a run is under way.
#[cfg(test)]
fn running() -> bool {
    with_run(|run, _| run.is_some())
}

/// What the thread is asked: the index, the folders (as the index keys
/// them), and the frames already in a stack, by their paths.
struct Job {
    db: PathBuf,
    folders: Vec<PathBuf>,
    stacked: HashSet<String>,
    running: Running,
}

/// What it found: each burst's frames, the sharpest first, as a
/// stack's references want them (the path as the index keys it and the
/// content hash).
struct Done {
    bursts: Vec<Vec<(String, String)>>,
    looked: usize,
    folders: usize,
    took: Duration,
    error: Option<String>,
    /// Carried to the landing, and dropped with it or with the closure
    /// that would have landed it.
    _running: Running,
}

impl Job {
    fn run(self) -> Done {
        let started = Instant::now();
        let Job {
            db,
            folders,
            stacked,
            running,
        } = self;
        let mut done = Done {
            bursts: Vec::new(),
            looked: 0,
            folders: folders.len(),
            took: Duration::ZERO,
            error: None,
            _running: running,
        };
        let leave = |p: &Path| stacked.contains(&hint_of(p));
        let running = &done._running;
        let beat = || running.beat();
        let mut bursts = Vec::new();
        let mut looked = 0;
        let found = greycard_library::Library::open(&db).and_then(|mut lib| {
            for folder in &folders {
                let found =
                    greycard_library::bursts::bursts_in(&mut lib, folder, &leave, &beat, THREADS)?;
                looked += found.looked;
                bursts.extend(
                    found
                        .bursts
                        .into_iter()
                        .map(|b| b.into_iter().map(|r| (hint_of(&r.path), r.hash)).collect()),
                );
            }
            Ok(())
        });
        (done.bursts, done.looked) = (bursts, looked);
        if let Err(e) = found {
            done.error = Some(e.to_string());
        }
        done.took = started.elapsed();
        done
    }
}

/// The frames' paths (as a stack keeps them) whose own frame is in a
/// stack the list shows.
fn stacked(st: &State) -> HashSet<String> {
    listed(st)
        .iter()
        .flat_map(|l| &l.members)
        .filter(|m| m.version.is_none())
        .filter_map(|m| hint_of_file(st, m.file))
        .collect()
}

/// Stack bursts: asked of the folders the list shows.
pub(crate) fn ask(st: &mut State, app: &App, worker: &Worker) {
    if st.views.collection_frames.is_some() {
        app.set_status("a stack is made in a folder's list, not in a collection's".into());
        return;
    }
    let db = st.views.index_path.clone().or_else(|| {
        st.views
            .index_reader
            .as_ref()
            .map(|l| l.path().to_path_buf())
    });
    let Some(db) = db else {
        app.set_status(crate::library::READER_UNAVAILABLE.into());
        return;
    };
    let Some(running) = Running::take() else {
        tracing::info!("stack bursts: already finding them");
        return;
    };
    let folders: BTreeSet<PathBuf> = (0..st.files.len())
        .filter_map(|f| folder_of(st, f))
        .collect();
    let job = Job {
        db,
        folders: folders.into_iter().collect(),
        stacked: stacked(st),
        running,
    };
    send_off(st, app, worker, job);
}

/// The finding on a thread of its own, landed on the window's.
#[cfg(not(test))]
fn send_off(_st: &mut State, app: &App, _worker: &Worker, job: Job) {
    let app_weak = app.as_weak();
    let spawned = std::thread::Builder::new()
        .name("greycard bursts".into())
        .spawn(move || {
            let done = job.run();
            let _ = slint::invoke_from_event_loop(move || {
                let (Some(app), Some(state), Some(worker)) = (
                    app_weak.upgrade(),
                    crate::STATE.with(|s| s.borrow().clone()),
                    crate::WORKER.with(|w| w.borrow().clone()),
                ) else {
                    return;
                };
                land(&mut state.borrow_mut(), &app, &worker, done);
            });
        });
    // The job, its guard with it, is dropped with the spawn's error.
    if let Err(e) = spawned {
        tracing::warn!("stack bursts: {e}");
    }
}

/// In a test the finding runs where it is asked: there is no event loop
/// to land it on.
#[cfg(test)]
fn send_off(st: &mut State, app: &App, worker: &Worker, job: Job) {
    let done = job.run();
    land(st, app, worker, done);
}

/// The bursts found made stacks, of the frames the list still shows
/// and no stack has taken since.
fn land(st: &mut State, app: &App, worker: &Worker, done: Done) {
    if let Some(e) = &done.error {
        tracing::warn!("stack bursts: {e}");
        app.set_status(crate::library::READER_UNAVAILABLE.into());
        return;
    }
    let shown: HashMap<String, usize> = (0..st.files.len())
        .filter_map(|f| Some((hint_of_file(st, f)?, f)))
        .collect();
    let stacked = stacked(st);
    let (mut made, mut frames) = (0, 0);
    for burst in done.bursts {
        let refs: Vec<Reference> = burst
            .into_iter()
            .filter(|(hint, _)| shown.contains_key(hint) && !stacked.contains(hint))
            .map(|(hint, hash)| Reference {
                hash,
                hint,
                version: None,
            })
            .collect();
        if refs.len() > 1 {
            frames += refs.len();
            made += 1;
            st.views.stacks.make(refs);
        }
    }
    tracing::info!(
        "stack bursts: {made} made of {frames} frames over {} folders; {} previews read \
         in {} ms",
        done.folders,
        done.looked,
        done.took.as_millis()
    );
    if made > 0 {
        crate::panel::stacks::settle(st, app, worker);
    }
    app.set_status(format!("{} stacked", crate::queue::frames(frames)).into());
}

pub(crate) fn install(app: &App, state: &Rc<Shared>, worker: &Rc<Worker>) {
    let (state, worker, app_weak) = (state.clone(), worker.clone(), app.as_weak());
    app.on_stack_bursts(move || {
        if let Some(app) = app_weak.upgrade() {
            ask(&mut state.borrow_mut(), &app, &worker);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roots::{View, land_sent, open_view};
    use crate::testing::{state_for, window};
    use greycard_library::fixture::{Frame, R5, write_frame};
    use slint::Model;

    /// A run that fails, panics, or sticks lets the next one start: a
    /// failed open lands and clears the mark, a panic drops the guard,
    /// and a run that has read nothing for [`STALL`] is passed over
    /// without its later end clearing the newer run's mark.
    #[test]
    fn a_run_that_fails_panics_or_sticks_lets_the_next_one_start() {
        let dir = crate::testing::scratch_dir("stack-bursts-fail");
        let app = window(0);
        let (state, _worker) = state_for(&app, Vec::new());
        // An index whose folder is a file: it will not open.
        let file = dir.join("not-a-folder");
        std::fs::write(&file, b"").unwrap();
        state.borrow_mut().views.index_path = Some(file.join("library.sqlite"));
        for _ in 0..2 {
            app.set_status("".into());
            app.invoke_stack_bursts();
            assert_eq!(app.get_status(), crate::library::READER_UNAVAILABLE);
            assert!(!running());
        }

        let held = Running::take().unwrap();
        assert!(running());
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _held = held;
            panic!("a decoder's panic");
        }));
        assert!(panicked.is_err());
        assert!(!running());

        let stuck = Running::take().unwrap();
        assert!(Running::take().is_none(), "one run at a time");
        with_run(|run, _| run.as_mut().unwrap().beat = Instant::now() - STALL);
        let fresh = Running::take().expect("a stuck run passed over");
        drop(stuck);
        assert!(running(), "the stuck run's end leaves the new run's mark");
        fresh.beat();
        drop(fresh);
        assert!(!running());
        crate::testing::remove_dir_retry(&dir);
    }

    /// The stacks' files, each as its members' names in order.
    fn on_disk(dir: &Path) -> Vec<Vec<String>> {
        let dir = greycard_library::stacks::dir_in(&dir.join(crate::collections::DIR));
        let mut out: Vec<Vec<String>> =
            greycard_library::stacks::read_folder(&dir, greycard_library::stacks::VERSION)
                .list
                .iter()
                .map(|s| {
                    s.refs
                        .iter()
                        .map(|r| {
                            Path::new(&r.hint)
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect()
                })
                .collect();
        out.sort();
        out
    }

    /// A shoot of six frames: x0 to x4 a second apart and alike, x5 a
    /// minute on. x3 and x4 are stacked by hand first; Stack bursts
    /// then makes x0 to x2 one stack, its sharpest on top, leaves the
    /// hand's stack as it was and x5 alone, and asked again makes
    /// nothing more.
    #[test]
    fn stack_bursts_makes_a_stack_a_burst() {
        let dir = crate::testing::scratch_dir("stack-bursts");
        let root = dir.join("photos");
        let shoot = root.join("shoot");
        std::fs::create_dir_all(&shoot).unwrap();
        let times = [
            "2026:10:31 10:00:00",
            "2026:10:31 10:00:01",
            "2026:10:31 10:00:02",
            "2026:10:31 10:00:03",
            "2026:10:31 10:00:04",
            "2026:10:31 10:01:00",
        ];
        let files: Vec<PathBuf> = times
            .iter()
            .enumerate()
            .map(|(i, taken)| {
                let p = shoot.join(format!("x{i}.tif"));
                write_frame(&p, &Frame { taken, ..R5 }, i as u16 + 1);
                p
            })
            .collect();
        let db = dir.join("index").join("library.sqlite");
        let mut writer = greycard_library::Library::open(&db).unwrap();
        writer.index_tree(&root, &mut |_| {}).unwrap();
        let app = window(0);
        app.set_grid_open(true);
        let (state, worker) = state_for(&app, Vec::new());
        {
            let mut st = state.borrow_mut();
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
        assert_eq!(app.get_thumbs().row_count(), 6);

        // x3 and x4 by hand.
        {
            let mut st = state.borrow_mut();
            let hints: Vec<String> = [3, 4]
                .iter()
                .map(|&i| {
                    let f = st.file_by_path(&files[i]).unwrap();
                    hint_of_file(&st, f).unwrap()
                })
                .collect();
            let refs = hints
                .into_iter()
                .map(|hint| Reference {
                    hash: greycard_library::hash_file(Path::new(&hint)).unwrap(),
                    hint,
                    version: None,
                })
                .collect();
            st.views.stacks.make(refs);
        }

        app.invoke_stack_bursts();
        assert_eq!(app.get_status(), "3 frames stacked");
        let stacks = on_disk(&dir);
        assert_eq!(stacks.len(), 2, "{stacks:?}");
        assert_eq!(stacks[1], ["x3.tif", "x4.tif"], "the hand's stack kept");
        let mut burst = stacks[0].clone();
        burst.sort();
        assert_eq!(burst, ["x0.tif", "x1.tif", "x2.tif"]);
        // The top is the sharpest the index keeps.
        let reader = greycard_library::Library::open_read_only(&db).unwrap();
        let focus: HashMap<String, f32> = reader
            .burst_rows(&shoot)
            .unwrap()
            .into_iter()
            .filter_map(|r| {
                let name = r.path.file_name().unwrap().to_string_lossy().into_owned();
                Some((name, r.look?.focus))
            })
            .collect();
        // The stacked frames were never looked at.
        assert_eq!(focus.len(), 4, "{focus:?}");
        assert!(!focus.contains_key("x3.tif") && !focus.contains_key("x4.tif"));
        assert!(stacks[0].iter().all(|n| focus[n] <= focus[&stacks[0][0]]));
        // Closed: the grid shows the two tops and x5.
        assert_eq!(app.get_thumbs().row_count(), 3);

        // Again: nothing more to stack.
        app.invoke_stack_bursts();
        assert_eq!(app.get_status(), "0 frames stacked");
        assert_eq!(on_disk(&dir), stacks);

        drop(reader);
        drop(writer);
        state.borrow_mut().views.index_reader = None;
        drop(app);
        drop(worker);
        drop(state);
        crate::testing::remove_dir_retry(&dir);
    }
}
