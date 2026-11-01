//! "preparing..." on the status line while a model loads or compiles
//! for longer than a moment: the runtime says when the first load
//! starts and the last ends (`greycard_ai::runtime::on_loading`), on
//! the loading thread; the word goes up once a load has run for
//! [`AFTER`], and comes down [`GRACE`] after it is ready or has
//! failed, the status line's own words showing again under it.

use std::cell::RefCell;
use std::time::Duration;

use crate::App;
use slint::ComponentHandle;

/// How long a load runs before the status line says so: shorter ones,
/// most of them, pass unremarked.
pub(crate) const AFTER: Duration = Duration::from_millis(300);

/// How long the word stays after a load ends, so a model of two
/// sessions opened one after the other (SAM's encoder, then its
/// decoder) keeps it up between them rather than flickering.
pub(crate) const GRACE: Duration = Duration::from_millis(200);

thread_local! {
    /// The wait before the word goes up, and the wait before it comes
    /// down, on the window's thread.
    static WAITS: RefCell<Option<(slint::Timer, slint::Timer)>> = const { RefCell::new(None) };
}

/// The runtime's word on loads heard by the window.
pub(crate) fn install(app: &App) {
    greycard_ai::runtime::on_loading(listener(app.as_weak(), |f| {
        // No event loop: the window has gone, and the word with it.
        let _ = slint::invoke_from_event_loop(f);
    }));
}

/// What the runtime calls on the loading thread: `post` takes the
/// change to the window's thread.
fn listener(
    app: slint::Weak<App>,
    post: impl Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static,
) -> impl Fn(bool) + Send + Sync + 'static {
    move |loading| {
        let app = app.clone();
        post(Box::new(move || {
            if let Some(app) = app.upgrade() {
                heard(&app, loading);
            }
        }));
    }
}

/// A load started (`true`) or the last ended, on the window's thread.
fn heard(app: &App, loading: bool) {
    WAITS.with(|waits| {
        let mut waits = waits.borrow_mut();
        let (up, down) = waits.get_or_insert_with(Default::default);
        let shown = app.get_preparing();
        let weak = app.as_weak();
        if loading {
            down.stop();
            if !shown {
                up.start(slint::TimerMode::SingleShot, AFTER, move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_preparing(true);
                    }
                });
            }
        } else {
            up.stop();
            if shown {
                down.start(slint::TimerMode::SingleShot, GRACE, move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_preparing(false);
                    }
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::window;
    use std::sync::mpsc;

    /// A fake loader's start and end through the listener: no word for
    /// the first moment, "preparing..." over the fill's own words
    /// after it, and those words back a grace after the model is ready;
    /// a load shorter than the moment never shows it, a second load
    /// within the grace keeps it up, and a failure takes it down as a
    /// load ready does.
    #[test]
    fn the_word_goes_up_after_a_moment_and_down_when_the_load_ends() {
        let app = window(1);
        let filling = "developing... the fill model is at work on Fill 1";
        app.set_status(filling.into());
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let tx = std::sync::Mutex::new(tx);
        let tell = listener(app.as_weak(), move |f| {
            tx.lock().unwrap().send(f).unwrap();
        });
        let tell = std::sync::Arc::new(tell);
        // The loader's thread, as the runtime's would be.
        let load = |on: bool| {
            let tell = tell.clone();
            std::thread::spawn(move || tell(on)).join().unwrap();
            for f in rx.try_iter() {
                f();
            }
        };
        let wait = |ms: u64| {
            i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(ms));
            slint::platform::update_timers_and_animations();
        };

        load(true);
        wait(AFTER.as_millis() as u64 / 2);
        assert!(!app.get_preparing(), "not for the first moment");
        assert_eq!(app.get_status_shown(), filling);
        wait(AFTER.as_millis() as u64);
        assert!(app.get_preparing());
        assert_eq!(app.get_status_shown(), "preparing...");
        assert_eq!(app.get_status(), filling, "the fill's words kept under it");
        load(false);
        assert!(app.get_preparing(), "up through the grace");
        wait(GRACE.as_millis() as u64 + 10);
        assert!(!app.get_preparing());
        assert_eq!(app.get_status_shown(), filling, "the fill's words follow");

        // Shorter than the moment: never shown, and the wait does not
        // fire later.
        load(true);
        wait(AFTER.as_millis() as u64 / 3);
        load(false);
        wait(2 * AFTER.as_millis() as u64);
        assert!(!app.get_preparing());

        // Two loads one after the other: up through both, no flicker.
        load(true);
        wait(2 * AFTER.as_millis() as u64);
        assert!(app.get_preparing());
        load(false);
        wait(GRACE.as_millis() as u64 / 2);
        load(true);
        wait(2 * GRACE.as_millis() as u64);
        assert!(app.get_preparing(), "kept up into the second load");

        // A failure ends a load as a load ready does.
        load(false);
        wait(GRACE.as_millis() as u64 + 10);
        assert!(!app.get_preparing());
    }
}
