//! The interface's scale: the Settings sheet's percentage of the
//! system's own scale, set as the window opens and held as it moves
//! between monitors.
//!
//! Slint's winit backend reads `SLINT_SCALE_FACTOR` as the window is
//! created, and the factor it names replaces the system's rather than
//! multiplying it: with it set, the backend takes no notice of the
//! system's scale at all, and passes over winit's scale changes. So
//! the variable alone could only offer absolute factors, which on a
//! Mac's panel at 2 would make 125 % smaller than the default. We set
//! it all the same, to our best guess of the system's scale times the
//! percentage, because it puts the window's scale in our hands: the
//! backend then leaves it alone, and the window is created and its
//! first frame drawn at that factor. Off Wayland the guess is the
//! scale the window last opened under, which the settings keep; it is
//! checked as the window's first event comes, against what winit
//! reports for the window, and corrected by dispatching Slint's
//! scale-changed event on the window, which is then sized to the
//! logical size it was fitted to. A guess that was right, the usual
//! case there, costs nothing. On Wayland winit reports 1 until the
//! compositor's scale reaches the surface, so the guess is 1 and the
//! scale-changed event that brings the real one carries the factor.
//! Each of winit's scale changes later (the window dragged to another
//! monitor) is followed the same way.
//!
//! At 100 % nothing is set and the backend follows the system as it
//! always has. A `SLINT_SCALE_FACTOR` set by hand wins over the setting.

use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};

use slint::winit_030::winit;

/// The setting's range, percent of the system's scale.
pub(crate) const LEAST: u32 = 50;
pub(crate) const MOST: u32 = 300;

/// The variable Slint's winit backend reads for the window's scale.
pub(crate) const SLINT_SCALE_FACTOR: &str = "SLINT_SCALE_FACTOR";

/// Set beside `SLINT_SCALE_FACTOR` where we set it: a greycard started
/// from anything we start inherits both, and reads the first as ours,
/// to be worked out again from its own settings, and not as set by
/// hand.
const OURS: &str = "GREYCARD_SET_SLINT_SCALE_FACTOR";

/// Whether we set `SLINT_SCALE_FACTOR` ourselves, and so take it off the
/// commands we start that let us.
static WE_SET: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The system's own scale under the window, as winit last
    /// reported it: kept in the settings at the close, for the next
    /// start's guess.
    static SYSTEM: Cell<Option<f64>> = const { Cell::new(None) };
}

/// The setting held to its range, so a hand-edited file opens at a
/// size the row shows.
pub(crate) fn held(percent: u32) -> u32 {
    percent.clamp(LEAST, MOST)
}

/// The row's text for a setting: the percentage, held.
pub(crate) fn name(percent: u32) -> String {
    held(percent).to_string()
}

/// The setting the row's text names, rounded to a whole percentage
/// and held; none for text that is not a number, which leaves the
/// setting as it was.
pub(crate) fn named(text: &str) -> Option<u32> {
    let text = text.trim().trim_end_matches('%').trim_end();
    let n = text.parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(held(n.round().clamp(0.0, f64::from(u32::MAX)) as u32))
}

/// How the window's scale is set this run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Scale {
    /// The system's own, the setting at 100 %: the backend follows it.
    System,
    /// The system's times this, from the setting.
    Times(f64),
    /// `SLINT_SCALE_FACTOR` set by hand to this: Slint's, whatever the
    /// system's.
    ByHand(f64),
}

impl Scale {
    /// Slint's factor on a monitor whose own scale is `own`.
    pub(crate) fn on(self, own: f64) -> f64 {
        match self {
            Scale::System => own,
            Scale::Times(times) => own * times,
            Scale::ByHand(factor) => factor,
        }
    }
}

/// What the start does to the environment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Env {
    /// Nothing.
    Leave,
    /// `SLINT_SCALE_FACTOR` set to this, with the marker beside it.
    Set(f64),
    /// Both taken off: ours, inherited, and the setting is at 100 %.
    Remove,
}

/// The decision, as a pure function: the setting's percentage, the
/// system's scale the window last opened under (none, or zero, for one
/// never seen, taken as 1), and the environment `var` reads. On
/// Wayland the scale seen is passed over: winit reports 1 there until
/// the compositor's scale comes, as an event that sets the factor.
pub(crate) fn decide(
    percent: u32,
    seen: f64,
    var: impl Fn(&str) -> Option<OsString>,
) -> (Scale, Env) {
    let wayland = wayland_in(&var);
    let ours = var(OURS).is_some();
    if let Some(value) = var(SLINT_SCALE_FACTOR).filter(|_| !ours) {
        // By hand: Slint reads it, and passes over the system's changes
        // whether it parses or not, so it is left to do so.
        let factor = value
            .to_str()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|f| f.is_finite() && *f > 0.0);
        return match factor {
            Some(f) => (Scale::ByHand(f), Env::Leave),
            None => (Scale::System, Env::Leave),
        };
    }
    let percent = held(percent);
    if percent == 100 {
        let env = if ours { Env::Remove } else { Env::Leave };
        return (Scale::System, env);
    }
    let times = f64::from(percent) / 100.0;
    let seen = if !wayland && seen.is_finite() && seen > 0.0 {
        seen
    } else {
        1.0
    };
    (Scale::Times(times), Env::Set(seen * times))
}

/// Whether winit will run on Wayland, from what it reads to choose:
/// `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` set and not empty, unless
/// `SLINT_BACKEND` names X11 or the system is WSL, where Slint takes
/// X11 itself. A wrong answer costs a correction at the first event.
fn wayland_in(var: &impl Fn(&str) -> Option<OsString>) -> bool {
    if !cfg!(all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        ))
    )) {
        return false;
    }
    let set = |name: &str| var(name).is_some_and(|v| !v.is_empty());
    let x11_asked = var("SLINT_BACKEND")
        .and_then(|v| v.into_string().ok())
        .is_some_and(|v| v.to_ascii_lowercase().contains("x11"));
    let wsl = std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
        || std::path::Path::new("/run/WSL").exists();
    (set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET")) && !x11_asked && !wsl
}

/// The logical size a window nothing fits opens at (on Wayland, where
/// the compositor places it, or with no monitor reported): under the
/// setting, the size asked for over the percentage, so it covers what
/// it covers at 100 %, though no less than the layout's least.
pub(crate) fn unfitted(scale: Scale, asked: (f32, f32), least: (f32, f32)) -> (f32, f32) {
    let Scale::Times(times) = scale else {
        return asked;
    };
    let times = times as f32;
    (
        (asked.0 / times).max(least.0).round(),
        (asked.1 / times).max(least.1).round(),
    )
}

/// The scale this run, from the settings, with the environment set to
/// match. Called on the window's thread before Slint's backend is
/// selected, once.
pub(crate) fn start(percent: u32, seen: f64) -> Scale {
    let (scale, env) = decide(percent, seen, |name| std::env::var_os(name));
    match env {
        Env::Leave => {}
        // SAFETY: on the window's thread before Slint's backend is
        // selected, where `adapter::probe` sets its variable too, and
        // earlier than it: before the event loop, the worker, any pool
        // of ours, or any wgpu instance starts, so no thread of ours
        // reads the environment meanwhile: a breakpoint here found the
        // main thread alone, with a raw named to open. Slint reads it
        // next, on this thread, as it creates the window.
        Env::Set(factor) => unsafe {
            std::env::set_var(SLINT_SCALE_FACTOR, factor_text(factor));
            std::env::set_var(OURS, "1");
            WE_SET.store(true, Ordering::Relaxed);
        },
        // SAFETY: as above.
        Env::Remove => unsafe {
            std::env::remove_var(SLINT_SCALE_FACTOR);
            std::env::remove_var(OURS);
        },
    }
    match scale {
        Scale::System => {}
        Scale::Times(times) => tracing::info!(
            "interface scale {:.0}% of the system's, opening at {}",
            times * 100.0,
            std::env::var(SLINT_SCALE_FACTOR).unwrap_or_default()
        ),
        Scale::ByHand(factor) => tracing::info!(
            "{SLINT_SCALE_FACTOR}={factor} set by hand; the interface scale setting waits"
        ),
    }
    scale
}

/// A factor as the variable holds it: four places at most, so a guess
/// of 0.8 times 1.5 reads 1.2 in the log and in a child's environment.
fn factor_text(factor: f64) -> String {
    let text = format!("{factor:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A command we start, without the variable where we set it.
pub(crate) fn scrub(command: &mut std::process::Command) -> &mut std::process::Command {
    if WE_SET.load(Ordering::Relaxed) {
        command.env_remove(SLINT_SCALE_FACTOR).env_remove(OURS);
    }
    command
}

/// The system's own scale under the window as winit last reported it,
/// none before the window's first event.
pub(crate) fn system_seen() -> Option<f64> {
    SYSTEM.with(Cell::get)
}

/// The factor a window should have and whether it differs from the
/// one it has: `own`, winit's scale for it, under `scale`.
pub(crate) fn wanted(scale: Scale, own: f64, now: f32) -> Option<f32> {
    let Scale::Times(_) = scale else {
        return None;
    };
    let want = scale.on(own) as f32;
    ((want - now).abs() > 1e-4).then_some(want)
}

/// Follows winit's scale for each window: notes it for the next
/// start, and under a setting other than 100 % sets Slint's factor
/// to it times the setting.
pub(crate) struct Follow {
    scale: Scale,
    met: HashSet<winit::window::WindowId>,
    /// The logical size the fit opened the window at, or the one it
    /// asked for: what a correction at its first event sets it to.
    planned: Option<slint::LogicalSize>,
    /// The first event's correction, held while the window opened
    /// maximized or full screen: set once it is first normal, so
    /// un-maximizing gives the size it was fitted to.
    owed: Option<Owed>,
    /// The layout's least, logical.
    least: Option<slint::LogicalSize>,
}

/// A correction owed: the size, and whether the platform has shown the
/// window maximized or full screen since.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Owed {
    size: slint::LogicalSize,
    seen: bool,
}

/// The first event's correction to the logical size `size`: set now on
/// a normal window, and owed on one maximized or full screen, which a
/// resize would take out of the maximize.
pub(crate) fn correction(
    size: slint::LogicalSize,
    maximized: bool,
    full: bool,
) -> (Option<slint::LogicalSize>, Option<Owed>) {
    if maximized || full {
        (None, Some(Owed { size, seen: false }))
    } else {
        (Some(size), None)
    }
}

/// A correction owed at an event that finds the window `maximized` or
/// `full` as the platform reports it: still owed, and the size to set
/// now. It is due once the window is normal after the platform has
/// shown it maximized or full screen, not before: on X11 a maximize
/// asked before the window is mapped may be passed over and asked
/// again once it is shown, and a resize in that gap would fight it.
pub(crate) fn due(
    owed: Option<Owed>,
    maximized: bool,
    full: bool,
) -> (Option<Owed>, Option<slint::LogicalSize>) {
    match owed {
        None => (None, None),
        Some(o) if maximized || full => (Some(Owed { seen: true, ..o }), None),
        Some(o) if o.seen => (None, Some(o.size)),
        Some(o) => (Some(o), None),
    }
}

impl Follow {
    pub(crate) fn new(scale: Scale) -> Self {
        Self {
            scale,
            met: HashSet::new(),
            planned: None,
            owed: None,
            least: None,
        }
    }

    /// The scale this run, for the fit.
    pub(crate) fn scale(&self) -> Scale {
        self.scale
    }

    /// The logical size the window was fitted to before it was made.
    pub(crate) fn planned(&mut self, size: (f32, f32)) {
        self.planned = Some(slint::LogicalSize::new(size.0, size.1));
    }

    /// The layout's least, in logical pixels: set on the window again at
    /// each factor (`window_event`).
    pub(crate) fn least(&mut self, size: (f32, f32)) {
        self.least = Some(slint::LogicalSize::new(size.0, size.1));
    }

    /// A window's event, before Slint sees it: its first, which checks
    /// the guess the window opened at, and a scale change. A window
    /// corrected at its first event is set to the logical size it was
    /// fitted to, so it grows or shrinks on the screen with the factor
    /// (its size then may still be on its way from the compositor, so
    /// it is not the one to keep); at a scale change winit resizes it
    /// for the new monitor itself, and the event that follows gives
    /// Slint the size.
    pub(crate) fn window_event(
        &mut self,
        winit_window: Option<&winit::window::Window>,
        slint_window: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) {
        let (Some(winit_window), Some(slint_window)) = (winit_window, slint_window) else {
            return;
        };
        let changed = match event {
            winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Some(*scale_factor)
            }
            _ => None,
        };
        // Not at a resize: Slint takes its size after this, and would
        // put back the one the correction replaces. The next event
        // comes before the first frame on Wayland, which draws none
        // before its first configure.
        if changed.is_none() && matches!(event, winit::event::WindowEvent::Resized(_)) {
            return;
        }
        // The platform's own state, not Slint's, which holds the
        // maximize asked before the window existed.
        let (owed, set) = if self.owed.is_some() {
            due(
                self.owed,
                winit_window.is_maximized(),
                winit_window.fullscreen().is_some(),
            )
        } else {
            (None, None)
        };
        self.owed = owed;
        if let Some(size) = set {
            let factor = slint_window.scale_factor();
            slint_window.set_size(size.to_physical(factor));
            tracing::info!(
                "window scale: the correction owed, {:.0}x{:.0} now the window is normal",
                size.width,
                size.height
            );
        }
        let first = self.met.insert(winit_window.id());
        if !first && changed.is_none() {
            return;
        }
        let own = changed.unwrap_or_else(|| winit_window.scale_factor());
        SYSTEM.with(|s| s.set(Some(own)));
        let now = slint_window.scale_factor();
        let Some(want) = wanted(self.scale, own, now) else {
            return;
        };
        let logical = slint_window.size().to_logical(now);
        slint_window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
            scale_factor: want,
        });
        // Slint keeps the window's least as logical constraints and
        // hands them on only when they change, which a new factor does
        // not: the platform would hold the least at the old factor's
        // pixels, on X11 a minimum larger than the screen that no
        // resize goes under and no maximize fits. Set at this one.
        if let Some(least) = self.least {
            let least = least.to_physical(want);
            winit_window.set_min_inner_size(Some(winit::dpi::PhysicalSize::new(
                least.width,
                least.height,
            )));
        }
        if changed.is_none() {
            let (now, owed) = correction(
                self.planned.take().unwrap_or(logical),
                slint_window.is_maximized(),
                slint_window.is_fullscreen(),
            );
            // Physical: a logical size goes to winit as one, which winit
            // turns into pixels at its own scale, not at Slint's.
            if let Some(size) = now {
                slint_window.set_size(size.to_physical(want));
            }
            self.owed = owed;
        }
        slint_window.request_redraw();
        tracing::info!(
            "window scale {want:.3}: the system's {own:.2} times the setting, was {now:.3} at \
             {:.0}x{:.0}",
            logical.width,
            logical.height
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn the_setting_is_held_to_its_range_and_named() {
        assert_eq!(held(100), 100);
        assert_eq!(held(137), 137);
        assert_eq!(held(0), 50);
        assert_eq!(held(20), 50);
        assert_eq!(held(400), 300);
        assert_eq!(name(137), "137");
        assert_eq!(name(100), "100");
        assert_eq!(name(1000), "300");
        assert_eq!(named("137"), Some(137));
        assert_eq!(named(" 112% "), Some(112));
        assert_eq!(named("20"), Some(50));
        assert_eq!(named("-5"), Some(50));
        assert_eq!(named("400"), Some(300));
        assert_eq!(named("99999999999"), Some(300));
        assert_eq!(named("137.6"), Some(138));
        for bad in ["abc", "", "System", "NaN", "inf"] {
            assert_eq!(named(bad), None, "{bad:?}");
        }
        assert_eq!(factor_text(0.8 * 1.5), "1.2");
        assert_eq!(factor_text(2.0), "2");
        assert_eq!(factor_text(1.25 * 1.75), "2.1875");
    }

    /// The factor is the system's times the setting: what the window
    /// opens at is the last seen system scale times it, and the
    /// variable is ours, marked.
    #[test]
    fn the_start_multiplies_the_scale_last_seen() {
        assert_eq!(decide(100, 2.0, env(&[])), (Scale::System, Env::Leave));
        assert_eq!(
            decide(150, 1.0, env(&[])),
            (Scale::Times(1.5), Env::Set(1.5))
        );
        // A Mac's panel at 2: 125 % is 2.5, larger than the default.
        assert_eq!(
            decide(125, 2.0, env(&[])),
            (Scale::Times(1.25), Env::Set(2.5))
        );
        // None seen yet: 1.
        assert_eq!(
            decide(200, 0.0, env(&[])),
            (Scale::Times(2.0), Env::Set(2.0))
        );
        assert_eq!(
            decide(175, f64::NAN, env(&[])),
            (Scale::Times(1.75), Env::Set(1.75))
        );
        // Any percentage, under 100 too; a hand-edited one out of
        // range, held.
        assert_eq!(
            decide(137, 1.0, env(&[])),
            (Scale::Times(1.37), Env::Set(1.37))
        );
        assert_eq!(
            decide(80, 2.0, env(&[])),
            (Scale::Times(0.8), Env::Set(1.6))
        );
        assert_eq!(
            decide(1000, 1.0, env(&[])),
            (Scale::Times(3.0), Env::Set(3.0))
        );
    }

    /// `SLINT_SCALE_FACTOR` by hand wins and is left alone; ours,
    /// inherited, is worked out again, or taken off at 100 %.
    #[test]
    fn a_variable_set_by_hand_wins_and_ours_is_not_taken_for_one() {
        let by_hand = [(SLINT_SCALE_FACTOR, "1.75")];
        assert_eq!(
            decide(150, 1.0, env(&by_hand)),
            (Scale::ByHand(1.75), Env::Leave)
        );
        assert_eq!(
            decide(100, 1.0, env(&by_hand)),
            (Scale::ByHand(1.75), Env::Leave)
        );
        assert_eq!(
            decide(150, 1.0, env(&[(SLINT_SCALE_FACTOR, "big")])),
            (Scale::System, Env::Leave)
        );
        let ours = [(SLINT_SCALE_FACTOR, "3"), (OURS, "1")];
        assert_eq!(
            decide(125, 1.0, env(&ours)),
            (Scale::Times(1.25), Env::Set(1.25))
        );
        assert_eq!(decide(100, 1.0, env(&ours)), (Scale::System, Env::Remove));
    }

    /// On Wayland the start is at 1 times the setting, the scale seen
    /// passed over, since winit reports 1 there until the compositor's
    /// scale comes; Slint's backend asked for X11 is not Wayland.
    #[test]
    fn on_wayland_the_start_is_at_one_times_the_setting() {
        if !cfg!(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            ))
        )) || std::path::Path::new("/run/WSL").exists()
            || std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
        {
            return;
        }
        let wayland = [("WAYLAND_DISPLAY", "wayland-0")];
        assert_eq!(
            decide(150, 2.0, env(&wayland)),
            (Scale::Times(1.5), Env::Set(1.5))
        );
        assert_eq!(
            decide(150, 2.0, env(&[("WAYLAND_SOCKET", "5")])),
            (Scale::Times(1.5), Env::Set(1.5))
        );
        // Empty is unset, as winit reads it; X11 by DISPLAY.
        assert_eq!(
            decide(150, 2.0, env(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", ":0")])),
            (Scale::Times(1.5), Env::Set(3.0))
        );
        assert_eq!(
            decide(
                150,
                2.0,
                env(&[
                    ("WAYLAND_DISPLAY", "wayland-0"),
                    ("SLINT_BACKEND", "winit-x11")
                ])
            ),
            (Scale::Times(1.5), Env::Set(3.0))
        );
    }

    /// A window nothing fits: under the setting, the size asked for over
    /// the percentage, held to the layout's least; else as asked.
    #[test]
    fn a_window_nothing_fits_covers_what_it_does_at_100() {
        let (asked, least) = ((1500.0, 800.0), (560.0, 509.0));
        assert_eq!(unfitted(Scale::System, asked, least), asked);
        assert_eq!(unfitted(Scale::ByHand(2.0), asked, least), asked);
        assert_eq!(unfitted(Scale::Times(1.5), asked, least), (1000.0, 533.0));
        assert_eq!(unfitted(Scale::Times(2.0), asked, least), (750.0, 509.0));
    }

    /// The first event's correction: set on a normal window; owed on a
    /// maximized or full-screen one, and due once the platform has
    /// shown it so and then normal.
    #[test]
    fn a_maximized_window_is_corrected_once_normal() {
        let size = slint::LogicalSize::new(1400.0, 900.0);
        let owed = Owed { size, seen: false };
        assert_eq!(correction(size, false, false), (Some(size), None));
        assert_eq!(correction(size, true, false), (None, Some(owed)));
        assert_eq!(correction(size, false, true), (None, Some(owed)));
        assert_eq!(due(None, false, false), (None, None));
        // Maximized, then normal: due.
        let (held, set) = due(Some(owed), true, false);
        assert_eq!(set, None);
        assert_eq!(due(held, false, false), (None, Some(size)));
        // Full screen the same.
        let (held, _) = due(Some(owed), false, true);
        assert_eq!(due(held, false, false), (None, Some(size)));
    }

    /// X11 with the maximize passed over before the window was mapped:
    /// events that find it normal before the maximize asked again takes
    /// leave the correction owed, so it does not fight the maximize.
    #[test]
    fn the_correction_waits_for_a_maximize_asked_again() {
        let size = slint::LogicalSize::new(733.0, 503.0);
        let (_, owed) = correction(size, true, false);
        // The first events: the platform has not maximized it.
        let (owed, set) = due(owed, false, false);
        assert_eq!(set, None);
        let (owed, set) = due(owed, false, false);
        assert_eq!(set, None);
        // The maximize asked again takes, and stays.
        let (owed, set) = due(owed, true, false);
        assert_eq!(set, None);
        let (owed, set) = due(owed, true, false);
        assert_eq!(set, None);
        // Un-maximized by the user: now.
        assert_eq!(due(owed, false, false), (None, Some(size)));
    }

    /// Slint's factor on a monitor, and whether a window is changed:
    /// only under the setting, and only when it is off.
    #[test]
    fn the_window_wants_the_system_times_the_setting() {
        assert_eq!(Scale::System.on(1.5), 1.5);
        assert_eq!(Scale::Times(1.5).on(2.0), 3.0);
        assert_eq!(Scale::ByHand(1.25).on(2.0), 1.25);
        // The guess was right: nothing to do.
        assert_eq!(wanted(Scale::Times(1.5), 1.0, 1.5), None);
        // Opened at a guess of 1 on a panel at 2.
        assert_eq!(wanted(Scale::Times(1.5), 2.0, 1.5), Some(3.0));
        // Dragged to a monitor at 1.25.
        assert_eq!(wanted(Scale::Times(2.0), 1.25, 3.0), Some(2.5));
        // The backend's own at 100 %, and by hand.
        assert_eq!(wanted(Scale::System, 2.0, 1.0), None);
        assert_eq!(wanted(Scale::ByHand(1.25), 2.0, 1.0), None);
    }
}
