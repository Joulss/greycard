//! Which way the system scrolls: whether a swipe up comes as a
//! scroll down, content following the fingers.
//!
//! A Mac does that by default, its "natural" scrolling, for the
//! trackpad and the mouse wheel alike, and the delta reaches a slider
//! already turned: a swipe up nudges it down. A Windows precision
//! touchpad and a GNOME touchpad do it by default too, but only the
//! touchpad: a mouse wheel beside it keeps its own way, which GNOME
//! sets separately.
//! The sliders turn a turned delta back; lists and the picture keep
//! the system's way, where content following the fingers is right.

use std::sync::OnceLock;

/// Whether each kind of device scrolls naturally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Natural {
    /// A mouse wheel.
    pub wheel: bool,
    /// A touchpad.
    pub touchpad: bool,
}

impl Natural {
    /// Whether a delta from a touchpad, or else from a wheel, comes
    /// turned.
    pub fn turned(self, touchpad: bool) -> bool {
        if touchpad { self.touchpad } else { self.wheel }
    }
}

static READ: OnceLock<Natural> = OnceLock::new();

/// The system's setting, read once, at the first call:
/// - macOS: `com.apple.swipescrolldirection`, natural when it is
///   absent as it is by default, for both kinds.
/// - Windows: the precision touchpad's `ScrollDirection` under
///   `HKCU\Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad`,
///   natural at 0 or absent; a wheel never.
/// - GNOME: `natural-scroll` of `org.gnome.desktop.peripherals`'
///   `touchpad` and `mouse`, through `gsettings`; a read that fails
///   is not natural.
///
/// Elsewhere neither.
pub fn natural() -> Natural {
    *READ.get_or_init(|| {
        let natural = read();
        match (natural.wheel, natural.touchpad) {
            (true, true) => tracing::info!("natural scrolling: a swipe up moves a slider up"),
            (false, true) => {
                tracing::info!("natural scrolling on the touchpad: a swipe up moves a slider up")
            }
            (true, false) => {
                tracing::info!("natural scrolling on the wheel: a wheel up moves a slider up")
            }
            (false, false) => {}
        }
        natural
    })
}

/// For the wheel event being handled, whether its delta comes turned,
/// when the kinds differ; None while they do not, or before the
/// setting is read, the start's value standing. `pixels`: whether the
/// delta came in pixels rather than lines.
pub fn for_event(pixels: bool) -> Option<bool> {
    for_event_in(*READ.get()?, || from_touchpad(pixels))
}

/// [`for_event`] under `natural`, `touchpad` asked only where the
/// kinds differ.
fn for_event_in(natural: Natural, touchpad: impl FnOnce() -> bool) -> Option<bool> {
    (natural.wheel != natural.touchpad).then(|| natural.turned(touchpad()))
}

/// Whether the wheel event being handled comes from a touchpad.
/// Wayland sends a finger's scroll in pixels and a wheel's in notches,
/// which winit passes on as lines; X11 sends both as lines, so a
/// touchpad there is read as a wheel.
#[cfg(target_os = "linux")]
fn from_touchpad(pixels: bool) -> bool {
    pixels
}

/// Windows sends both as lines, so the message's source says: the
/// event is handled within the window procedure of its message.
#[cfg(windows)]
fn from_touchpad(_pixels: bool) -> bool {
    use windows_sys::Win32::UI::Input::{
        GetCurrentInputMessageSource, IMDT_TOUCHPAD, INPUT_MESSAGE_SOURCE,
    };
    let mut source = INPUT_MESSAGE_SOURCE {
        deviceType: 0,
        originId: 0,
    };
    // SAFETY: the out-parameter is a valid, writable structure.
    let ok = unsafe { GetCurrentInputMessageSource(&mut source) } != 0;
    ok && source.deviceType == IMDT_TOUCHPAD
}

#[cfg(not(any(target_os = "linux", windows)))]
fn from_touchpad(_pixels: bool) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn read() -> Natural {
    use objc2_core_foundation::{
        CFPreferencesGetAppBooleanValue, CFString, kCFPreferencesAnyApplication,
    };
    let key = CFString::from_static_str("com.apple.swipescrolldirection");
    let mut valid = 0u8;
    // SAFETY: the out-parameter is a valid Boolean, and the any
    // application name is the framework's own constant.
    let on =
        unsafe { CFPreferencesGetAppBooleanValue(&key, kCFPreferencesAnyApplication, &mut valid) };
    let natural = on || valid == 0;
    Natural {
        wheel: natural,
        touchpad: natural,
    }
}

#[cfg(windows)]
fn read() -> Natural {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad");
    let name = wide("ScrollDirection");
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: both names are NUL-terminated, and the data pointer and
    // its size are a valid, writable DWORD.
    let error = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    Natural {
        wheel: false,
        touchpad: touchpad_natural((error == 0).then_some(value)),
    }
}

/// The precision touchpad's `ScrollDirection`: 0 is "Down motion
/// scrolls up", content following the fingers, and the default, so
/// natural too when absent; 0xFFFFFFFF is "Down motion scrolls down".
#[cfg(any(windows, test))]
fn touchpad_natural(scroll_direction: Option<u32>) -> bool {
    matches!(scroll_direction, None | Some(0))
}

#[cfg(target_os = "linux")]
fn read() -> Natural {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !gnome(&desktop) {
        return Natural::default();
    }
    let get = |device: &str| {
        std::process::Command::new("gsettings")
            .args([
                "get",
                &format!("org.gnome.desktop.peripherals.{device}"),
                "natural-scroll",
            ])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| boolean(&String::from_utf8_lossy(&out.stdout)))
            .unwrap_or(false)
    };
    Natural {
        wheel: get("mouse"),
        touchpad: get("touchpad"),
    }
}

/// Whether the desktop is GNOME or one built on it: one of the names
/// in `XDG_CURRENT_DESKTOP`, as in "ubuntu:GNOME". Elsewhere GNOME's
/// schemas may be installed but not the ones the compositor reads.
#[cfg(any(target_os = "linux", test))]
fn gnome(desktop: &str) -> bool {
    desktop.split(':').any(|name| name == "GNOME")
}

/// A boolean as `gsettings get` prints it.
#[cfg(any(target_os = "linux", test))]
fn boolean(printed: &str) -> Option<bool> {
    match printed.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn read() -> Natural {
    Natural::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_sets_the_way_only_where_the_kinds_differ() {
        let natural = |wheel, touchpad| Natural { wheel, touchpad };
        for same in [natural(true, true), natural(false, false)] {
            for device in [true, false] {
                assert_eq!(for_event_in(same, || device), None, "{same:?}");
            }
        }
        assert_eq!(for_event_in(natural(false, true), || true), Some(true));
        assert_eq!(for_event_in(natural(false, true), || false), Some(false));
        assert_eq!(for_event_in(natural(true, false), || true), Some(false));
        assert_eq!(for_event_in(natural(true, false), || false), Some(true));
    }

    #[test]
    fn a_precision_touchpad_is_natural_at_zero_or_unset() {
        assert!(touchpad_natural(None));
        assert!(touchpad_natural(Some(0)));
        assert!(!touchpad_natural(Some(0xFFFF_FFFF)));
    }

    #[test]
    fn gnome_is_read_on_gnome_and_its_derivatives_only() {
        assert!(gnome("GNOME"));
        assert!(gnome("ubuntu:GNOME"));
        assert!(gnome("GNOME-Classic:GNOME"));
        assert!(!gnome("KDE"));
        assert!(!gnome("X-Cinnamon"));
        assert!(!gnome(""));
    }

    #[test]
    fn gsettings_prints_a_boolean_or_nothing_read() {
        assert_eq!(boolean("true\n"), Some(true));
        assert_eq!(boolean("false\n"), Some(false));
        assert_eq!(boolean(""), None);
        assert_eq!(boolean("No such schema"), None);
    }
}
