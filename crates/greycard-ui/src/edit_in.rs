//! Edit in another editor, as Lightroom hands a frame to one: the
//! command that gives an exported file to an app on the
//! Settings sheet's list, the name the sheet shows for that app, and the
//! stamp a written file is watched by. Nothing here starts a process
//! or needs a window; the window's side is `panel::edit_in`.
//!
//! Nothing goes through a shell: on a Mac `open -a App file`, on
//! Windows the program with the file as its argument, and elsewhere
//! the chosen executable, or a desktop entry through `gio launch`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The platform a command is built for: this one at run time, any of
/// them in a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

impl Platform {
    /// The platform this build runs on.
    pub fn this() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// A command as it is started: the program and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Launch {
    pub fn command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.program);
        cmd.args(&self.args);
        cmd
    }
}

/// `path`'s extension, lowercase.
fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Whether `app` is a desktop entry, which is launched rather than run.
pub fn is_desktop_entry(app: &Path) -> bool {
    extension(app) == "desktop"
}

/// The command that opens `file` in `app` on `platform`. A Mac's
/// application bundle goes through `open -a`, anything else chosen
/// there is run as a program; Windows runs the program; elsewhere a
/// desktop entry goes through `gio launch` and anything else is run.
/// The file is made absolute first, since the app starts in a folder
/// of its own.
pub fn launch(app: &Path, file: &Path, platform: Platform) -> Launch {
    let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    match platform {
        Platform::Mac if extension(app) == "app" => Launch {
            program: "open".into(),
            args: vec!["-a".into(), app.into(), file.into()],
        },
        Platform::Linux if is_desktop_entry(app) => Launch {
            program: "gio".into(),
            args: vec!["launch".into(), app.into(), file.into()],
        },
        _ => Launch {
            program: app.to_path_buf(),
            args: vec![file.into()],
        },
    }
}

/// What a desktop entry falls back on when `gio` is not installed:
/// `xdg-open` on the file, which opens it in the desktop's own choice
/// of app for the format rather than the one chosen.
pub fn fallback(file: &Path) -> Launch {
    let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    Launch {
        program: "xdg-open".into(),
        args: vec![file.into()],
    }
}

/// The `Name` of a desktop entry's `[Desktop Entry]` group, the
/// untranslated one.
pub fn desktop_name(text: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry
            && let Some(name) = line.strip_prefix("Name")
            && let Some(name) = name.trim_start().strip_prefix('=')
        {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Whether `a` and `b` are the same app: the same file once both
/// resolve, links and `..` followed, else the same path as written.
pub fn same_app(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The name the Settings sheet shows for `app`: a desktop entry's own
/// name when `entry` holds its text, else the file's name without an
/// app's extension (`Photoshop.app`, `Topaz.exe`, `gimp.desktop`).
pub fn app_name(app: &Path, entry: Option<&str>) -> String {
    if let Some(name) = entry.and_then(desktop_name) {
        return name;
    }
    let stem = || {
        app.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match extension(app).as_str() {
        "app" | "exe" | "desktop" | "appimage" => stem(),
        _ => app
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| app.display().to_string()),
    }
}

/// The folder the chooser for the app opens on: where the platform
/// keeps its apps.
pub fn apps_folder(platform: Platform) -> PathBuf {
    match platform {
        Platform::Mac => PathBuf::from("/Applications"),
        Platform::Windows => std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files")),
        Platform::Linux => PathBuf::from("/usr/share/applications"),
    }
}

/// What a watched file is told apart by: when it was last written and
/// its size. A save in another editor moves one or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub size: u64,
}

/// `path`'s stamp, none while it is not there.
pub fn stamp_of(path: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: m.modified().ok(),
        size: m.len(),
    })
}

/// A watched file's stamps, looked at once a tick: a new one is taken
/// only once it has held across two looks, so a writer still at work
/// on the file is waited for rather than read at every look.
#[derive(Debug, Clone, Default)]
pub struct Settle {
    /// The stamp last taken, or the file's as it was handed over.
    taken: Option<Stamp>,
    /// A new stamp seen once, waiting for the next look to agree.
    seen: Option<Stamp>,
}

impl Settle {
    pub fn from(taken: Option<Stamp>) -> Settle {
        Settle { taken, seen: None }
    }

    /// The stamp last taken, or the file's as it was handed over.
    pub fn taken(&self) -> Option<Stamp> {
        self.taken
    }

    /// The file's stamp now, none while it is not there: true when a
    /// new one has held since the last look, and is taken.
    pub fn look(&mut self, now: Option<Stamp>) -> bool {
        match now {
            Some(now) if Some(now) != self.taken => {
                if self.seen == Some(now) {
                    self.taken = Some(now);
                    self.seen = None;
                    true
                } else {
                    self.seen = Some(now);
                    false
                }
            }
            _ => {
                self.seen = None;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(l: &Launch) -> Vec<String> {
        l.args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_launch_is_built_per_platform() {
        let file = Path::new("/p/IMG_0001-edit.greycard.tif");
        // Handed over absolute, which on Windows puts the current
        // drive before a rooted path.
        let handed = std::path::absolute(file).unwrap();
        let handed = handed.to_str().unwrap();
        let mac = launch(
            Path::new("/Applications/Photoshop.app"),
            file,
            Platform::Mac,
        );
        assert_eq!(mac.program, Path::new("open"));
        assert_eq!(args(&mac), ["-a", "/Applications/Photoshop.app", handed]);
        // A plain program chosen on a Mac is run.
        let tool = launch(Path::new("/usr/local/bin/tool"), file, Platform::Mac);
        assert_eq!(tool.program, Path::new("/usr/local/bin/tool"));
        assert_eq!(args(&tool), [handed]);

        let exe = Path::new(r"C:\Program Files\Topaz\Topaz.exe");
        let win = launch(exe, file, Platform::Windows);
        assert_eq!(win.program, exe);
        assert_eq!(win.args.len(), 1);

        let entry = Path::new("/usr/share/applications/gimp.desktop");
        let gio = launch(entry, file, Platform::Linux);
        assert_eq!(gio.program, Path::new("gio"));
        assert_eq!(
            args(&gio),
            ["launch", "/usr/share/applications/gimp.desktop", handed]
        );
        let run = launch(Path::new("/usr/bin/gimp"), file, Platform::Linux);
        assert_eq!(run.program, Path::new("/usr/bin/gimp"));
        assert_eq!(args(&run), [handed]);
        let back = fallback(file);
        assert_eq!(back.program, Path::new("xdg-open"));
        assert_eq!(args(&back), [handed]);
    }

    #[test]
    fn a_relative_file_is_handed_over_whole() {
        let l = launch(
            Path::new("/usr/bin/gimp"),
            Path::new("a.tif"),
            Platform::Linux,
        );
        let handed = PathBuf::from(&l.args[0]);
        assert!(handed.is_absolute(), "{}", handed.display());
        assert!(handed.ends_with("a.tif"));
    }

    #[test]
    fn the_app_is_named_as_the_desktop_names_it() {
        assert_eq!(
            app_name(Path::new("/Applications/Photoshop.app"), None),
            "Photoshop"
        );
        assert_eq!(app_name(Path::new("Topaz.EXE"), None), "Topaz");
        assert_eq!(app_name(Path::new("/usr/bin/gimp"), None), "gimp");
        let entry = "[Desktop Entry]\nType=Application\nName[de]=GIMP-Bildbearbeitung\n\
                     Name = GNU Image Manipulation Program\nExec=gimp %U\n\
                     [Desktop Action new]\nName=New Window\n";
        assert_eq!(
            app_name(
                Path::new("/usr/share/applications/gimp.desktop"),
                Some(entry)
            ),
            "GNU Image Manipulation Program"
        );
        assert_eq!(
            app_name(Path::new("/x/gimp.desktop"), Some("[Other]\nName=No\n")),
            "gimp"
        );
    }

    fn at(secs: u64, size: u64) -> Option<Stamp> {
        Some(Stamp {
            modified: Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)),
            size,
        })
    }

    /// A writer at work moves the stamp at every look: nothing is taken
    /// until a stamp holds across two looks, and then once.
    #[test]
    fn a_save_is_taken_once_it_has_settled() {
        let mut s = Settle::from(at(0, 10));
        assert!(!s.look(at(0, 10)), "as handed over");
        assert!(!s.look(at(1, 20)), "written");
        assert!(!s.look(at(2, 30)), "still being written");
        assert!(s.look(at(2, 30)), "settled");
        assert!(!s.look(at(2, 30)), "taken once");
        assert!(!s.look(None), "gone");
        assert!(!s.look(at(3, 30)), "back, new");
        assert!(!s.look(at(2, 30)), "back as taken");
        assert!(!s.look(at(3, 30)), "new again, seen once");
        assert!(s.look(at(3, 30)), "settled again");
    }
}
