//! The window as it was closed, kept for the next run: its normal
//! size, whether it was maximized, and where it was on which monitor.
//!
//! One record a window, by name (only "main" today), so a second
//! window on a second monitor gets its own. Kept in the settings
//! file's window half, never in the half a headless run reads, and
//! written at the close with the panel's choices.
//!
//! The units are the desktop's logical pixels, the system's points:
//! physical pixels over the monitor's own scale, not over Slint's,
//! which the interface scale multiplies. So a window keeps the share
//! of the screen it had when the interface scale changes between
//! runs. The place is the window's corner from its monitor's corner in
//! that monitor's points, so it follows a monitor moved in the
//! desktop's arrangement and a change of its scale; the monitor is
//! known again by its name, its size, and its place.
//!
//! What is kept is the normal size, the one un-maximizing gives: a
//! maximized or full-screen window's own is the screen's. Each event
//! reports the window's state, and one in neither is taken only when
//! the next event also finds it in neither, since Windows reports the
//! move into a maximize before it says the window is maximized.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use slint::winit_030::winit;

use super::scale::Scale;
use super::{Monitor, Plan, WORK_MARGIN_HEIGHT, WORK_MARGIN_WIDTH, plan, work_area};

/// The editor's one window's name.
pub(crate) const MAIN: &str = "main";

/// Points of a title bar's height the check below asks to be on a
/// screen, and of its width.
const TITLE_BAR: f64 = 28.0;
const TITLE_SEEN: f64 = 120.0;

/// Each window's record by its name. A name this build does not know
/// is kept as read.
pub type Windows = BTreeMap<String, Kept>;

/// One window as it was closed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Kept {
    /// The normal size, the window's inside, in points.
    pub width: f32,
    pub height: f32,
    /// The corner from the monitor's corner in its points: the outer
    /// frame's on Windows and X11, the inside's on macOS, which is
    /// what each creates a window at. None where the platform does
    /// not say (Wayland).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<[f32; 2]>,
    pub maximized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor: Option<KeptMonitor>,
}

/// The monitor a window was on, as winit reports it: enough to know it
/// again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeptMonitor {
    /// winit's name for it, empty for none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Its corner and size in the desktop's physical pixels.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A monitor present now, as winit reports it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Screen {
    pub(crate) name: String,
    pub(crate) position: (i32, i32),
    pub(crate) size: (u32, u32),
    pub(crate) scale: f64,
}

impl Screen {
    pub(crate) fn of(m: &winit::monitor::MonitorHandle) -> Self {
        let (p, s) = (m.position(), m.size());
        Screen {
            name: m.name().unwrap_or_default(),
            position: (p.x, p.y),
            size: (s.width, s.height),
            scale: m.scale_factor(),
        }
    }

    fn kept(&self) -> KeptMonitor {
        KeptMonitor {
            name: self.name.clone(),
            x: self.position.0,
            y: self.position.1,
            width: self.size.0,
            height: self.size.1,
        }
    }

    fn rect(&self) -> work_area::Rect {
        (self.position.0, self.position.1, self.size.0, self.size.1)
    }
}

/// Which of `screens` is the monitor kept: the same size, and the same
/// name where both have one; of those, the one in the same place, else
/// the only one when it is known by name (a monitor moved in the
/// arrangement). None when it is gone, or two could be it.
pub(crate) fn find(kept: &KeptMonitor, screens: &[Screen]) -> Option<usize> {
    let named = |s: &Screen| kept.name.is_empty() || s.name.is_empty() || s.name == kept.name;
    let candidates: Vec<usize> = screens
        .iter()
        .enumerate()
        .filter(|(_, s)| s.size == (kept.width, kept.height) && named(s))
        .map(|(i, _)| i)
        .collect();
    if let Some(&i) = candidates
        .iter()
        .find(|&&i| screens[i].position == (kept.x, kept.y))
    {
        return Some(i);
    }
    match candidates[..] {
        [i] if !kept.name.is_empty() && screens[i].name == kept.name => Some(i),
        _ => None,
    }
}

/// The window's state at an event.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct State {
    pub(crate) maximized: bool,
    pub(crate) full: bool,
    pub(crate) minimized: bool,
}

/// The window's geometry at an event, as the record keeps it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Normal {
    pub(crate) size: (f32, f32),
    pub(crate) at: Option<[f32; 2]>,
    pub(crate) monitor: Option<KeptMonitor>,
}

/// The geometry from what winit reports: the inside's physical size at
/// the window's own scale, its corner in the desktop's pixels where the
/// platform says, and the monitor it is on.
pub(crate) fn normal_of(
    inner: (u32, u32),
    scale: f64,
    corner: Option<(i32, i32)>,
    screen: Option<&Screen>,
) -> Normal {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let at = corner.zip(screen).map(|(c, s)| {
        [
            (f64::from(c.0 - s.position.0) / s.scale) as f32,
            (f64::from(c.1 - s.position.1) / s.scale) as f32,
        ]
    });
    Normal {
        size: (
            (f64::from(inner.0) / scale) as f32,
            (f64::from(inner.1) / scale) as f32,
        ),
        at,
        monitor: screen.map(Screen::kept),
    }
}

/// What the window's events have shown of it this run.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    /// The last normal geometry confirmed by a later event.
    normal: Option<Normal>,
    /// The last event's, while it found the window normal: confirmed by
    /// the next that does too, or at the close.
    pending: Option<Normal>,
    /// Whether the window was maximized, as last seen out of full
    /// screen.
    maximized: Option<bool>,
    /// The window is closing: what comes now is its withdrawal (on
    /// X11 an unmap and a reparent to the root at the frame's corner,
    /// which winit reports as a move), not the user's.
    frozen: bool,
    /// The event loop has ended: frozen for good.
    ended: bool,
}

/// Whether two readings are the same rectangle's, to a point.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1.0
}

impl Tracker {
    /// An event, and what it found. Minimized, or with no size, it says
    /// nothing; full screen, nothing but that, and maximized that it is
    /// maximized. Then the last normal reading is taken unless it is
    /// the maximized or full-screen rectangle's own, by its size or its
    /// corner: Windows reports the move into a maximize before it says
    /// maximized.
    pub(crate) fn observe(&mut self, state: State, now: Normal) {
        if self.frozen || state.minimized || now.size.0 <= 0.0 || now.size.1 <= 0.0 {
            return;
        }
        if state.full || state.maximized {
            if let Some(pending) = self.pending.take() {
                let same_size =
                    near(pending.size.0, now.size.0) && near(pending.size.1, now.size.1);
                let same_corner = pending
                    .at
                    .zip(now.at)
                    .is_some_and(|(p, n)| near(p[0], n[0]) && near(p[1], n[1]));
                if !same_size && !same_corner {
                    self.normal = Some(pending);
                }
            }
            if state.maximized {
                self.maximized = Some(true);
            }
            return;
        }
        if let Some(confirmed) = self.pending.replace(now) {
            self.normal = Some(confirmed);
        }
        self.maximized = Some(false);
    }

    /// The record at the close, over the one kept before: the normal
    /// geometry seen, else the kept one's, and the state. None when the
    /// run showed nothing, which leaves the kept one as it was.
    pub(crate) fn kept(&self, before: Option<&Kept>) -> Option<Kept> {
        let normal = self.pending.as_ref().or(self.normal.as_ref());
        if normal.is_none() && self.maximized.is_none() {
            return None;
        }
        let mut kept = before.cloned().unwrap_or_default();
        if let Some(n) = normal {
            kept.width = n.size.0;
            kept.height = n.size.1;
            kept.at = n.at;
            kept.monitor = n.monitor.clone();
        }
        if let Some(m) = self.maximized {
            kept.maximized = m;
        }
        Some(kept)
    }

    /// No reading is taken from now (`frozen`), or again.
    pub(crate) fn freeze(&mut self, frozen: bool) {
        self.frozen = frozen;
    }
}

thread_local! {
    /// This run's tracker; none for a run that keeps no window.
    static TRACKER: RefCell<Option<Tracker>> = const { RefCell::new(None) };
}

/// This run keeps the window: its events are followed from now.
pub(crate) fn start() {
    TRACKER.with(|t| *t.borrow_mut() = Some(Tracker::default()));
}

/// The window was asked to close: no reading from now, unless the close
/// is refused (an import running asks first) and the window, still
/// shown, is sized or moved again (`observe`).
pub(crate) fn close_requested() {
    TRACKER.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.freeze(true);
        }
    });
}

/// The event loop is ending: no reading from now, whatever comes.
pub(crate) fn exiting() {
    TRACKER.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.freeze(true);
            t.ended = true;
        }
    });
}

/// An event on the window: its state and geometry, as winit has them.
/// None is read once the window is hidden (`shown`, Slint's, false, or
/// winit's own), as it is before a close withdraws it.
pub(crate) fn observe(w: &winit::window::Window, shown: bool) {
    if TRACKER.with(|t| t.borrow().is_none()) || !shown || w.is_visible() == Some(false) {
        return;
    }
    // Shown and sized or moved after a close was asked: it was refused.
    TRACKER.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut()
            && !t.ended
        {
            t.freeze(false);
        }
    });
    let state = State {
        maximized: w.is_maximized(),
        full: w.fullscreen().is_some(),
        minimized: w.is_minimized() == Some(true),
    };
    let inner = w.inner_size();
    // The corner a window is created at: the inside's on macOS, where
    // winit makes the window from a content rectangle, the frame's
    // elsewhere.
    let corner = if cfg!(target_os = "macos") {
        w.inner_position()
    } else {
        w.outer_position()
    };
    let screen = w.current_monitor().map(|m| Screen::of(&m));
    let now = normal_of(
        (inner.width, inner.height),
        w.scale_factor(),
        corner.ok().map(|p| (p.x, p.y)),
        screen.as_ref(),
    );
    TRACKER.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.observe(state, now);
        }
    });
}

/// An event as a test has it, with no window.
#[cfg(test)]
pub(crate) fn observe_as(state: State, now: Normal) {
    TRACKER.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.observe(state, now);
        }
    });
}

/// The run keeps no window again, as a test leaves it.
#[cfg(test)]
pub(crate) fn stop() {
    TRACKER.with(|t| *t.borrow_mut() = None);
}

/// Put the window's record in `windows` for the close's write; nothing
/// for a run that keeps none.
pub(crate) fn store(windows: &mut Windows) {
    let record = TRACKER.with(|t| t.borrow().as_ref().map(|t| t.kept(windows.get(MAIN))));
    if let Some(Some(record)) = record {
        tracing::debug!(
            "window: kept {:.0}x{:.0}{}",
            record.width,
            record.height,
            if record.maximized { ", maximized" } else { "" }
        );
        windows.insert(MAIN.into(), record);
    }
}

/// What a run does with the kept window: whether it keeps it, and the
/// record to open as.
#[derive(Debug, Clone, Default)]
pub(crate) struct Memory {
    pub(crate) on: bool,
    pub(crate) kept: Option<Kept>,
}

impl Memory {
    /// For a run that keeps the window (`on`), the record from the
    /// file's `windows`.
    pub(crate) fn of(on: bool, windows: &Windows) -> Self {
        Memory {
            on,
            kept: on.then(|| windows.get(MAIN).cloned()).flatten(),
        }
    }
}

/// Where the window goes: in the desktop's physical pixels, and in the
/// monitor's points (macOS, where a window is created at points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Place {
    pub(crate) physical: (i32, i32),
    pub(crate) points: (f64, f64),
}

/// The window to open as, from a record.
#[derive(Debug, PartialEq)]
pub(crate) struct Restored {
    /// The fit, of the kept size.
    pub(crate) plan: Plan,
    /// The kept size in Slint's pixels, before the fit.
    pub(crate) size: (f32, f32),
    /// Where we put it; none to leave it to the platform, or to the
    /// fit's own place on Windows.
    pub(crate) at: Option<Place>,
    pub(crate) maximized: bool,
    /// The log's line.
    pub(crate) line: String,
}

/// The window to open as, from `kept`, the monitors present now
/// (`screens` and `monitors` the same ones, in the same order), the
/// run's scale, and the layout's least. The size goes through the fit
/// (§306, §500): against the kept monitor's room when it is there and
/// the place is used, else as a new window is fitted. The place is used
/// only on the kept monitor, and only when the title bar would be on a
/// screen; it is then moved in so the window is inside the room where
/// it fits. None for a record with no size.
pub(crate) fn restore(
    kept: &Kept,
    wayland: bool,
    screens: &[Screen],
    monitors: &[Monitor],
    scale: Scale,
    least: (f32, f32),
) -> Option<Restored> {
    let valid = |v: f32| v.is_finite() && v > 0.0;
    if !valid(kept.width) || !valid(kept.height) {
        return None;
    }
    // Slint's pixels in a point on a monitor of scale `own`.
    let size_on = |own: f64| {
        let per = (own / scale.on(own)) as f32;
        (kept.width * per, kept.height * per)
    };
    let maximized = if kept.maximized { ", maximized" } else { "" };
    let said = format!("window: kept {:.0}x{:.0}", kept.width, kept.height);
    if wayland {
        // No monitor's scale is known before the window is, so 1.
        let size = size_on(1.0);
        let size = (size.0.max(least.0).round(), size.1.max(least.1).round());
        return Some(Restored {
            plan: Plan::Wayland,
            size,
            at: None,
            maximized: kept.maximized,
            line: format!("{said}{maximized}; the compositor places it"),
        });
    }
    let found = kept.monitor.as_ref().and_then(|m| find(m, screens));
    let why = match (found, kept.at) {
        (None, _) if kept.monitor.is_some() => "its monitor is gone or changed",
        (None, _) | (_, None) => "no place kept",
        (Some(i), Some(at)) => {
            let own = screens[i].scale;
            let size = size_on(own);
            let fitted = plan(false, size, &monitors[i..=i], scale, least);
            match &fitted {
                Plan::Fit { opened, .. } => {
                    match placed(at, &screens[i], monitors[i].work, *opened, scale, screens) {
                        Some(place) => {
                            let name = &screens[i].name;
                            return Some(Restored {
                                plan: fitted,
                                size,
                                at: Some(place),
                                maximized: kept.maximized,
                                line: format!(
                                    "{said} at {},{} on {}{maximized}",
                                    place.physical.0,
                                    place.physical.1,
                                    if name.is_empty() { "its monitor" } else { name }
                                ),
                            });
                        }
                        None => "its place is off the screens",
                    }
                }
                _ => "its monitor has no size",
            }
        }
    };
    // As a new window: fitted to the smallest room, at the scale of the
    // monitor the system opens it on.
    let own = monitors
        .iter()
        .find(|m| m.opens_here)
        .or(monitors.first())
        .map_or(1.0, |m| m.scale);
    let size = size_on(own);
    Some(Restored {
        plan: plan(false, size, monitors, scale, least),
        size,
        at: None,
        maximized: kept.maximized,
        line: format!("{said}{maximized}; {why}, placed as a new window"),
    })
}

/// The kept corner `at` on `screen` in the desktop's pixels, when the
/// title bar of a window `opened` (Slint's pixels under `scale`) is on
/// one of `screens` there: at least a title bar's half height and
/// `TITLE_SEEN` points across, or the whole width of a narrower one.
/// Then moved in so the window and its frame are inside the room (the
/// work area, else the monitor) where they fit, from its top left where
/// they do not.
fn placed(
    at: [f32; 2],
    screen: &Screen,
    work: Option<work_area::Rect>,
    opened: (f32, f32),
    scale: Scale,
    screens: &[Screen],
) -> Option<Place> {
    let own = screen.scale;
    if !(at[0].is_finite() && at[1].is_finite()) || own <= 0.0 {
        return None;
    }
    let factor = scale.on(own);
    let outer = (
        f64::from(opened.0) * factor + f64::from(WORK_MARGIN_WIDTH) * own,
        f64::from(opened.1) * factor + f64::from(WORK_MARGIN_HEIGHT) * own,
    );
    let x = f64::from(screen.position.0) + f64::from(at[0]) * own;
    let y = f64::from(screen.position.1) + f64::from(at[1]) * own;
    let seen = |r: work_area::Rect| {
        let across = (x + outer.0).min(f64::from(r.0) + f64::from(r.2)) - x.max(f64::from(r.0));
        let down =
            (y + TITLE_BAR * own).min(f64::from(r.1) + f64::from(r.3)) - y.max(f64::from(r.1));
        across >= (TITLE_SEEN * own).min(outer.0) && down >= TITLE_BAR * own / 2.0
    };
    if !screens.iter().any(|s| seen(s.rect())) {
        return None;
    }
    let room = work.unwrap_or(screen.rect());
    let inside = |v: f64, start: i32, length: u32, size: f64| {
        let start = f64::from(start);
        v.min(start + f64::from(length) - size).max(start)
    };
    let x = inside(x, room.0, room.2, outer.0).round();
    let y = inside(y, room.1, room.3, outer.1).round();
    Some(Place {
        physical: (x as i32, y as i32),
        points: (x / own, y / own),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAST: (f32, f32) = (560.0, 509.0);

    fn screen(name: &str, position: (i32, i32), size: (u32, u32), scale: f64) -> Screen {
        Screen {
            name: name.into(),
            position,
            size,
            scale,
        }
    }

    fn monitor(s: &Screen, work: Option<work_area::Rect>) -> Monitor {
        Monitor {
            size: s.size,
            scale: s.scale,
            work,
            opens_here: false,
        }
    }

    fn normal(size: (f32, f32), at: [f32; 2], s: &Screen) -> Normal {
        Normal {
            size,
            at: Some(at),
            monitor: Some(s.kept()),
        }
    }

    const NORMAL: State = State {
        maximized: false,
        full: false,
        minimized: false,
    };
    const MAXIMIZED: State = State {
        maximized: true,
        full: false,
        minimized: false,
    };
    const FULL: State = State {
        maximized: false,
        full: true,
        minimized: false,
    };

    /// What winit reports, as the record keeps it: points, and the
    /// corner from the monitor's in its points.
    #[test]
    fn the_geometry_is_kept_in_points_from_the_monitors_corner() {
        let right = screen("DP-2", (2560, 0), (3840, 2160), 1.5);
        let n = normal_of((2100, 1350), 1.5, Some((2710, 150)), Some(&right));
        assert_eq!(n.size, (1400.0, 900.0));
        assert_eq!(n.at, Some([100.0, 100.0]));
        assert_eq!(n.monitor.as_ref().unwrap().name, "DP-2");
        // Wayland: no corner.
        let n = normal_of((2100, 1350), 1.5, None, Some(&right));
        assert_eq!(n.at, None);
    }

    /// Normal at the close: the last size and place.
    #[test]
    fn saved_while_normal() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        t.observe(NORMAL, normal((1300.0, 820.0), [60.0, 40.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!((kept.width, kept.height), (1300.0, 820.0));
        assert_eq!(kept.at, Some([60.0, 40.0]));
        assert!(!kept.maximized);
        // A run that showed nothing leaves the kept one as it was.
        assert_eq!(Tracker::default().kept(Some(&kept)), None);
    }

    /// Maximized, then closed: the normal size and place it had, and
    /// the flag, though Windows reports the move into the maximize as a
    /// normal window's before it says maximized.
    #[test]
    fn maximize_then_close_keeps_the_normal_size() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        // Windows: Moved to the maximized corner, not yet maximized.
        t.observe(NORMAL, normal((1200.0, 800.0), [-8.0, -8.0], &s));
        t.observe(MAXIMIZED, normal((1920.0, 1032.0), [-8.0, -8.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!((kept.width, kept.height), (1200.0, 800.0));
        assert_eq!(kept.at, Some([50.0, 40.0]));
        assert!(kept.maximized);
        // Opened maximized and never un-maximized: the size kept before.
        let mut t = Tracker::default();
        t.observe(MAXIMIZED, normal((1920.0, 1032.0), [-8.0, -8.0], &s));
        let again = t.kept(Some(&kept)).unwrap();
        assert_eq!(again, kept);
        // Un-maximized later: normal again.
        t.observe(NORMAL, normal((1000.0, 700.0), [10.0, 10.0], &s));
        let kept = t.kept(Some(&kept)).unwrap();
        assert_eq!(
            ((kept.width, kept.height), kept.maximized),
            ((1000.0, 700.0), false)
        );
    }

    /// One resize, then a maximize: the resize is the normal size, not
    /// the reading before it; a last reading of the maximized size, at
    /// any corner, is dropped.
    #[test]
    fn a_single_step_before_a_maximize_is_kept() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        t.observe(NORMAL, normal((1300.0, 820.0), [50.0, 40.0], &s));
        t.observe(MAXIMIZED, normal((1920.0, 1032.0), [0.0, 0.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!(
            ((kept.width, kept.height), kept.maximized),
            ((1300.0, 820.0), true)
        );
        // The only reading, then full screen: kept.
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1300.0, 820.0), [50.0, 40.0], &s));
        t.observe(FULL, normal((1920.0, 1080.0), [0.0, 0.0], &s));
        assert_eq!(t.kept(None).unwrap().width, 1300.0);
        // A last reading at the maximized size, not yet maximized.
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        t.observe(NORMAL, normal((1920.0, 1032.0), [30.0, 30.0], &s));
        t.observe(MAXIMIZED, normal((1920.0, 1032.0), [0.0, 0.0], &s));
        assert_eq!(t.kept(None).unwrap().width, 1200.0);
    }

    /// Once the window is closing, nothing is read: X11's withdrawal
    /// reports a move to the frame's corner, which winit would take
    /// less the frame once more.
    #[test]
    fn nothing_is_read_after_the_freeze() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 700.0), [300.0, 150.0], &s));
        t.freeze(true);
        t.observe(NORMAL, normal((1200.0, 700.0), [300.0, 113.0], &s));
        t.observe(NORMAL, normal((1920.0, 1043.0), [0.0, -37.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!((kept.at, kept.width), (Some([300.0, 150.0]), 1200.0));
        // A close refused: read again.
        t.freeze(false);
        t.observe(NORMAL, normal((1200.0, 700.0), [310.0, 150.0], &s));
        assert_eq!(t.kept(None).unwrap().at, Some([310.0, 150.0]));
    }

    /// Full screen, then closed: as it was before full screen, maximized
    /// or not, at its normal size.
    #[test]
    fn full_screen_then_close() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        t.observe(FULL, normal((1920.0, 1080.0), [0.0, 0.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!(
            ((kept.width, kept.height), kept.maximized),
            ((1200.0, 800.0), false)
        );
        let mut t = Tracker::default();
        t.observe(NORMAL, normal((1200.0, 800.0), [50.0, 40.0], &s));
        t.observe(MAXIMIZED, normal((1920.0, 1032.0), [0.0, 0.0], &s));
        // Windows reports a full-screen window not maximized.
        t.observe(FULL, normal((1920.0, 1080.0), [0.0, 0.0], &s));
        let kept = t.kept(None).unwrap();
        assert_eq!(
            ((kept.width, kept.height), kept.maximized),
            ((1200.0, 800.0), true)
        );
        // Minimized says nothing.
        t.observe(
            State {
                minimized: true,
                ..NORMAL
            },
            normal((0.0, 0.0), [0.0, 0.0], &s),
        );
        assert_eq!(t.kept(None).unwrap(), kept);
    }

    fn kept_on(s: &Screen, size: (f32, f32), at: [f32; 2], maximized: bool) -> Kept {
        Kept {
            width: size.0,
            height: size.1,
            at: Some(at),
            maximized,
            monitor: Some(s.kept()),
        }
    }

    /// On its monitor, still there: the size and the place, in the
    /// desktop's pixels and in points.
    #[test]
    fn restored_on_its_monitor() {
        let left = screen("DISPLAY1", (0, 0), (1920, 1080), 1.0);
        let right = screen("DISPLAY2", (1920, 0), (3840, 2160), 1.5);
        let kept = kept_on(&right, (1400.0, 900.0), [100.0, 80.0], true);
        let screens = [left.clone(), right.clone()];
        let monitors = [monitor(&left, None), monitor(&right, None)];
        let r = restore(&kept, false, &screens, &monitors, Scale::System, LEAST).unwrap();
        assert_eq!(r.size, (1400.0, 900.0));
        assert_eq!(
            r.at,
            Some(Place {
                physical: (2070, 120),
                points: (1380.0, 80.0)
            })
        );
        assert!(r.maximized);
        assert_eq!(
            r.line,
            "window: kept 1400x900 at 2070,120 on DISPLAY2, maximized"
        );
        let Plan::Fit {
            opened,
            monitor: logical,
            ..
        } = r.plan
        else {
            panic!("{:?}", r.plan)
        };
        assert_eq!((opened, logical), ((1400.0, 900.0), (2560.0, 1440.0)));
        // The monitor moved in the arrangement, known by its name: the
        // window goes with it.
        let moved = screen("DISPLAY2", (-3840, 0), (3840, 2160), 1.5);
        let r = restore(
            &kept,
            false,
            &[left.clone(), moved.clone()],
            &[monitor(&left, None), monitor(&moved, None)],
            Scale::System,
            LEAST,
        )
        .unwrap();
        assert_eq!(r.at.unwrap().physical, (-3690, 120));
    }

    /// The monitor gone: the size, fitted as a new window's, and the
    /// place left to the platform.
    #[test]
    fn restore_with_the_monitor_gone() {
        let gone = screen("DISPLAY2", (1920, 0), (3840, 2160), 1.5);
        let laptop = screen("DISPLAY1", (0, 0), (1920, 1080), 1.0);
        let kept = kept_on(&gone, (1400.0, 900.0), [100.0, 80.0], false);
        let r = restore(
            &kept,
            false,
            std::slice::from_ref(&laptop),
            &[monitor(&laptop, None)],
            Scale::System,
            LEAST,
        )
        .unwrap();
        assert_eq!(r.at, None);
        assert_eq!(
            r.line,
            "window: kept 1400x900; its monitor is gone or changed, placed as a new window"
        );
        // 1920 x 1080 less the guess's 80 x 140.
        let Plan::Fit { opened, .. } = r.plan else {
            panic!()
        };
        assert_eq!(opened, (1400.0, 900.0));
        // Two of the same size and no names: neither is taken for it.
        let a = screen("", (0, 0), (1920, 1080), 1.0);
        let b = screen("", (1920, 0), (1920, 1080), 1.0);
        let k = KeptMonitor {
            x: 3840,
            ..a.kept()
        };
        assert_eq!(find(&k, &[a.clone(), b.clone()]), None);
        assert_eq!(find(&b.kept(), &[a, b]), Some(1));
    }

    /// Kept on a big monitor, opened on a laptop's: no larger than the
    /// laptop's room, whether the monitor is that one or another.
    #[test]
    fn restore_onto_a_smaller_screen_is_fitted() {
        let air = screen("Monitor #1", (0, 0), (2880, 1800), 2.0);
        let work = Some((0, 74, 2880, 1640));
        let kept = kept_on(&air, (2400.0, 1300.0), [20.0, 40.0], false);
        let r = restore(
            &kept,
            false,
            std::slice::from_ref(&air),
            &[monitor(&air, work)],
            Scale::System,
            LEAST,
        )
        .unwrap();
        let Plan::Fit { opened, .. } = r.plan else {
            panic!()
        };
        // The work area 1440 x 820 points less 16 x 48.
        assert_eq!(opened, (1424.0, 772.0));
        // Moved in so it fits: from the work area's corner.
        assert_eq!(r.at.unwrap().physical, (0, 74));
    }

    /// A place with no title bar on any screen is dropped; one partly
    /// off is moved in.
    #[test]
    fn a_place_off_screen_is_dropped() {
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        let monitors = [monitor(&s, None)];
        let r = |at| {
            restore(
                &kept_on(&s, (1200.0, 800.0), at, false),
                false,
                std::slice::from_ref(&s),
                &monitors,
                Scale::System,
                LEAST,
            )
            .unwrap()
        };
        for off in [
            [5000.0, 100.0],
            [100.0, 2000.0],
            [-1300.0, 100.0],
            [100.0, -40.0],
        ] {
            let r = r(off);
            assert_eq!(r.at, None, "{off:?}");
            assert!(
                r.line
                    .ends_with("its place is off the screens, placed as a new window")
            );
        }
        // Its title bar on the screen, its foot under it: moved up.
        assert_eq!(r([300.0, 600.0]).at.unwrap().physical, (300, 232));
        // Not a number: dropped.
        assert_eq!(r([f32::NAN, 0.0]).at, None);
    }

    /// The interface scale changed between runs: the window covers what
    /// it did, in Slint's pixels the points over the percentage.
    #[test]
    fn the_scale_changed_between_runs() {
        let s = screen("A", (0, 0), (3840, 2160), 1.0);
        let kept = kept_on(&s, (1500.0, 900.0), [100.0, 100.0], false);
        let monitors = [monitor(&s, None)];
        let at = |scale| {
            let r = restore(
                &kept,
                false,
                std::slice::from_ref(&s),
                &monitors,
                scale,
                LEAST,
            )
            .unwrap();
            let Plan::Fit { opened, .. } = r.plan else {
                panic!()
            };
            (r.size, opened, r.at.unwrap().physical)
        };
        assert_eq!(
            at(Scale::System),
            ((1500.0, 900.0), (1500.0, 900.0), (100, 100))
        );
        assert_eq!(
            at(Scale::Times(1.5)),
            ((1000.0, 600.0), (1000.0, 600.0), (100, 100))
        );
        assert_eq!(
            at(Scale::Times(0.5)),
            ((3000.0, 1800.0), (3000.0, 1800.0), (100, 100))
        );
        // SLINT_SCALE_FACTOR=2 by hand on a monitor at 1.
        assert_eq!(at(Scale::ByHand(2.0)).0, (750.0, 450.0));
        // Wayland: at 1 times the setting, as the size asked for is.
        let r = restore(&kept, true, &[], &[], Scale::Times(1.5), LEAST).unwrap();
        assert_eq!(
            (r.plan, r.size, r.at),
            (Plan::Wayland, (1000.0, 600.0), None)
        );
        // And the record's points come back the same from what winit
        // reports of a window opened so: the scale is not in them.
        let n = normal_of((1500, 900), 1.0, Some((100, 100)), Some(&s));
        assert_eq!((n.size, n.at), ((1500.0, 900.0), Some([100.0, 100.0])));
    }

    /// A record with no size is no record.
    #[test]
    fn a_record_with_no_size_is_none() {
        let k = Kept::default();
        assert_eq!(restore(&k, false, &[], &[], Scale::System, LEAST), None);
    }

    /// A settings file from before: no windows, and a record missing
    /// fields reads with defaults; an entry that is no record is left
    /// out, and a name this build does not know survives a write.
    #[test]
    fn older_files_and_unknown_windows() {
        let read: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert!(read.windows.is_empty());
        let text = serde_json::to_string(&read).unwrap();
        assert!(!text.contains("windows"), "{text}");
        let read: crate::settings::Settings = serde_json::from_str(
            r#"{"windows": {"main": {"width": 1200, "height": 800},
                "second": {"width": 900, "height": 600, "at": [10, 20],
                           "monitor": {"name": "HDMI-1", "x": 1920, "y": 0,
                                       "width": 1920, "height": 1080},
                           "later": true},
                "broken": 5}}"#,
        )
        .unwrap();
        assert_eq!(read.windows.keys().collect::<Vec<_>>(), ["main", "second"]);
        assert_eq!(read.windows["main"].at, None);
        let mut windows = read.windows.clone();
        start();
        let s = screen("A", (0, 0), (1920, 1080), 1.0);
        observe_as(MAXIMIZED, normal((1920.0, 1032.0), [0.0, 0.0], &s));
        store(&mut windows);
        stop();
        assert!(windows["main"].maximized);
        assert_eq!(windows["main"].width, 1200.0);
        assert_eq!(windows["second"], read.windows["second"]);
        // Nothing stored by a run that keeps none.
        let mut none = Windows::new();
        store(&mut none);
        assert!(none.is_empty());
    }
}
