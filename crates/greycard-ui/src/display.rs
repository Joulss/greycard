//! The monitors the system's color manager knows, and their profiles:
//! colord's on Linux, ColorSync's on macOS. The display table itself,
//! from the output's profile to the monitor's, is the viewport's.

use anyhow::Result;

pub use greycard_viewport::display::{
    LUT_SIZE, Lut3d, MonitorProfile, Proof, ProofIntent, ProofProfile,
};

/// Where "System" takes the profile from, for the panel's note.
pub const SYSTEM_SOURCE: &str = if cfg!(target_os = "macos") {
    "ColorSync"
} else {
    "colord"
};

/// A monitor the system's color manager knows, with its current
/// profile.
#[derive(Debug, Clone)]
pub struct Monitor {
    pub model: String,
    /// Only colord and ColorSync say which display is the primary
    /// one, and only their own sort reads this; the field stays
    /// either way so the type is one type.
    #[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
    pub primary: bool,
    pub profile: Option<std::path::PathBuf>,
}

/// The displays the system's color manager knows, primary first,
/// each with the path of its current profile if it has one: colord's
/// on Linux, ColorSync's on macOS. Until the compositor can say which
/// monitor a window is on, the primary is the best guess.
#[cfg(target_os = "linux")]
pub fn system_monitors() -> Result<Vec<Monitor>> {
    colord_monitors()
}

#[cfg(target_os = "macos")]
pub fn system_monitors() -> Result<Vec<Monitor>> {
    Ok(monitors_from(
        colorsync::displays(),
        profile_cache().as_deref(),
    ))
}

/// Windows' WCS is not read yet: no monitor, and the output falls
/// back to sRGB.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn system_monitors() -> Result<Vec<Monitor>> {
    Ok(Vec::new())
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn sort_primary_first(monitors: &mut [Monitor]) {
    monitors.sort_by_key(|m| !m.primary);
}

/// One display as ColorSync reports it, before its profile is a file.
#[cfg(any(target_os = "macos", test))]
struct Display {
    id: u32,
    main: bool,
    name: Option<String>,
    icc: Option<Vec<u8>>,
}

/// Where the displays' profiles are written: beside the thumbnail
/// cache, under `$XDG_CACHE_HOME` when that is set, else the
/// platform's cache.
#[cfg(target_os = "macos")]
fn profile_cache() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(dirs::cache_dir)
        .map(|c| c.join("greycard").join("monitors"))
}

/// The displays as monitors, primary first, each display's ICC data
/// written to `<cache>/<id>.icc` so a profile is a path here as it is
/// from colord. Without a cache, or when the write fails, the display
/// is listed without a profile.
#[cfg(any(target_os = "macos", test))]
fn monitors_from(displays: Vec<Display>, cache: Option<&std::path::Path>) -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = displays
        .into_iter()
        .map(|d| {
            let model = d.name.unwrap_or_else(|| format!("Display {}", d.id));
            let profile = match (d.icc, cache) {
                (Some(icc), Some(dir)) => match cache_profile(dir, d.id, &icc) {
                    Ok((path, _)) => Some(path),
                    Err(e) => {
                        tracing::warn!("ColorSync: {model}'s profile not cached: {e:#}");
                        None
                    }
                },
                _ => None,
            };
            Monitor {
                model,
                primary: d.main,
                profile,
            }
        })
        .collect();
    sort_primary_first(&mut monitors);
    for m in &monitors {
        let path = m
            .profile
            .as_ref()
            .map_or("no profile".to_string(), |p| p.display().to_string());
        let primary = if m.primary { " (primary)" } else { "" };
        tracing::info!("ColorSync: {}{primary}: {path}", m.model);
    }
    monitors
}

/// `icc` as `<dir>/<id>.icc`, written only when the file is missing
/// or holds other bytes, through a temporary file renamed over it so
/// a second editor never reads half a profile. True when written.
#[cfg(any(target_os = "macos", test))]
fn cache_profile(dir: &std::path::Path, id: u32, icc: &[u8]) -> Result<(std::path::PathBuf, bool)> {
    use anyhow::Context;
    let path = dir.join(format!("{id}.icc"));
    if std::fs::read(&path).is_ok_and(|old| old == icc) {
        return Ok((path, false));
    }
    std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    let tmp = dir.join(format!("{id}.icc.{}.tmp", std::process::id()));
    let written = std::fs::write(&tmp, icc)
        .with_context(|| tmp.display().to_string())
        .and_then(|()| std::fs::rename(&tmp, &path).with_context(|| path.display().to_string()));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map(|()| (path, true))
}

/// The active displays from CoreGraphics, each with its ColorSync
/// profile's ICC data and the name AppKit gives it.
#[cfg(target_os = "macos")]
mod colorsync {
    use std::ptr::NonNull;

    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSScreen;
    use objc2_core_foundation::CFRetained;
    use objc2_core_graphics::{
        CGColorSpace, CGDirectDisplayID, CGError, CGGetActiveDisplayList, CGMainDisplayID,
    };
    use objc2_foundation::{NSNumber, ns_string};

    use super::Display;

    // The crate's own binding panics when a display has no color
    // space; this one says so.
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayCopyColorSpace(display: CGDirectDisplayID) -> Option<NonNull<CGColorSpace>>;
    }

    pub(super) fn displays() -> Vec<Display> {
        let mut ids = [0 as CGDirectDisplayID; 16];
        let mut count = 0u32;
        // SAFETY: the list holds `ids.len()` entries and the count is
        // a valid u32.
        let err = unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
        if err != CGError::Success {
            tracing::warn!("ColorSync: no display list (CGError {})", err.0);
            return Vec::new();
        }
        let names = screen_names();
        let main = CGMainDisplayID();
        ids[..count.min(ids.len() as u32) as usize]
            .iter()
            .map(|&id| Display {
                id,
                main: id == main,
                name: names
                    .iter()
                    .find(|(n, _)| *n == id)
                    .map(|(_, name)| name.clone()),
                icc: icc_data(id),
            })
            .collect()
    }

    fn icc_data(id: CGDirectDisplayID) -> Option<Vec<u8>> {
        // SAFETY: a display id from the active list; a null return is
        // handled, and a Copy function's result is ours to release.
        let space =
            unsafe { CGDisplayCopyColorSpace(id) }.map(|p| unsafe { CFRetained::from_raw(p) })?;
        CGColorSpace::icc_data(Some(&space)).map(|d| d.to_vec())
    }

    /// Each screen's display id and localized name; nothing off the
    /// main thread, where AppKit cannot be asked.
    fn screen_names() -> Vec<(CGDirectDisplayID, String)> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Vec::new();
        };
        NSScreen::screens(mtm)
            .iter()
            .filter_map(|screen| {
                let description = screen.deviceDescription();
                let number: Retained<AnyObject> =
                    description.objectForKey(ns_string!("NSScreenNumber"))?;
                let number = number.downcast_ref::<NSNumber>()?;
                Some((
                    number.unsignedIntValue(),
                    screen.localizedName().to_string(),
                ))
            })
            .collect()
    }
}

/// The displays colord manages, primary first, each with the path of
/// its current profile if it has one.
#[cfg(target_os = "linux")]
fn colord_monitors() -> Result<Vec<Monitor>> {
    use anyhow::Context;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::OwnedObjectPath;
    const SERVICE: &str = "org.freedesktop.ColorManager";
    let conn = Connection::system().context("the system bus")?;
    let manager =
        Proxy::new(&conn, SERVICE, "/org/freedesktop/ColorManager", SERVICE).context("colord")?;
    let devices: Vec<OwnedObjectPath> = manager
        .call("GetDevicesByKind", &("display",))
        .context("asking colord for displays")?;
    let mut monitors = Vec::new();
    for path in devices {
        let device = Proxy::new(&conn, SERVICE, path, "org.freedesktop.ColorManager.Device")?;
        let model: String = device.get_property("Model").unwrap_or_default();
        let metadata: std::collections::HashMap<String, String> =
            device.get_property("Metadata").unwrap_or_default();
        let profiles: Vec<OwnedObjectPath> = device.get_property("Profiles").unwrap_or_default();
        let profile = match profiles.first() {
            Some(p) => {
                let profile = Proxy::new(
                    &conn,
                    SERVICE,
                    p.clone(),
                    "org.freedesktop.ColorManager.Profile",
                )?;
                profile
                    .get_property::<String>("Filename")
                    .ok()
                    .filter(|f| !f.is_empty())
                    .map(std::path::PathBuf::from)
            }
            None => None,
        };
        monitors.push(Monitor {
            model,
            primary: metadata
                .get("OutputPriority")
                .is_some_and(|p| p == "primary"),
            profile,
        });
    }
    sort_primary_first(&mut monitors);
    Ok(monitors)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("greycard-{name}-{}", std::process::id()));
        greycard_render::testing::remove_dir_retry(&dir);
        dir
    }

    #[test]
    fn a_cached_profile_is_rewritten_only_when_its_bytes_change() {
        let dir = scratch("monitor-cache");
        let (path, wrote) = cache_profile(&dir, 7, b"first").unwrap();
        assert!(wrote);
        assert_eq!(path, dir.join("7.icc"));
        assert_eq!(
            cache_profile(&dir, 7, b"first").unwrap(),
            (path.clone(), false)
        );
        assert_eq!(
            cache_profile(&dir, 7, b"second").unwrap(),
            (path.clone(), true)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        // Nothing left over from the rename.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        greycard_render::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn the_main_display_comes_first_and_each_has_a_name() {
        let dir = scratch("monitor-list");
        let display = |id, main, name: Option<&str>, icc: Option<&[u8]>| Display {
            id,
            main,
            name: name.map(str::to_string),
            icc: icc.map(<[u8]>::to_vec),
        };
        let monitors = monitors_from(
            vec![
                display(3, false, Some("Studio Display"), Some(b"p3")),
                display(1, true, None, Some(b"xdr")),
                display(5, false, None, None),
            ],
            Some(&dir),
        );
        let names: Vec<&str> = monitors.iter().map(|m| m.model.as_str()).collect();
        assert_eq!(names, ["Display 1", "Studio Display", "Display 5"]);
        assert!(monitors[0].primary && !monitors[1].primary);
        assert_eq!(monitors[0].profile, Some(dir.join("1.icc")));
        assert_eq!(std::fs::read(dir.join("3.icc")).unwrap(), b"p3");
        assert_eq!(monitors[2].profile, None);
        // Without a cache, no profile is a path.
        let bare = monitors_from(vec![display(1, true, None, Some(b"xdr"))], None);
        assert_eq!(bare[0].profile, None);
        greycard_render::testing::remove_dir_retry(&dir);
    }
}
