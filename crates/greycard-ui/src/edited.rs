//! The window's side of the edit's pictures: which of its cells the
//! worker holds as edited and under what key, told as the list, the
//! saves, and the rows change, and the cells asked again when what they
//! show may have moved. The pictures themselves, and the map the worker
//! keeps of them, are `edited_pictures`'s.

use crate::edited_pictures::{Iso, is_edited};
use crate::versions::Key;

/// Cell `c`'s ISO as its file's row gave it (`rows::FromRow::iso`).
fn iso_of(st: &crate::State, c: usize) -> Iso {
    st.from_row.get(st.file_at(c)?).and_then(|r| r.iso)
}

/// Whether the window holds cell `c` as edited: by its row while its
/// file stands in from one (the index's `edited`, the version's own
/// for a version), by its line's edit once the sidecar is read, under
/// the rule the cell's edited mark goes by.
pub(crate) fn held_edited(st: &crate::State, c: usize) -> bool {
    crate::panel::browser::frame_marks(st, c).0
}

/// Cell `c`'s develop key as its row gave it, while the row stands in
/// for the sidecar (`rows::FromRow::develop_key`, the version's own for
/// a version).
fn key_of(st: &crate::State, c: usize) -> Option<u64> {
    let cell = st.cell(c)?;
    let row = st.from_row.get(cell.file).filter(|r| !r.read)?;
    match cell.version.as_deref() {
        None => row.develop_key,
        Some(id) => row
            .versions
            .iter()
            .find(|v| v.id == id)
            .and_then(|v| v.develop_key)
            .filter(|&k| k != 0),
    }
}

/// The look or profile directory may have changed under the window (a
/// fit or a refit of the camera match, a look removed or renamed, the
/// LOOK or CAMERA PROFILE section opened, which reads its directory
/// again and so picks up a file put there by hand): when what the keys
/// are made under moved, every edited frame's key is read again, its
/// cells ask for their pictures again (a frame whose key moved shows the
/// camera's until the pool has made its new ones), and the index's keys
/// are cleared to be made anew by the next pass.
pub(crate) fn looks_moved(st: &mut crate::State) {
    let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) else {
        return;
    };
    if !worker.edited().files_moved() {
        return;
    }
    let held: std::collections::HashSet<Key> = worker.edited().held_frames().into_iter().collect();
    tracing::info!(
        "the edit's pictures: the look or profile directory changed; {} edited frame(s) read again",
        held.len()
    );
    for c in 0..st.cells.len() {
        if st.key_of(c).is_some_and(|k| held.contains(&k)) {
            ask_again(st, &worker, c);
        }
    }
    if st.culling.cull.is_some() {
        crate::panel::cull::cull_refresh(st);
    }
    if let Some(indexer) = &st.views.index {
        indexer.keys_stale();
    }
}

/// What the window is doing, as the reduced develops go by it: the frame
/// open in the editor, or none while culling, in the grid (`grid`), or
/// with no frame. Said on every report of the strip's or the grid's
/// range, and on the way into culling.
pub(crate) fn note_editing(st: &mut crate::State, worker: &crate::worker::Worker, grid: bool) {
    let open = st
        .current
        .filter(|_| st.culling.cull.is_none() && !grid)
        .and_then(|c| st.key_of(c));
    let back = open.is_some() && !worker.edited().any_open();
    worker.set_open_frame(open);
    // Back from culling or the grid on the frame still open: its
    // neighbors, let go on the way out, are sent again, in the order
    // the strip has now.
    if back && let Some(c) = st.current {
        crate::panel::browser::neighbors_back(st, worker, c);
    }
}

/// A new list in the window: the worker is told which of its frames
/// are edited, and under what key where the index knows, before their
/// thumbnails are asked for.
pub(crate) fn note_list(st: &crate::State, worker: &crate::worker::Worker) {
    let list: Vec<(Key, bool, Iso, Option<u64>)> = (0..st.cells.len())
        .filter_map(|c| {
            Some((
                st.key_of(c)?,
                held_edited(st, c),
                iso_of(st, c),
                key_of(st, c),
            ))
        })
        .collect();
    worker.edited().note_all(list);
}

/// Cell `c`'s sidecar was written, or taken whole from its archive
/// copy. The open frame's edit goes to the worker, which keeps the
/// pictures of it from the develop in hand (or when that lands, or when
/// the frame is left); any other cell (rated in the browser, culled,
/// pasted onto, or joined) is held as edited or not by what its line is
/// now, its key read again, and asked again when that may change its
/// picture. A file's cells are each their own here: a save of the file
/// is said for every cell of it whose line moved
/// ([`crate::library::sidecar_written`]).
pub(crate) fn saved(st: &mut crate::State, c: usize) {
    let (Some(frame), Some(line), Some(sidecar)) = (st.key_of(c), st.line(c), st.sidecar_at(c))
    else {
        return;
    };
    let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) else {
        return;
    };
    let iso = iso_of(st, c);
    if st.current == Some(c) && st.culling.cull.is_none() {
        worker.send(crate::worker::Job::Keep {
            frame,
            edit: line.current.clone(),
            turn: sidecar.turn,
            iso,
        });
        return;
    }
    // A base of it the worker shelved under the edit before is no base
    // an open of it would take now.
    worker.edit_changed(frame.path.clone(), file_edits(st, c));
    // The line's key under the edit just written is its hint until the
    // thumbnail reads it: what the cell shows now, not before the save.
    let edited = is_edited(&frame.path, &line.current, iso);
    let key = edited.then(|| worker.edited().key_of(&line.current));
    if worker.edited().note(&frame, edited, iso, key) {
        worker.send(crate::rows::thumb_job(st, c));
    }
    // A neighbor of the open frame is made again under its new edit.
    crate::panel::browser::neighbors_moved(st);
}

/// The window's develop of the open frame landed at `turn`: when what it
/// developed is the frame's saved edit, the worker keeps its pictures
/// from it (or finds them kept already, a key and a few stats), so a
/// frame edited before its pictures were kept has them once it is
/// opened. A frame with no edit, and none held, asks nothing.
pub(crate) fn landed(st: &crate::State, turn: u8) {
    let Some(c) = st.current.filter(|_| st.culling.cull.is_none()) else {
        return;
    };
    let (Some(frame), Some(line), Some(sidecar)) = (st.key_of(c), st.line(c), st.sidecar_at(c))
    else {
        return;
    };
    // The sidecar on disk is the frame's own, and holds what was
    // developed: nothing on the panel is unsaved.
    if !crate::panel::edit::writable(st, c)
        || line.current != st.develop.edit
        || sidecar.turn != turn
    {
        return;
    }
    let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) else {
        return;
    };
    let iso = iso_of(st, c);
    if !is_edited(&frame.path, &line.current, iso) && !worker.edited().holds(&frame) {
        return;
    }
    worker.send(crate::worker::Job::Keep {
        frame,
        edit: line.current.clone(),
        turn,
        iso,
    });
}

/// What the window holds of cell `c` changed (its row came in, or its
/// sidecar was read): the worker holds it as edited or not to match, and
/// when that turned, the cell asks for its picture again.
pub(crate) fn refresh(st: &mut crate::State, c: usize) {
    let Some(frame) = st.key_of(c) else {
        return;
    };
    let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) else {
        return;
    };
    if worker
        .edited()
        .hold(&frame, held_edited(st, c), iso_of(st, c), key_of(st, c))
    {
        kept(st, &worker, &frame);
    }
}

/// [`refresh`] over every cell of file `file`: its sidecar was read, or
/// its row came in, which says something of each of its lines.
pub(crate) fn refresh_file(st: &mut crate::State, file: usize) {
    for c in st.cells_of_file(file) {
        refresh(st, c);
    }
}

/// Cell `c`'s sidecar changed on disk under the window, as its row now
/// says (another machine's save joined, a pass that found it rewritten):
/// its key is the row's when the row has one, else read again; and the
/// cell asks for its picture again.
pub(crate) fn changed(st: &mut crate::State, c: usize) {
    let Some(frame) = st.key_of(c) else {
        return;
    };
    let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) else {
        return;
    };
    // Its bases on the worker's shelf: kept only under an edit the
    // window has read; a sidecar still standing in from its row says
    // nothing of what changed, and they all go.
    worker.edit_changed(frame.path.clone(), file_edits(st, c));
    let (edited, iso) = (held_edited(st, c), iso_of(st, c));
    let moved = match key_of(st, c) {
        Some(key) => {
            worker.edited().hold(&frame, edited, iso, Some(key));
            true
        }
        None => worker.edited().note(&frame, edited, iso, None),
    };
    if moved {
        kept(st, &worker, &frame);
    }
    crate::panel::browser::neighbors_moved(st);
}

/// Every edit cell `c`'s file holds, its own and its versions', and its
/// quarter turns, as the window has read them from its sidecar: what an
/// open of any of them would make a base under. None while the sidecar
/// stands in from the file's row, which does not say.
fn file_edits(st: &crate::State, c: usize) -> Option<crate::worker::FileNow> {
    let file = st.file_at(c)?;
    if !crate::rows::file_is_loaded(st, file) {
        return None;
    }
    let sidecar = st.sidecars.get(file)?;
    Some(crate::worker::FileNow {
        edits: std::iter::once(&sidecar.line)
            .chain(sidecar.versions.iter().map(|v| &v.line))
            .map(|l| l.current.clone())
            .collect(),
        turn: sidecar.turn,
    })
}

/// The pictures `frame` shows may have changed (`Outcome::EditedKept`):
/// its cell asks for its again, and the loupe drops its copy so its
/// next refresh asks too.
pub(crate) fn kept(st: &mut crate::State, worker: &crate::worker::Worker, frame: &Key) {
    let Some(c) = st.find_cell(frame) else {
        return;
    };
    ask_again(st, worker, c);
    if st.culling.cull.is_some() {
        crate::panel::cull::cull_refresh(st);
    }
}

/// Cell `c` asks for its picture again and the loupe drops its copy
/// where it is the edit's: the develop view's stand-in, and a versions
/// compare's tile. Cull on the camera's JPEG keeps the camera's. The
/// loupe's refresh is the caller's.
fn ask_again(st: &mut crate::State, worker: &crate::worker::Worker, c: usize) {
    worker.send(crate::rows::thumb_job(st, c));
    crate::lightbox::forget(st, c);
    if let Some(cull) = st.culling.cull.as_mut()
        && cull.shows_edit(c)
    {
        cull.cache.forget(c);
    }
    if let Some(hold) = st.culling.hold.as_mut() {
        hold.cache.forget(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edited_pictures::BIG;
    use crate::edited_pictures::tests::{mean, scratch};
    use greycard_edit::Edit;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// A picture's mean, and the strip's drawn copy of it when the row
    /// has a cell: what frame `i` shows in the window.
    fn shown(st: &crate::State, app: &crate::App, i: usize) -> (f64, Option<f64>) {
        use slint::Model;
        let (_, _, rgb) = st.views.thumb_base[i].as_ref().expect("a picture held");
        let drawn = crate::panel::browser::row_of(st, i)
            .and_then(|row| app.get_thumbs().row_data(row))
            .and_then(|t| t.image.to_rgb8())
            .map(|buf| mean(buf.as_bytes()));
        (mean(rgb), drawn)
    }

    /// The window's own hooks, headless: a frame opened, brightened and
    /// saved through the panel's save shows its edit in the strip; a
    /// reset saved the same way shows the camera's; and a frame not open
    /// turned in the strip keeps its edit's picture, the turn being drawn
    /// and not kept.
    #[test]
    fn the_window_shows_a_saved_edit_and_a_turned_frame_keeps_it() {
        use crate::worker::{Job, Outcome, Worker};
        use std::time::{Duration, Instant};
        let dir = scratch("window");
        let files: Vec<PathBuf> = ["a.png", "b.png"]
            .iter()
            .map(|name| {
                let path = dir.join(name);
                image::RgbImage::from_fn(300, 200, |x, y| {
                    image::Rgb([(30 + x / 6) as u8, (40 + y / 5) as u8, 50])
                })
                .save(&path)
                .unwrap();
                path
            })
            .collect();
        let app = crate::testing::window(2);
        let (state, _) = crate::testing::state_for(&app, files.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        let worker = std::rc::Rc::new(Worker::new(move |o| {
            let _ = tx.lock().unwrap().send(o);
        }));
        worker.set_thumb_cache(Some(
            greycard_library::thumbs::Thumbs::at(
                dir.join("thumbs"),
                greycard_library::thumbs::DEFAULT_CAP,
            )
            .with_previews(BIG, greycard_library::thumbs::DEFAULT_PREVIEW_CAP),
        ));
        crate::WORKER.with(|w| *w.borrow_mut() = Some(worker.clone()));
        {
            let mut st = state.borrow_mut();
            st.window.write_sidecars = true;
            for s in &mut st.sidecars {
                s.current = Edit::for_picture();
            }
            st.current = Some(0);
            crate::panel::browser::rebuild_browser(&mut st, &app);
            note_list(&st, &worker);
            for i in 0..2 {
                worker.send(crate::rows::thumb_job(&st, i));
            }
        }
        // The window takes in what comes, as its event loop would; a
        // develop's own landing is the viewport's and not this test's.
        let developed = std::cell::Cell::new(0);
        let pump = |until: &dyn Fn(&crate::State) -> bool, what: &str| {
            let deadline = Instant::now() + Duration::from_secs(120);
            while !until(&state.borrow()) {
                assert!(Instant::now() < deadline, "waiting for {what}");
                let Ok(outcome) = rx.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                if matches!(outcome, Outcome::Developed { .. }) {
                    developed.set(developed.get() + 1);
                }
                if matches!(
                    outcome,
                    Outcome::Thumbnail { .. }
                        | Outcome::NoThumbnail { .. }
                        | Outcome::Thumbnails(_)
                        | Outcome::EditedKept { .. }
                ) {
                    crate::panel::deliver::deliver(&app, outcome);
                }
            }
        };
        pump(
            &|st| st.views.thumb_base.iter().all(Option::is_some),
            "the camera's",
        );
        let camera = shown(&state.borrow(), &app, 0);
        // Open the frame: its develop in the worker's hand.
        let opened = |path: &Path, generation: u64| {
            worker.send(Job::Open {
                path: path.to_path_buf(),
                version: None,
                edit: Edit::for_picture(),
                generation,
                seed_blend: false,
                turn: 0,
                stance_known: false,
            });
        };
        opened(&files[0], 1);
        pump(&|_| developed.get() == 1, "the develop");
        // Brightened on the panel and saved by the panel's own save.
        {
            let mut st = state.borrow_mut();
            st.sidecars[0].current.light.exposure = 1.0;
            assert!(crate::panel::edit::save_sidecar(&mut st, 0));
        }
        pump(
            &|st| shown(st, &app, 0).0 > camera.0 + 20.0,
            "the edit's picture in the strip",
        );
        let edited = shown(&state.borrow(), &app, 0);
        if let (Some(drawn), Some(before)) = (edited.1, camera.1) {
            assert!(
                drawn > before + 20.0,
                "drawn: {drawn:.1} against {before:.1}"
            );
        }
        // Reset and saved: the camera's again.
        {
            let mut st = state.borrow_mut();
            st.sidecars[0].current = Edit::for_picture();
            assert!(crate::panel::edit::save_sidecar(&mut st, 0));
        }
        pump(
            &|st| (shown(st, &app, 0).0 - camera.0).abs() < 1.0,
            "the camera's again",
        );
        // Brightened and kept again, then left for the other frame.
        {
            let mut st = state.borrow_mut();
            st.sidecars[0].current.light.exposure = 1.0;
            assert!(crate::panel::edit::save_sidecar(&mut st, 0));
        }
        pump(
            &|st| shown(st, &app, 0).0 > camera.0 + 20.0,
            "the edit's picture again",
        );
        state.borrow_mut().current = Some(1);
        opened(&files[1], 2);
        // Turned in the strip while it is not open: its sidecar written,
        // its cell asked again, and it keeps the edit's picture.
        {
            let mut st = state.borrow_mut();
            crate::panel::browser::turn_frames(&mut st, &app, &worker, &[0], 1);
            assert_eq!(st.sidecars[0].turn, 1);
            st.views.thumb_base[0] = None;
            worker.send(crate::rows::thumb_job(&st, 0));
        }
        pump(
            &|st| st.views.thumb_base[0].is_some(),
            "the turned frame's picture",
        );
        let turned = shown(&state.borrow(), &app, 0).0;
        assert!(
            (turned - edited.0).abs() < 1.0,
            "the turned frame shows its edit: {turned:.1}, the edit's {:.1}, the camera's {:.1}",
            edited.0,
            camera.0
        );
        crate::WORKER.with(|w| *w.borrow_mut() = None);
        crate::STATE.with(|s| *s.borrow_mut() = None);
        worker.stop();
        drop(state);
        crate::testing::remove_dir_retry(&dir);
    }
}
