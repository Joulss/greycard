//! Edit in another editor, from the frame menu, as Lightroom hands a
//! frame to one: the frame exported with its edit as a 16-bit TIFF
//! beside its file, under its name and "-edit", through the export's
//! own path as a set of one; the file handed to the app chosen on the
//! Settings sheet (`crate::edit_in` builds the command); and the file
//! watched, so that once the app has written it the index has it and
//! the list shows it beside its raw, stacked with it.
//!
//! The export is the sheet's defaults but for the format, TIFF 16-bit,
//! and the color space, which is the sheet's: the space the last export
//! went out in unless it has been changed since. The name goes through
//! the set's naming, so a file there already is never written over.
//! The frame right-clicked is the one exported, a version as itself.
//!
//! The watch is one thread over every file handed over, looking at
//! each file's time and size once a second, off the window's thread
//! since the folder may be a share that stops answering. A save is
//! taken once its stamp has held across two looks, so a writer still
//! at work is waited for; the file is hashed then, and its raw once,
//! at its first look. A file is let go once it has been gone for a
//! while, or a while after its last save was taken, and every file is
//! let go with the window.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use greycard_library::collections::Reference;

use crate::edit_in::{self as how, Platform, Stamp};
use crate::export::{Format, OnExists};
use crate::queue;
use crate::sheet::Sheet;
use crate::*;

/// How often a watched file is looked at.
const WATCH_EVERY: Duration = Duration::from_secs(1);

/// How many looks in a row a file may be gone before it is let go. A
/// look counts only when the file's folder answers, so a share that
/// has dropped out keeps its watches.
const GONE_LOOKS: u32 = 60;
/// How many times a raw that will not read is tried for its hash.
const RAW_TRIES: u8 = 3;

/// How long after its last save was taken a file is let go.
const KEEP_AFTER: Duration = Duration::from_secs(60 * 60);

/// What goes after the frame's name.
pub(crate) const SUFFIX: &str = "-edit";

/// Edit in as the window holds it.
#[derive(Default)]
pub(crate) struct EditIn {
    /// The app chosen on the Settings sheet; none offers no Edit in.
    pub(crate) app: Option<PathBuf>,
    /// The set exporting a frame for the app, until its frame is done,
    /// and that frame: its file and its version's id.
    set: Option<Arc<queue::Set>>,
    frame: Option<(PathBuf, Option<String>)>,
    /// The files handed to the app, one a name, shared with the
    /// thread that watches them; it lets go when this does.
    pub(crate) watches: Arc<Mutex<Vec<Watch>>>,
    /// Whether that thread is running.
    watching: bool,
    /// A file the app wrote was given to the indexer: the list is read
    /// again once its row is in.
    relist: bool,
}

/// A file handed to the app.
pub(crate) struct Watch {
    pub(crate) out: PathBuf,
    /// The file it was exported from, as the list names it, and the
    /// version, none for the file's own.
    pub(crate) source: PathBuf,
    pub(crate) version: Option<String>,
    /// The raw's hash, read at the file's first look.
    raw_hash: Option<String>,
    /// How many looks have tried to read the raw.
    raw_tries: u8,
    settle: how::Settle,
    /// How many looks in a row the file has been gone.
    gone: u32,
    /// When its last save was taken.
    taken: Option<Instant>,
}

/// A save of a watched file taken: the file, what it was exported
/// from, and its hash and its raw's.
pub(crate) struct Changed {
    pub(crate) out: PathBuf,
    pub(crate) source: PathBuf,
    pub(crate) version: Option<String>,
    pub(crate) raw_hash: Option<String>,
    pub(crate) edit_hash: Option<String>,
}

/// `path`'s content hash, none (and said) when it will not read.
fn hash(path: &Path) -> Option<String> {
    greycard_library::hash_file(path)
        .map_err(|e| tracing::warn!("edit in: {}: {e}", path.display()))
        .ok()
}

/// One watched file as a look found it: the file, its source, its
/// stamp (none when the file is gone, unknown when its folder does not
/// answer, as on a share that dropped out), and the raw's hash when
/// the look read it.
type Seen = (
    PathBuf,
    PathBuf,
    Option<Option<Stamp>>,
    Option<Option<String>>,
);

/// One look at every watched file: the saves that have settled, each
/// hashed, and the files gone too long or kept long enough let go. The
/// disk is asked with the list let go, so a share that is slow to
/// answer never holds the window's thread at the lock.
pub(crate) fn look(watches: &Mutex<Vec<Watch>>) -> Vec<Changed> {
    let asked: Vec<(PathBuf, PathBuf, bool)> = watches
        .lock()
        .expect("edit in watches")
        .iter()
        .map(|w| {
            let raw = w.raw_hash.is_none() && w.raw_tries < RAW_TRIES;
            (w.out.clone(), w.source.clone(), raw)
        })
        .collect();
    let seen: Vec<Seen> = asked
        .into_iter()
        .map(|(out, source, raw)| {
            let raw = raw.then(|| hash(&source));
            let stamp = match how::stamp_of(&out) {
                Some(stamp) => Some(Some(stamp)),
                None => out.parent().is_none_or(|p| p.is_dir()).then_some(None),
            };
            (out, source, stamp, raw)
        })
        .collect();
    let mut taken = Vec::new();
    {
        let mut list = watches.lock().expect("edit in watches");
        for (out, source, stamp, raw) in seen {
            // Let go, or handed over again, meanwhile.
            let Some(w) = list.iter_mut().find(|w| w.out == out && w.source == source) else {
                continue;
            };
            if let Some(raw) = raw {
                w.raw_tries += 1;
                w.raw_hash = raw;
            }
            let Some(stamp) = stamp else {
                continue;
            };
            w.gone = if stamp.is_none() { w.gone + 1 } else { 0 };
            if w.settle.look(stamp) {
                w.taken = Some(Instant::now());
                taken.push((out, w.source.clone(), w.version.clone(), w.raw_hash.clone()));
            }
        }
        list.retain(|w| {
            let keep = w.gone < GONE_LOOKS && w.taken.is_none_or(|t| t.elapsed() < KEEP_AFTER);
            if !keep {
                tracing::info!("edit in: {} no longer watched", w.out.display());
            }
            keep
        });
    }
    taken
        .into_iter()
        .map(|(out, source, version, raw_hash)| Changed {
            edit_hash: hash(&out),
            out,
            source,
            version,
            raw_hash,
        })
        .collect()
}

/// The app's name, on the Settings sheet's row and the frame menu's
/// condition; empty for none, and for one no longer there, so the menu
/// does not offer it.
pub(crate) fn show_app(app: &App, chosen: Option<&Path>) {
    let name = chosen
        .filter(|p| p.exists())
        .map_or_else(String::new, |path| {
            let entry = how::is_desktop_entry(path)
                .then(|| std::fs::read_to_string(path).ok())
                .flatten();
            how::app_name(path, entry.as_deref())
        });
    app.set_edit_in_app(name.into());
}

/// The export Edit in writes: the sheet's defaults as TIFF 16-bit, in
/// the color space the sheet holds.
pub(crate) fn settings_for(app: &App) -> crate::export::Settings {
    Sheet {
        format: Format::Tiff.name().into(),
        space: app.get_export_space().into(),
        ..Sheet::default()
    }
    .settings()
}

/// The stem the file is written under: its source's and "-edit"; a
/// version's name joins it as for any export of a version.
pub(crate) fn stem_of(source: &Path) -> String {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into());
    format!("{stem}{SUFFIX}")
}

/// Where the export of `source` (a version named `version`) goes, as
/// the set names it: beside the file, under [`stem_of`].
pub(crate) fn out_of(source: &Path, version: Option<&str>) -> PathBuf {
    queue::named_as(
        &[(source.to_path_buf(), version.map(str::to_string))],
        &[stem_of(source)],
        None,
        None,
        Format::Tiff,
        OnExists::Increment,
    )
    .into_iter()
    .next()
    .map(|(p, _)| p)
    .unwrap_or_default()
}

/// The frame menu's Edit in: the frame right-clicked exported, once its
/// sidecar is in hand.
pub(crate) fn asked(state: &Rc<Shared>, app: &App, worker: &Rc<Worker>) {
    let key = {
        let st = state.borrow();
        let Some(chosen) = &st.work.edit_in.app else {
            return;
        };
        // Gone since it was chosen: nothing written for it, and the menu
        // offers it no more.
        if !chosen.exists() {
            tracing::warn!(
                "edit in: {} is not there; nothing exported",
                chosen.display()
            );
            show_app(app, Some(chosen));
            return;
        }
        if st.work.exporting.is_some() || st.work.export_choosing {
            app.set_status("not exported: another export is under way".into());
            return;
        }
        let Some(key) = st.views.menu_file.or(st.current).and_then(|c| st.key_of(c)) else {
            return;
        };
        key
    };
    crate::rows::request(
        state,
        app,
        worker,
        vec![key.clone()],
        crate::rows::Scope::Line,
        "export",
        Box::new(move |state, app, _, at| {
            let frames: Vec<usize> = at.into_iter().flatten().collect();
            start(state, app, &key, &frames);
        }),
    );
}

/// The export of the frame `pressed`, its sidecar in hand.
fn start(state: &Rc<Shared>, app: &App, pressed: &crate::versions::Key, frames: &[usize]) {
    {
        let st = state.borrow();
        if st.work.exporting.is_some() || st.work.export_choosing {
            return;
        }
    }
    let Some((_, _, frames, _)) =
        crate::panel::deliver::pressed_frames(state, app, pressed, frames)
    else {
        return;
    };
    let Some(frame) = frames.into_iter().next() else {
        return;
    };
    let mut st = state.borrow_mut();
    let source = frame.0.clone();
    let version = frame.4.clone();
    let out = out_of(&source, version.as_ref().map(|v| v.name.as_str()));
    tracing::info!("edit in: {} to {}", source.display(), out.display());
    let settings = settings_for(app);
    let set = crate::panel::deliver::start_set(
        &mut st,
        app,
        vec![frame],
        None,
        None,
        settings,
        OnExists::Increment,
        None,
        vec![stem_of(&source)],
    );
    st.work.edit_in.frame = set.is_some().then(|| (source, version.map(|v| v.id)));
    st.work.edit_in.set = set;
}

/// A frame of `set` done: when it is Edit in's and was written, the
/// file is handed to the app and watched.
pub(crate) fn frame_done(st: &mut State, app: &App, set: &Arc<queue::Set>, done: &queue::Done) {
    let edit_in = &mut st.work.edit_in;
    if !edit_in.set.as_ref().is_some_and(|s| Arc::ptr_eq(s, set)) {
        return;
    }
    edit_in.set = None;
    let (Some((source, version)), queue::Done::Exported { path, .. }) =
        (edit_in.frame.take(), done)
    else {
        return;
    };
    let Some(chosen) = edit_in.app.clone() else {
        return;
    };
    {
        // One watch a name: an earlier one's stamp would take this
        // export for a save.
        let mut list = edit_in.watches.lock().expect("edit in watches");
        list.retain(|w| w.out != *path);
        list.push(Watch {
            out: path.clone(),
            source,
            version,
            raw_hash: None,
            raw_tries: 0,
            settle: how::Settle::from(how::stamp_of(path)),
            gone: 0,
            taken: None,
        });
    }
    if !std::mem::replace(&mut edit_in.watching, true) {
        watch(app, Arc::downgrade(&edit_in.watches));
    }
    match hand_over(&chosen, path) {
        Ok(()) => tracing::info!("edit in: {} handed to {}", path.display(), chosen.display()),
        Err(e) => tracing::warn!(
            "edit in: {} would not start for {}: {e}",
            chosen.display(),
            path.display()
        ),
    }
}

#[cfg(test)]
thread_local! {
    /// The commands a test's Edit in would have started.
    pub(crate) static LAUNCHED: RefCell<Vec<how::Launch>> = const { RefCell::new(Vec::new()) };
}

/// `file` opened in `chosen`, without waiting on it. A desktop entry
/// with no `gio` to launch it opens the file in the desktop's own app
/// for it instead.
fn hand_over(chosen: &Path, file: &Path) -> std::io::Result<()> {
    let launch = how::launch(chosen, file, Platform::this());
    #[cfg(test)]
    {
        LAUNCHED.with(|l| l.borrow_mut().push(launch));
        Ok(())
    }
    #[cfg(not(test))]
    match spawn(&launch) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && how::is_desktop_entry(chosen) => {
            tracing::warn!(
                "edit in: no {} to launch {}; the file opened with xdg-open",
                launch.program.display(),
                chosen.display()
            );
            spawn(&how::fallback(file))
        }
        other => other,
    }
}

/// `launch` started and reaped on a thread of its own, so it is never
/// left a zombie and the window never waits on it.
#[cfg(not(test))]
fn spawn(launch: &how::Launch) -> std::io::Result<()> {
    let mut cmd = launch.command();
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null());
    let mut child = crate::adapter::scrub(&mut cmd).spawn()?;
    let program = launch.program.clone();
    std::thread::spawn(move || match child.wait() {
        Ok(status) if !status.success() => {
            tracing::warn!("edit in: {} said {status}", program.display());
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("edit in: {}: {e}", program.display()),
    });
    Ok(())
}

/// The watched files looked at once a tick on a thread of its own
/// while the list is held, each save taken brought to the window's
/// thread.
fn watch(app: &App, watches: std::sync::Weak<Mutex<Vec<Watch>>>) {
    if cfg!(test) {
        return;
    }
    let weak = app.as_weak();
    let spawned = std::thread::Builder::new()
        .name("greycard edit in".into())
        .spawn(move || {
            loop {
                std::thread::sleep(WATCH_EVERY);
                let Some(list) = watches.upgrade() else {
                    return;
                };
                let taken = look(&list);
                drop(list);
                for c in taken {
                    let sent = weak.upgrade_in_event_loop(move |app| {
                        let (Some(state), Some(worker)) = (
                            crate::STATE.with(|s| s.borrow().clone()),
                            crate::WORKER.with(|w| w.borrow().clone()),
                        ) else {
                            return;
                        };
                        changed(&state, &app, &worker, c);
                    });
                    if sent.is_err() {
                        return;
                    }
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("edit in: the watch would not start: {e}");
    }
}

/// A watched file written by the app: given to the indexer, stacked
/// with its raw (after it in the raw's stack, or in a new stack of the
/// two) and the stack opened, and the list read again so it shows.
pub(crate) fn changed(state: &Rc<Shared>, app: &App, worker: &Rc<Worker>, c: Changed) {
    {
        let mut held = state.borrow_mut();
        let st: &mut State = &mut held;
        let (source, version) = (c.source, c.version);
        tracing::info!("edit in: {} written", c.out.display());
        if let Some(index) = &st.views.index {
            index.file(c.out.clone());
            st.work.edit_in.relist = true;
        }
        match (c.raw_hash, c.edit_hash) {
            (Some(raw_hash), Some(edit_hash)) => {
                let with = Reference {
                    hash: raw_hash,
                    hint: match st.file_by_path(&source) {
                        Some(f) => crate::stacks::hint_of_file(st, f)
                            .unwrap_or_else(|| crate::stacks::hint_of_path(st, &source)),
                        None => crate::stacks::hint_of_path(st, &source),
                    },
                    version,
                };
                let adding = Reference {
                    hash: edit_hash,
                    hint: crate::stacks::hint_of_path(st, &c.out),
                    version: None,
                };
                if st.views.stacks.stack_beside(with, adding).is_some() {
                    tracing::info!(
                        "edit in: {} stacked with {}",
                        c.out.display(),
                        source.display()
                    );
                    crate::panel::stacks::settle(st, app, worker);
                }
            }
            _ => tracing::warn!("edit in: {} not stacked: unread", c.out.display()),
        }
    }
    crate::roots::refresh_view(state, app, worker);
}

/// The indexer has written rows: a file the app wrote among them is
/// listed now.
pub(crate) fn indexed(state: &Rc<Shared>, app: &App) {
    let relist = std::mem::take(&mut state.borrow_mut().work.edit_in.relist);
    if relist && let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) {
        crate::roots::refresh_view(state, app, &worker);
    }
}

#[cfg(test)]
thread_local! {
    /// What the app's chooser answers in a test, in place of asking.
    pub(crate) static CHOSEN_IN_TEST: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The Settings sheet's Edit in row: the app asked of the desktop's
/// file chooser, opening where the one chosen is or where the platform
/// keeps its apps, and kept when one is picked.
fn choose(state: &Rc<Shared>, app: &App) {
    let start = state
        .borrow()
        .work
        .edit_in
        .app
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| how::apps_folder(Platform::this()));
    #[cfg(test)]
    {
        let _ = start;
        let answer = CHOSEN_IN_TEST.with(|c| c.borrow_mut().take());
        chosen(state, app, Ok(answer));
    }
    #[cfg(not(test))]
    {
        let weak = app.as_weak();
        chooser::choose_open(
            "Edit in",
            start,
            (String::new(), Vec::new()),
            move |picked| {
                let _ = weak.upgrade_in_event_loop(move |app| {
                    if let Some(state) = crate::STATE.with(|s| s.borrow().clone()) {
                        chosen(&state, &app, picked);
                    }
                });
            },
        );
    }
}

/// The chooser's answer: an app taken, shown, and written to the
/// settings; a cancel leaves the row as it was.
fn chosen(state: &Rc<Shared>, app: &App, picked: anyhow::Result<Option<PathBuf>>) {
    match picked {
        Ok(Some(path)) => {
            let mut st = state.borrow_mut();
            tracing::info!("edit in: {}", path.display());
            show_app(app, Some(&path));
            let kept = path.to_string_lossy().into_owned();
            crate::panel::crop::keep_in_file(&st, |s| s.edit_in = kept);
            st.work.edit_in.app = Some(path);
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("file chooser: {e:#}");
            app.set_settings_note(chooser::chooser_failed(&e).into());
        }
    }
}

pub(crate) fn install(app: &App, state: &Rc<Shared>, worker: &Rc<Worker>) {
    {
        let (state, worker, app_weak) = (state.clone(), worker.clone(), app.as_weak());
        app.on_menu_edit_in(move || {
            if let Some(app) = app_weak.upgrade() {
                asked(&state, &app, &worker);
            }
        });
    }
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_edit_in_choose(move || {
            if let Some(app) = app_weak.upgrade() {
                choose(&state, &app);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roots::{View, land_sent, open_view};
    use crate::testing::{folder, state_for, strip_cell, window};
    use crate::worker::Outcome;
    use greycard_library::fixture::{A7, R5, R6, write_frame};
    use slint::platform::{PointerEventButton, WindowEvent};

    /// An app named `name` made in `dir`, and chosen.
    fn choose_app(state: &Rc<Shared>, app: &App, dir: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, b"").unwrap();
        CHOSEN_IN_TEST.with(|c| *c.borrow_mut() = Some(path.clone()));
        app.invoke_edit_in_choose();
        assert_eq!(state.borrow().work.edit_in.app.as_deref(), Some(&*path));
        path
    }

    fn watches(state: &Rc<Shared>) -> Vec<(PathBuf, PathBuf)> {
        let st = state.borrow();
        let list = st.work.edit_in.watches.lock().unwrap();
        list.iter()
            .map(|w| (w.out.clone(), w.source.clone()))
            .collect()
    }

    /// The frame menu's items labeled `label`, with the menu up over
    /// the strip's first frame.
    fn menu_items(app: &App, label: &str) -> usize {
        use i_slint_backend_testing::ElementHandle;
        let position = slint::LogicalPosition::new(strip_cell(0).0, strip_cell(0).1);
        for event in [
            WindowEvent::PointerMoved { position },
            WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Right,
            },
            WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Right,
            },
        ] {
            app.window().dispatch_event(event);
        }
        assert!(app.get_menu_up());
        let n = ElementHandle::find_by_accessible_label(app, label).count();
        crate::testing::press(app, slint::platform::Key::Escape);
        n
    }

    /// Edit in is not offered until an app is chosen on the Settings
    /// sheet, whose row then names it; a cancel leaves it as it was. An
    /// app gone since is offered no more, and nothing is exported for
    /// it.
    #[test]
    fn the_menu_offers_edit_in_once_an_app_is_chosen() {
        let dir = crate::testing::scratch_dir("edit-in-menu");
        let app = window(2);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (state, _worker) = state_for(&app, folder(2));
        app.invoke_select(0);
        app.set_busy(false);
        assert_eq!(app.get_edit_in_app(), "");
        assert_eq!(menu_items(&app, "Edit in..."), 0);
        // Pressed with no app: nothing exported.
        app.invoke_menu_edit_in();
        assert!(state.borrow().work.exporting.is_none());

        let chosen = choose_app(&state, &app, &dir, "Photoshop.app");
        assert_eq!(app.get_edit_in_app(), "Photoshop");
        assert_eq!(menu_items(&app, "Edit in..."), 1);
        // A cancel keeps the app.
        CHOSEN_IN_TEST.with(|c| *c.borrow_mut() = None);
        app.invoke_edit_in_choose();
        assert_eq!(app.get_edit_in_app(), "Photoshop");

        // Uninstalled since: refused, and offered no more.
        std::fs::remove_file(&chosen).unwrap();
        app.invoke_menu_edit_in();
        assert!(state.borrow().work.exporting.is_none());
        assert_eq!(app.get_edit_in_app(), "");
        assert_eq!(menu_items(&app, "Edit in..."), 0);
        crate::testing::remove_dir_retry(&dir);
    }

    /// Edit in exports the frame on screen as a set of one: TIFF 16-bit
    /// in the sheet's color space, beside the raw under "-edit", never
    /// over a file there; the written file is handed to the app and
    /// watched, and the status line ends on the export's own line. A
    /// second Edit in under the same name is watched once.
    #[test]
    fn edit_in_exports_beside_the_raw_and_hands_the_file_over() {
        let dir = crate::testing::scratch_dir("edit-in-export");
        let app = window(2);
        let (state, _worker) = state_for(&app, folder(2));
        state.borrow_mut().current = Some(1);
        app.set_export_space("Display P3".into());
        app.set_export_format("JPEG".into());
        let gimp = choose_app(&state, &app, &dir, "gimp");
        app.invoke_menu_edit_in();
        let set = state.borrow().work.exporting.clone().expect("the set");
        assert_eq!(set.total, 1);
        assert_eq!(set.settings.format, Format::Tiff);
        assert_eq!(set.settings.space, crate::export::Space::DisplayP3);
        assert_eq!(set.settings.long_edge, None);
        assert_eq!(set.on_exists, OnExists::Increment);
        assert_eq!((set.folder.as_ref(), set.sub.as_ref()), (None, None));
        assert_eq!(set.preset, None);
        let source = state.borrow().files[1].clone();
        assert_eq!(
            out_of(&source, None),
            Path::new("/nowhere/IMG_0001-edit.greycard.tif")
        );
        assert_eq!(
            out_of(&source, Some("Warm")),
            Path::new("/nowhere/IMG_0001-edit-Warm.greycard.tif")
        );
        assert_eq!(
            state.borrow().work.edit_in.frame,
            Some((source.clone(), None))
        );
        // A second press while it runs exports nothing more.
        app.invoke_menu_edit_in();
        assert!(Arc::ptr_eq(
            state.borrow().work.exporting.as_ref().unwrap(),
            &set
        ));

        let path = PathBuf::from("/nowhere/IMG_0001-edit.greycard.tif");
        LAUNCHED.with(|l| l.borrow_mut().clear());
        let written = |set: &Arc<queue::Set>, path: &Path| {
            crate::panel::deliver::deliver(
                &app,
                Outcome::SetFrameDone {
                    set: set.clone(),
                    index: 0,
                    source: source.clone(),
                    edit: Edit::default(),
                    version: None,
                    done: queue::Done::Exported {
                        path: path.to_path_buf(),
                        seconds: 1.5,
                        note: None,
                        left_out: Vec::new(),
                    },
                },
            );
        };
        written(&set, &path);
        let launched = LAUNCHED.with(|l| l.borrow().clone());
        assert_eq!(launched, [how::launch(&gimp, &path, Platform::this())]);
        assert_eq!(watches(&state), [(path.clone(), source.clone())]);
        assert!(state.borrow().work.edit_in.set.is_none());
        let tally = queue::Tally {
            exported: 1,
            ..queue::Tally::default()
        };
        crate::panel::deliver::deliver(&app, Outcome::SetDone { set, tally });
        assert_eq!(
            app.get_status(),
            "exported IMG_0001-edit.greycard.tif in 1.50 s"
        );

        // The file deleted and Edit in again: the same name, one watch.
        app.invoke_menu_edit_in();
        let set = state
            .borrow()
            .work
            .exporting
            .clone()
            .expect("the second set");
        written(&set, &path);
        assert_eq!(LAUNCHED.with(|l| l.borrow().len()), 2);
        assert_eq!(watches(&state), [(path.clone(), source.clone())]);
        let tally = queue::Tally {
            exported: 1,
            ..queue::Tally::default()
        };
        crate::panel::deliver::deliver(&app, Outcome::SetDone { set, tally });

        // Another set's frame is never handed over.
        let set = Arc::new(queue::Set::new(
            1,
            None,
            settings_for(&app),
            OnExists::Increment,
        ));
        written(&set, Path::new("/nowhere/IMG_0001.jpg"));
        assert_eq!(LAUNCHED.with(|l| l.borrow().len()), 2);
        assert_eq!(watches(&state).len(), 1);
        crate::testing::remove_dir_retry(&dir);
    }

    struct Setup {
        dir: PathBuf,
        files: Vec<PathBuf>,
        writer: greycard_library::Library,
        app: App,
        state: Rc<Shared>,
        worker: Rc<Worker>,
    }

    /// A root of a shoot of three frames, indexed, with the grid up
    /// over it by name, its stacks kept in the test's own folder.
    fn setup(what: &str) -> Setup {
        let dir = crate::testing::scratch_dir(&format!("edit-in-{what}"));
        let root = dir.join("photos");
        let shoot = root.join("shoot");
        std::fs::create_dir_all(&shoot).unwrap();
        let files: Vec<PathBuf> = (0..3)
            .map(|i| {
                let p = shoot.join(format!("x{i}.tif"));
                write_frame(&p, [&R5, &R6, &A7][i], i as u16 + 1);
                p
            })
            .collect();
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
        Setup {
            dir,
            files,
            writer,
            app,
            state,
            worker,
        }
    }

    fn teardown(s: Setup) {
        let Setup {
            dir,
            writer,
            app,
            state,
            worker,
            ..
        } = s;
        drop(writer);
        state.borrow_mut().views.index_reader = None;
        drop(app);
        drop(worker);
        drop(state);
        crate::testing::remove_dir_retry(&dir);
    }

    /// The grid's rows: each frame's name and its stack's mark, "[3]"
    /// on a closed stack's top and "[2/3]" on an open one's members.
    fn rows(app: &App) -> Vec<String> {
        let thumbs = app.get_thumbs();
        (0..thumbs.row_count())
            .map(|r| {
                let t = thumbs.row_data(r).unwrap();
                match (t.stack, t.stack_open) {
                    (0, _) => t.name.to_string(),
                    (n, false) => format!("{} [{n}]", t.name),
                    (n, true) => format!("{} [{}/{n}]", t.name, t.stack_place),
                }
            })
            .collect()
    }

    /// Frame `i` of the shoot opened, Edit in pressed on it, and its
    /// export said written to the file the set named: the file's path.
    fn edit_in(s: &Setup, i: usize) -> PathBuf {
        let c = {
            let st = s.state.borrow();
            st.own_cell(st.file_by_path(&s.files[i]).unwrap()).unwrap()
        };
        let row = crate::panel::browser::row_of(&s.state.borrow(), c).unwrap() as i32;
        s.app.invoke_select(row);
        s.app.invoke_menu_edit_in();
        crate::rows::land_pending(&s.state, &s.app, &s.worker);
        let set = s.state.borrow().work.exporting.clone().expect("the set");
        // The test's worker writes nothing: the file is the test's.
        set.cancel();
        let out = out_of(&s.files[i], None);
        // What the worker writes: the frame as a TIFF.
        write_frame(&out, &R5, 40 + i as u16);
        crate::panel::deliver::deliver(
            &s.app,
            Outcome::SetFrameDone {
                set: set.clone(),
                index: 0,
                source: s.files[i].clone(),
                edit: Edit::default(),
                version: None,
                done: queue::Done::Exported {
                    path: out.clone(),
                    seconds: 1.0,
                    note: None,
                    left_out: Vec::new(),
                },
            },
        );
        let tally = queue::Tally {
            exported: 1,
            ..queue::Tally::default()
        };
        crate::panel::deliver::deliver(&s.app, Outcome::SetDone { set, tally });
        out
    }

    /// The app saving `out`: the watch's look sees the new stamp once
    /// and takes it at the next, the file indexed (by the test's
    /// writer, as the indexer would) in between, and the save brought
    /// to the window.
    fn saved(s: &mut Setup, out: &Path, n: u16) {
        let before = how::stamp_of(out);
        std::thread::sleep(Duration::from_millis(20));
        write_frame(out, &R6, n);
        assert_ne!(how::stamp_of(out), before, "the save is seen");
        let list = s.state.borrow().work.edit_in.watches.clone();
        assert!(look(&list).is_empty(), "seen once, not yet taken");
        let mut taken = look(&list);
        assert_eq!(taken.len(), 1, "taken once it held");
        assert!(look(&list).is_empty(), "and only once");
        s.writer.index_file(out).unwrap();
        changed(&s.state, &s.app, &s.worker, taken.remove(0));
        land_sent(&s.state, &s.app, &s.worker);
    }

    /// The file the app writes is indexed and listed beside its raw, in
    /// a new stack of the two, opened; a frame already in a stack takes
    /// it after itself in that stack. A second save stacks nothing more.
    #[test]
    fn a_saved_file_is_listed_and_stacked_with_its_raw() {
        let mut s = setup("stack");
        let apps = s.dir.join("apps");
        choose_app(&s.state, &s.app, &apps, "gimp");
        // A file of the name there already is never written over.
        let taken = s.files[2].with_file_name("x2-edit.greycard.tif");
        std::fs::write(&taken, b"").unwrap();
        assert_eq!(
            out_of(&s.files[2], None),
            s.files[2].with_file_name("x2-edit.greycard-1.tif")
        );
        std::fs::remove_file(&taken).unwrap();
        assert_eq!(rows(&s.app), ["x0.tif", "x1.tif", "x2.tif"]);

        let out = edit_in(&s, 1);
        assert_eq!(out, s.files[1].with_file_name("x1-edit.greycard.tif"));
        // Written, not yet saved by the app: the list is as it was.
        assert_eq!(rows(&s.app), ["x0.tif", "x1.tif", "x2.tif"]);
        saved(&mut s, &out, 50);
        assert_eq!(
            rows(&s.app),
            [
                "x0.tif",
                "x1.tif [1/2]",
                "x1-edit.greycard.tif [2/2]",
                "x2.tif"
            ]
        );
        // Saved again: still one stack of two.
        saved(&mut s, &out, 51);
        assert_eq!(s.state.borrow().views.stacks.list.len(), 1);
        assert_eq!(
            rows(&s.app),
            [
                "x0.tif",
                "x1.tif [1/2]",
                "x1-edit.greycard.tif [2/2]",
                "x2.tif"
            ]
        );

        // x0 and x2 stacked, closed, x0 on top: x0's file goes after it.
        {
            let mut st = s.state.borrow_mut();
            let refs = [0, 2]
                .map(|i| Reference {
                    hash: greycard_library::hash_file(&s.files[i]).unwrap(),
                    hint: crate::stacks::hint_of_path(&st, &s.files[i]),
                    version: None,
                })
                .to_vec();
            st.views.stacks.make(refs);
            crate::panel::stacks::settle(&mut st, &s.app, &s.worker);
        }
        assert_eq!(
            rows(&s.app),
            ["x0.tif [2]", "x1.tif [1/2]", "x1-edit.greycard.tif [2/2]"]
        );
        let out = edit_in(&s, 0);
        saved(&mut s, &out, 52);
        assert_eq!(
            rows(&s.app),
            [
                "x0.tif [1/3]",
                "x0-edit.greycard.tif [2/3]",
                "x2.tif [3/3]",
                "x1.tif [1/2]",
                "x1-edit.greycard.tif [2/2]"
            ]
        );
        teardown(s);
    }
}
