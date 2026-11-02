//! What another launch hands the running window (`instance`): its path
//! opened as the command line opens one at the start, a file in the
//! Loupe and a folder as its view, and the window brought forward. The
//! paths go through the desktop's queue (`finder`), the Finder's way
//! in on a Mac, so one that comes before the window can develop waits
//! there as the Finder's do.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::panel::browser::{open_folder, open_paths};
use crate::worker::Worker;
use crate::{App, Shared, instance};

thread_local! {
    /// The editor serving, until the window's last moment.
    static SERVED: RefCell<Option<instance::Served>> = const { RefCell::new(None) };
}

/// Listen for other launches from now on: the window is made and the
/// event loop's queue takes what the listener's thread sends it.
pub(crate) fn serve(app: &App, instance: instance::Instance) {
    let app = app.as_weak();
    // Answered only once the window's queue has the paths: a launch
    // not answered opens a window of its own rather than lose them.
    let served = instance.serve(move |launch| {
        let app = app.clone();
        let paths = launch.paths;
        let sent = slint::invoke_from_event_loop(move || {
            if let Some(app) = app.upgrade() {
                raise(&app);
            }
            if !paths.is_empty() {
                crate::finder::arrive(paths);
            }
        });
        if let Err(e) = &sent {
            tracing::warn!("another launch's paths could not reach the window: {e}");
        }
        sent.is_ok()
    });
    SERVED.with(|s| *s.borrow_mut() = Some(served));
}

/// The socket taken away: the window is closing, and a launch from now
/// on starts an editor of its own rather than hand its path to this
/// one. Once; the next call finds nothing.
pub(crate) fn leave() {
    if let Some(mut served) = SERVED.with(|s| s.borrow_mut().take()) {
        served.leave();
    }
}

/// Paths the desktop or another launch asked to open, as the command
/// line's path opens at the start: one folder listed with the last
/// file open in it chosen, as a folder chosen by hand is; files, or
/// several paths, each listed and the first opened.
pub(crate) fn open(state: &Rc<Shared>, app: &App, worker: &Rc<Worker>, paths: &[PathBuf]) {
    match paths {
        [dir] if dir.is_dir() => open_folder(state, app, worker, dir),
        _ => open_paths(state, app, worker, paths),
    }
}

/// The window brought forward where the platform lets it: out of the
/// taskbar or the dock if minimized, focused, and the user's attention
/// asked for. On Wayland a window cannot take the focus; it asks the
/// compositor through xdg-activation, which decides whether to bring it
/// up or only mark it.
fn raise(app: &App) {
    use slint::winit_030::WinitWindowAccessor;
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::winit::window::UserAttentionType;
    let raised = app.window().with_winit_window(|w| {
        // Wayland says nothing of a minimized window, and winit warns
        // at every unminimize there.
        if w.is_minimized() == Some(true) {
            w.set_minimized(false);
        }
        w.focus_window();
        w.request_user_attention(Some(UserAttentionType::Informational));
        matches!(
            w.window_handle().map(|h| h.as_raw()),
            Ok(RawWindowHandle::Wayland(_))
        )
    });
    match raised {
        Some(true) => tracing::info!(
            "window asked forward; on Wayland the compositor decides whether it comes up"
        ),
        Some(false) => tracing::info!("window brought forward"),
        None => tracing::info!("no window yet to bring forward"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{state_for, window};

    /// A file handed over opens alone, as the command line's file
    /// does; a folder handed over lists its frames.
    #[test]
    fn a_file_handed_over_opens_alone_and_a_folder_lists_its_frames() {
        let dir = crate::testing::scratch_dir("handed");
        let dir = dunce::canonicalize(&dir).unwrap();
        for name in ["A.CR3", "B.CR3"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let app = window(0);
        let (state, worker) = state_for(&app, Vec::new());

        open(&state, &app, &worker, &[dir.join("B.CR3")]);
        crate::roots::land_sent(&state, &app, &worker);
        assert_eq!(state.borrow().files, [dir.join("B.CR3")]);

        open(&state, &app, &worker, std::slice::from_ref(&dir));
        crate::roots::land_sent(&state, &app, &worker);
        assert_eq!(state.borrow().files, [dir.join("A.CR3"), dir.join("B.CR3")]);

        crate::testing::remove_scratch(state, &dir);
    }
}
