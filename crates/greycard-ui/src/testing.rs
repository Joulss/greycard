use std::cell::RefCell;
use std::rc::Rc;

use slint::platform::WindowEvent;
use slint::{ComponentHandle, ModelRc};

use crate::worker::Worker;
use crate::{App, Shared, State, Thumb, grid, install_callbacks};

// The helpers the render crate's tests share with these.
pub(crate) use greycard_engine::testing::{linear_dng, scratch_dir};
pub(crate) use greycard_render::testing::{bar_png, remove_dir_retry};

thread_local! {
    /// The look folder a test's removal or rename works on, in place of
    /// the user's (`panel::look_remove::store`).
    pub(crate) static LOOK_STORE: RefCell<Option<std::path::PathBuf>> =
        const { RefCell::new(None) };
}

/// The windows a test's thread has shown, hidden as the thread ends.
/// A shown window is held by the backend, and with it every callback
/// and the state and worker they hold, so none would drop: each
/// test's worker and culling threads would then sleep on for the rest
/// of the run, and macOS caps a process's threads well short of the
/// suite's thousand and more workers.
struct Shown(Vec<slint::Weak<App>>);

impl Drop for Shown {
    fn drop(&mut self) {
        for weak in self.0.drain(..) {
            // Run as the thread's locals are torn down, some of them
            // perhaps gone already: a panic here, in the hide or in the
            // drops of the App and all it holds, would abort the whole
            // run rather than fail one test. So the App is upgraded and
            // dropped inside the catch.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                if let Some(app) = weak.upgrade() {
                    let _ = app.hide();
                }
            }));
        }
    }
}

thread_local! {
    static SHOWN: RefCell<Shown> = const { RefCell::new(Shown(Vec::new())) };
}

/// An App with a folder of eleven frames on it, shown and
/// focused, on a backend that opens no window.
pub(crate) fn window(count: usize) -> App {
    // Once a thread, whether the tests run one at a time or all
    // at once: the backend is a thread's own and setting it twice
    // panics.
    thread_local! {
        static BACKEND: () = i_slint_backend_testing::init_no_event_loop();
    }
    BACKEND.with(|()| ());
    let app = App::new().expect("the window builds");
    // English, whatever the machine's locale, which Slint would pick
    // for its bundled catalogs on its own.
    slint::select_bundled_translation("").expect("the window's catalogs are bundled");
    crate::panel::prefs::show_languages(&app, "");
    let thumbs: Vec<Thumb> = (0..count)
        .map(|i| Thumb {
            name: format!("f{i:02}.CR3").into(),
            image: slint::Image::default(),
            ..Default::default()
        })
        .collect();
    crate::cells::set_rows(&app, thumbs);
    // The key table, without which no key does anything.
    crate::keys::install(&app);
    app.on_grid_columns(grid::columns);
    app.on_grid_slack(grid::slack);
    crate::panel::browser::grid_layout(&app);
    crate::panel::browser::strip_layout(&app);
    app.on_grid_tiles_top(grid::tiles_top);
    app.on_grid_common_pitch(grid::common_pitch);
    app.on_lightbox_cell(crate::lightbox::cell);
    app.on_tab_panes(crate::panes::tab);
    app.on_bar_length(grid::bar_length);
    app.on_bar_offset(grid::bar_offset);
    app.on_bar_drag(grid::bar_drag);
    app.show().expect("the window shows");
    SHOWN.with(|s| s.borrow_mut().0.push(app.as_weak()));
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app
}

/// A press and a release at a point of the window, logical pixels
/// from its top left: what a click on whatever is there does.
pub(crate) fn click(app: &App, x: f32, y: f32) {
    click_with(app, slint::platform::PointerEventButton::Left, x, y);
}

/// [`click`] with another button.
pub(crate) fn click_with(app: &App, button: slint::platform::PointerEventButton, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    for event in [
        WindowEvent::PointerMoved { position },
        WindowEvent::PointerPressed { position, button },
        WindowEvent::PointerReleased { position, button },
    ] {
        app.window().dispatch_event(event);
    }
}

/// A press at `path[0]`, a move through each point after it, and a
/// release at the last: what dragging across the window, logical
/// pixels from its top left, does. Each point is its own
/// `PointerMoved`, so a caller after a click-versus-drag threshold
/// gets to decide, on the way through, which side of it a step lands.
pub(crate) fn drag(app: &App, path: &[(f32, f32)]) {
    drag_with(app, slint::platform::PointerEventButton::Left, path);
}

/// [`drag`] with another button.
pub(crate) fn drag_with(
    app: &App,
    button: slint::platform::PointerEventButton,
    path: &[(f32, f32)],
) {
    let at = |p: (f32, f32)| slint::LogicalPosition::new(p.0, p.1);
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: at(path[0]),
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at(path[0]),
        button,
    });
    for &p in &path[1..] {
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position: at(p) });
    }
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at(*path.last().unwrap()),
        button,
    });
}

pub(crate) fn press(app: &App, key: impl Into<slint::SharedString>) {
    let text = key.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// A state over `files` and a no-op worker, wired to `app` exactly
/// as `run` wires the real one: enough to drive any of the window's
/// callbacks without a picture open or a file on disk.
pub(crate) fn state_for(app: &App, files: Vec<std::path::PathBuf>) -> (Rc<Shared>, Rc<Worker>) {
    let worker = Rc::new(Worker::new(|_| {}));
    (state_with(app, files, worker.clone()), worker)
}

/// [`state_for`] with the caller's worker: one whose outcomes the test
/// takes in itself.
pub(crate) fn state_with(
    app: &App,
    files: Vec<std::path::PathBuf>,
    worker: Rc<Worker>,
) -> Rc<Shared> {
    let state = Rc::new(Shared::new(State::empty(files, app)));
    app.set_patch_handles(ModelRc::from(state.borrow().develop.patch_handles.clone()));
    install_callbacks(app, state.clone(), worker);
    state
}

/// The current generation's develop landed, a 60x40 picture with
/// `learned` and `fills` its reports, and the status line it leaves.
pub(crate) fn land_develop(
    app: &App,
    state: &Shared,
    learned: crate::worker::LearnedReport,
    fills: crate::worker::FillReport,
) -> String {
    let generation = state.borrow().develop.generation;
    crate::panel::deliver::deliver(
        app,
        crate::worker::Outcome::Developed {
            generation,
            turn: 0,
            kind: crate::finish::Source::SCENE,
            image: crate::worker::Developed::Halves(std::sync::Arc::new(crate::worker::Halves {
                width: 60,
                height: 40,
                pixels: Vec::new(),
            })),
            guide: std::sync::Arc::new(crate::finish::Guide::NONE),
            planes: None,
            white: crate::worker::WhiteBase::IDENTITY,
            seconds: 13.7,
            sharpen: None,
            sources: Vec::new(),
            learned,
            fills,
        },
    );
    app.get_status().to_string()
}

/// A file-less state and a no-op worker: enough to drive the retouch
/// callbacks without a picture open.
pub(crate) fn retouch_state(app: &App) -> (Rc<Shared>, Rc<Worker>) {
    state_for(app, Vec::new())
}

/// A folder of `count` frames nothing has ever written, named as a
/// camera names them, in a directory that does not exist: the
/// browser reads the sidecars it is given and never the disk.
pub(crate) fn folder(count: usize) -> Vec<std::path::PathBuf> {
    (0..count)
        .map(|i| std::path::PathBuf::from("/nowhere").join(format!("IMG_{i:04}.CR3")))
        .collect()
}

/// How many elements anywhere in the window carry `label` as their
/// accessible label: whether a control the layout means to show once
/// is there once, or twice by a mistake in it.
pub(crate) fn count_labeled(app: &App, label: &str) -> usize {
    use i_slint_backend_testing::ElementHandle;
    // As `labeled`'s own first move: a control a property has just
    // brought in is built on the next event. Query only once the
    // layout has settled into the shape under test — a query before
    // a control's `if` turns true leaves later ones, even of a
    // control already there, finding nothing: call this again after
    // such a change rather than before it.
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(0.0, 0.0),
    });
    ElementHandle::find_by_accessible_label(app, label).count()
}

/// The bounds of the element a screen reader knows as `label`,
/// scrolled into the panel's view first: its top left and its size,
/// in logical pixels from the window's top left.
pub(crate) fn labeled(app: &App, label: &str) -> (slint::LogicalPosition, slint::LogicalSize) {
    use i_slint_backend_testing::ElementHandle;
    let find = || {
        // A control a property has just brought in is built on the
        // next event: a pointer move at the corner, where nothing is.
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(0.0, 0.0),
        });
        ElementHandle::find_by_accessible_label(app, label).next()
    };
    // The panel's view, measured rather than guessed: it starts under
    // the tab bar and, on the Masks tab, the Adjustments list pinned
    // above it, and ends over the footer. What sits above a control
    // depends on the machine (the note naming the profile directory,
    // the looks in the store), and a step longer than the view lets a
    // control jump from under its foot to half behind the tab bar,
    // where a click at its middle misses it. Looked up once the layout
    // has its shape, as `find` does.
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(0.0, 0.0),
    });
    let view = || {
        let view = ElementHandle::find_by_element_id(app, "App::panel-view").next()?;
        let sections = ElementHandle::find_by_element_id(app, "App::panel-sections").next()?;
        let (at, size) = (view.absolute_position(), view.size());
        // As far as the view scrolls: the sections' foot at its own,
        // where a wheel stops.
        let end = (sections.size().height - size.height).max(0.0);
        Some((at.y, at.y + size.height, end))
    };
    let (top, bottom, _) = view().unwrap_or_else(|| {
        let height = app
            .window()
            .size()
            .to_logical(app.window().scale_factor())
            .height;
        (80.0, height, 0.0)
    });
    let scroll_to = |to: f32| {
        let end = view().map_or(f32::INFINITY, |(_, _, end)| end);
        app.set_panel_scroll(to.clamp(-end, 0.0));
    };
    // The panel's scroll only builds what is in view: down it most of
    // a view at a time from the top until the control is there, then
    // to where it is well inside the view.
    scroll_to(0.0);
    for _ in 0..40 {
        if let Some(element) = find() {
            let (at, size) = (element.absolute_position(), element.size());
            // Above the view (the tab bar, the pinned list): there
            // already.
            if at.y > 80.0 && at.y + size.height <= top {
                return (at, size);
            }
            if at.y >= top && at.y + size.height <= bottom {
                return (at, size);
            }
            scroll_to(app.get_panel_scroll() - (at.y - (top + bottom) / 2.0));
            let element = find().unwrap_or_else(|| panic!("{label:?} scrolled away"));
            return (element.absolute_position(), element.size());
        }
        // Less than the view's height, so nothing falls between steps.
        scroll_to(app.get_panel_scroll() - ((bottom - top) * 0.8).max(40.0));
    }
    panic!("nothing labeled {label:?}");
}

/// The buttons a screen reader knows as `label`, top first: their top
/// left and size, in logical pixels from the window's top left. A
/// text's own label is its words, so a row whose name is drawn in it
/// is found once here rather than twice.
pub(crate) fn buttons(app: &App, label: &str) -> Vec<(slint::LogicalPosition, slint::LogicalSize)> {
    use i_slint_backend_testing::{AccessibleRole, ElementHandle};
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(0.0, 0.0),
    });
    let mut found: Vec<_> = ElementHandle::find_by_accessible_label(app, label)
        .filter(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .map(|e| (e.absolute_position(), e.size()))
        .collect();
    found.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    found
}

/// The middle of row `i`'s picture in the strip, in logical pixels
/// from the window's top left: its cell where the strip's layout puts
/// it, less the strip's scroll, in the window's foot.
pub(crate) fn strip_cell(app: &App, i: usize) -> (f32, f32) {
    let (left, width) = crate::cells::strip_layout(app)
        .place(i as i32)
        .expect("a row the strip has");
    let height = app
        .window()
        .size()
        .to_logical(app.window().scale_factor())
        .height;
    (
        left + width / 2.0 - app.get_strip_scroll(),
        height - app.get_strip_h()
            + crate::strip::PAD
            + crate::strip::RING
            + app.get_strip_photo() / 2.0,
    )
}

/// Type `text` a key at a time into whatever has the focus.
pub(crate) fn type_text(app: &App, text: &str) {
    for c in text.chars() {
        press(app, c.to_string());
    }
}

/// A test's scratch folder removed once the state has let go of what
/// it holds open under it: the index's reader is closed first, since
/// the window's callbacks keep their own handle on the state and a
/// `drop` in the test does not end it, and Windows refuses to remove
/// an open file. The removal is tried again for a while, as
/// [`rename_away`] is, for the pool's reads still in flight.
pub(crate) fn remove_scratch(state: Rc<Shared>, dir: &std::path::Path) {
    state.borrow_mut().views.index_reader = None;
    drop(state);
    remove_dir_retry(dir);
}

/// The pointer at the middle of the element a screen reader knows as
/// `label`, resting there past the hold; its place in the window.
pub(crate) fn rest_on(app: &App, label: &str) -> (f32, f32) {
    let (at, size) = labeled(app, label);
    let (x, y) = (at.x + size.width / 2.0, at.y + size.height / 2.0);
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(x, y),
    });
    slint::platform::update_timers_and_animations();
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(800));
    slint::platform::update_timers_and_animations();
    (x, y)
}

/// Timers and animations a few frames on, after a control a property
/// has just brought in is built on the next event: a sheet's layout
/// settled.
pub(crate) fn settle(app: &App) {
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(0.0, 0.0),
    });
    for _ in 0..5 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(20));
        slint::platform::update_timers_and_animations();
    }
}

/// The top and height of the text that starts with `words`; `None`
/// while it is clipped out of sight whole, as past a pane's foot.
fn line_starting(app: &App, words: &str) -> Option<(f32, f32)> {
    use i_slint_backend_testing::ElementHandle;
    ElementHandle::find_by_element_type_name(app, "Text")
        .find(|e| e.accessible_label().is_some_and(|l| l.starts_with(words)))
        .map(|e| (e.absolute_position().y, e.size().height))
}

/// A sheet's scroller, the one `width` wide (at least the first
/// number and under the second): its top left and size.
fn pane(app: &App, width: (f32, f32)) -> (slint::LogicalPosition, slint::LogicalSize) {
    use i_slint_backend_testing::ElementHandle;
    ElementHandle::find_by_element_type_name(app, "Flickable")
        .map(|e| (e.absolute_position(), e.size()))
        .find(|(p, s)| s.width >= width.0 && s.width < width.1 && p.y > 0.0)
        .expect("the sheet's pane")
}

/// Whether the line that starts with `last` lies whole inside the pane
/// `pane_width` wide, as laid out now, and where it is.
fn inside(app: &App, last: &str, pane_width: (f32, f32)) -> (bool, String) {
    let (p, s) = pane(app, pane_width);
    let foot = p.y + s.height;
    match line_starting(app, last) {
        Some((y, h)) => (
            y >= p.y - 0.5 && y + h <= foot + 0.5,
            format!("{last:?} at {y}+{h}, the pane {}..{foot}", p.y),
        ),
        None => (
            false,
            format!("{last:?} out of sight, the pane {}..{foot}", p.y),
        ),
    }
}

/// The line that starts with `last` is past the foot of the pane
/// `pane_width` wide, and the wheel over the pane brings it in: the
/// pane's content is as tall as what it draws.
pub(crate) fn wheel_brings_in(app: &App, last: &str, pane_width: (f32, f32)) {
    settle(app);
    let (whole, said) = inside(app, last, pane_width);
    assert!(!whole, "the wheel has nothing to prove: {said}");
    let (p, s) = pane(app, pane_width);
    for _ in 0..40 {
        app.window().dispatch_event(WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(p.x + 40.0, p.y + (s.height / 2.0).min(40.0)),
            delta_x: 0.0,
            delta_y: -120.0,
        });
        settle(app);
        if inside(app, last, pane_width).0 {
            return;
        }
    }
    let (_, said) = inside(app, last, pane_width);
    panic!("the wheel left {said}");
}

/// A sheet whose notes wrap keeps its last line, the text that starts
/// with `last`, in its pane `pane_width` wide: inside it in a window
/// `tall` high, room for all of the sheet, and in one 400 high the
/// wheel over the pane brings it in. A layout's preferred height, read
/// as a property, can count a wrapped note as one line, and a sheet or
/// a pane sized from it cuts its foot.
pub(crate) fn keeps_its_last_line(app: &App, last: &str, pane_width: (f32, f32), tall: f32) {
    app.window().set_size(slint::LogicalSize::new(1500.0, tall));
    settle(app);
    let (whole, said) = inside(app, last, pane_width);
    assert!(whole, "in a window {tall} high, {said}");
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 400.0));
    wheel_brings_in(app, last, pane_width);
}
