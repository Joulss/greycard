//! Each monitor's work area, the part a menu bar, a Dock, a task bar, or
//! a panel leave, as the platform reports it: a rectangle in the
//! desktop's physical pixels, the units winit gives a monitor's own
//! position and size in. None where the platform does not say, and the
//! fit falls back to its guessed margin there.
//!
//! - macOS: `NSScreen`'s `visibleFrame`, for the screen whose display id
//!   is the monitor's, in proportion to its `frame` and from the
//!   monitor's position.
//! - Windows: `GetMonitorInfoW`'s `rcWork` for the monitor's handle.
//! - X11: the window manager's `_NET_WORKAREA` for the current desktop,
//!   clipped to the monitor. It is one rectangle over the whole root
//!   window, so a panel on one monitor of several can narrow the others.
//! - Wayland: nothing. A client is told no work area (a layer shell's
//!   exclusive zones are the compositor's), and the compositor places
//!   and sizes the window itself.

use slint::winit_030::winit::monitor::MonitorHandle;

/// A rectangle as x, y, width, and height in the desktop's physical
/// pixels, y running down.
pub(crate) type Rect = (i32, i32, u32, u32);

/// The work area of each of `monitors`, in order.
pub(crate) fn read(wayland: bool, monitors: &[MonitorHandle]) -> Vec<Option<Rect>> {
    if wayland {
        return vec![None; monitors.len()];
    }
    platform(monitors)
}

#[cfg(target_os = "macos")]
fn platform(monitors: &[MonitorHandle]) -> Vec<Option<Rect>> {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::{NSNumber, ns_string};
    use slint::winit_030::winit::platform::macos::MonitorHandleExtMacOS;

    // AppKit is asked on the main thread only, where winit's handler
    // runs.
    let Some(mtm) = MainThreadMarker::new() else {
        return vec![None; monitors.len()];
    };
    let screens: Vec<_> = NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let description = screen.deviceDescription();
            let number: Retained<AnyObject> =
                description.objectForKey(ns_string!("NSScreenNumber"))?;
            let id = number.downcast_ref::<NSNumber>()?.unsignedIntValue();
            let (f, v) = (screen.frame(), screen.visibleFrame());
            Some((
                id,
                [f.origin.x, f.origin.y, f.size.width, f.size.height],
                [v.origin.x, v.origin.y, v.size.width, v.size.height],
            ))
        })
        .collect();
    monitors
        .iter()
        .map(|m| {
            let &(_, frame, visible) = screens.iter().find(|s| s.0 == m.native_id())?;
            let (at, size) = (m.position(), m.size());
            let (x, y, w, h) = visible_in_pixels((size.width, size.height), frame, visible)?;
            Some((at.x + x, at.y + y, w, h))
        })
        .collect()
}

/// A visible frame in a monitor's pixels, from the monitor's top-left
/// corner: both frames are AppKit's x, y, width, and height in points,
/// y running up, and the visible one's share of the frame is its share
/// of the pixels, whatever the display's scaling makes a point.
#[cfg(any(target_os = "macos", test))]
fn visible_in_pixels(pixels: (u32, u32), frame: [f64; 4], visible: [f64; 4]) -> Option<Rect> {
    let [fx, fy, fw, fh] = frame;
    if fw <= 0.0 || fh <= 0.0 {
        return None;
    }
    let left = (visible[0] - fx).clamp(0.0, fw);
    let right = (visible[0] + visible[2] - fx).clamp(left, fw);
    let foot = (visible[1] - fy).clamp(0.0, fh);
    let top = (visible[1] + visible[3] - fy).clamp(foot, fh);
    let across = |p: f64| (p * f64::from(pixels.0) / fw).round();
    let down = |p: f64| (p * f64::from(pixels.1) / fh).round();
    let (w, h) = (across(right) - across(left), down(top) - down(foot));
    (w > 0.0 && h > 0.0).then(|| {
        (
            across(left) as i32,
            (down(fh) - down(top)) as i32,
            w as u32,
            h as u32,
        )
    })
}

#[cfg(windows)]
fn platform(monitors: &[MonitorHandle]) -> Vec<Option<Rect>> {
    use slint::winit_030::winit::platform::windows::MonitorHandleExtWindows;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};

    monitors
        .iter()
        .map(|m| {
            // SAFETY: a zeroed MONITORINFO with its size set is a valid
            // one to fill; the handle is winit's for a live monitor, and
            // a failure (one unplugged since) returns zero, handled.
            let info = unsafe {
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                (GetMonitorInfoW(m.hmonitor() as HMONITOR, &mut info) != 0).then_some(info)
            }?;
            let r = info.rcWork;
            Some((
                r.left,
                r.top,
                u32::try_from(r.right - r.left).ok()?,
                u32::try_from(r.bottom - r.top).ok()?,
            ))
        })
        .collect()
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn platform(monitors: &[MonitorHandle]) -> Vec<Option<Rect>> {
    let work = match net_workarea() {
        Ok(work) => work,
        Err(e) => {
            tracing::debug!("window: no _NET_WORKAREA ({e})");
            None
        }
    };
    monitors
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            clip((p.x, p.y, s.width, s.height), work?)
        })
        .collect()
}

/// The root window's `_NET_WORKAREA` for the current desktop, as x, y,
/// width, and height in pixels; None when the window manager sets none.
/// A connection of our own, opened and closed here: winit's is Xlib's,
/// behind a pointer.
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn net_workarea() -> anyhow::Result<Option<Rect>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let atom =
        |name: &[u8]| -> anyhow::Result<u32> { Ok(conn.intern_atom(true, name)?.reply()?.atom) };
    let (workarea, current) = (atom(b"_NET_WORKAREA")?, atom(b"_NET_CURRENT_DESKTOP")?);
    if workarea == 0 {
        return Ok(None);
    }
    let desktop = if current == 0 {
        0
    } else {
        conn.get_property(false, root, current, AtomEnum::CARDINAL, 0, 1)?
            .reply()?
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or(0)
    };
    let values: Vec<u32> = conn
        .get_property(false, root, workarea, AtomEnum::CARDINAL, 0, u32::MAX / 4)?
        .reply()?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    // Four values a desktop; a desktop past the list reads the first.
    let at = |d: usize| values.get(d * 4..d * 4 + 4);
    Ok(at(desktop as usize)
        .or_else(|| at(0))
        .map(|r| (r[0] as i32, r[1] as i32, r[2], r[3])))
}

/// The part of a monitor's rectangle inside the work area's; None when
/// they do not meet.
#[cfg(any(
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ),
    test
))]
fn clip(monitor: Rect, work: Rect) -> Option<Rect> {
    let span = |a: i32, a_len: u32, b: i32, b_len: u32| {
        let start = a.max(b);
        let end = (i64::from(a) + i64::from(a_len)).min(i64::from(b) + i64::from(b_len));
        let len = u32::try_from(end - i64::from(start))
            .ok()
            .filter(|&n| n > 0)?;
        Some((start, len))
    };
    let (x, w) = span(monitor.0, monitor.2, work.0, work.2)?;
    let (y, h) = span(monitor.1, monitor.3, work.1, work.3)?;
    Some((x, y, w, h))
}

#[cfg(not(any(
    target_os = "macos",
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
)))]
fn platform(monitors: &[MonitorHandle]) -> Vec<Option<Rect>> {
    vec![None; monitors.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Mac's visible frame in its pixels: a 14-inch's 1512 x 982
    /// points at 2x, with the 37-point menu bar and a 70-point Dock on
    /// the left or at the foot; a frame off the main screen's origin; a
    /// scaled mode whose pixels are not twice its points; a visible
    /// frame past its screen held to it; and an empty frame.
    #[test]
    fn a_visible_frame_in_pixels() {
        let mac = (3024, 1964);
        let frame = [0.0, 0.0, 1512.0, 982.0];
        assert_eq!(
            visible_in_pixels(mac, frame, [70.0, 0.0, 1442.0, 945.0]),
            Some((140, 74, 2884, 1890))
        );
        assert_eq!(
            visible_in_pixels(mac, frame, [0.0, 70.0, 1512.0, 875.0]),
            Some((0, 74, 3024, 1750))
        );
        assert_eq!(
            visible_in_pixels(
                mac,
                [-1512.0, 200.0, 1512.0, 982.0],
                [-1442.0, 200.0, 1442.0, 945.0]
            ),
            Some((140, 74, 2884, 1890))
        );
        assert_eq!(
            visible_in_pixels(
                (3600, 2338),
                [0.0, 0.0, 1800.0, 1169.0],
                [0.0, 0.0, 1800.0, 1132.0]
            ),
            Some((0, 74, 3600, 2264))
        );
        assert_eq!(
            visible_in_pixels(mac, frame, [-10.0, -10.0, 2000.0, 2000.0]),
            Some((0, 0, 3024, 1964))
        );
        assert_eq!(visible_in_pixels(mac, [0.0, 0.0, 0.0, 982.0], frame), None);
        assert_eq!(visible_in_pixels(mac, frame, [0.0, 0.0, 0.0, 945.0]), None);
    }

    /// X11's one work area over two monitors side by side, with a
    /// 30-pixel panel along the top of the left one: each monitor gets
    /// its part, and a monitor off the area gets none.
    #[test]
    fn the_x11_work_area_clipped_to_each_monitor() {
        let work = (0, 30, 4480, 1410);
        assert_eq!(clip((0, 0, 1920, 1080), work), Some((0, 30, 1920, 1050)));
        assert_eq!(
            clip((1920, 0, 2560, 1440), work),
            Some((1920, 30, 2560, 1410))
        );
        assert_eq!(clip((5000, 0, 1920, 1080), work), None);
        assert_eq!(clip((-1920, 0, 1920, 1080), work), None);
    }
}
