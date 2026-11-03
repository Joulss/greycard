//! Every page the window opens in a browser: the guide, the bug
//! report's form, and the release page.
//!
//! On Linux and the BSDs we ask the desktop, through `xdg-open` (or
//! `gio open` where there is no `xdg-open`), before `webbrowser`,
//! which tries `$BROWSER` first: a `BROWSER` a shell profile set for
//! the terminal would otherwise beat the browser the desktop is set
//! to. On macOS and Windows `webbrowser` asks the system already.

/// Opens `url` in the desktop's browser. Callers keep it off the
/// event loop and out of tests. An opener that fails within its
/// answer time hands the page to `webbrowser`, as a missing one does.
pub(crate) fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some((program, args)) = opener(std::env::var_os("PATH").as_deref()) {
        return desktop::open(&program, args, url).or_else(|e| {
            tracing::info!("{e}; trying webbrowser instead");
            webbrowser::open(url)
        });
    }
    webbrowser::open(url)
}

/// The desktop's opener on `path` (a `PATH`), and the arguments
/// before the URL; nothing where neither is there. The dry form: it
/// names the program and runs nothing.
#[cfg(all(unix, not(target_os = "macos")))]
fn opener(path: Option<&std::ffi::OsStr>) -> Option<(std::path::PathBuf, &'static [&'static str])> {
    use std::os::unix::fs::PermissionsExt;
    let find = |name: &str| {
        std::env::split_paths(path?)
            .map(|d| d.join(name))
            .find(|p| {
                p.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
    };
    const NONE: &[&str] = &[];
    find("xdg-open")
        .map(|p| (p, NONE))
        .or_else(|| find("gio").map(|p| (p, &["open"][..])))
}

#[cfg(all(unix, not(target_os = "macos")))]
mod desktop {
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// How long the opener has to fail before we take it as opened.
    /// `xdg-open` returns once the browser has the page, except where
    /// it started the browser itself, when it waits for the browser
    /// to close; past this its status says nothing about the page.
    const ANSWER: Duration = Duration::from_secs(2);

    /// Runs the opener detached: its own process group, so a Ctrl-C
    /// in the terminal greycard was started from does not reach the
    /// browser, and no handles of ours. A failure within `ANSWER` is
    /// the caller's, which falls back to `webbrowser`; after it the
    /// child is reaped on a thread of its own.
    pub(super) fn open(program: &Path, args: &[&str], url: &str) -> std::io::Result<()> {
        let mut command = Command::new(program);
        command
            .args(args)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = crate::adapter::scrub(&mut command).spawn()?;
        let start = Instant::now();
        while start.elapsed() < ANSWER {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other(format!(
                        "{}: {status}",
                        program.display()
                    )))
                };
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn script(dir: &Path, name: &str, mode: u32) {
        let file = dir.join(name);
        std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn the_desktop_opener_goes_first() {
        let root = crate::testing::scratch_dir("browser-opener");
        let (xdg, gio, empty) = (root.join("xdg"), root.join("gio"), root.join("empty"));
        for d in [&xdg, &gio, &empty] {
            std::fs::create_dir_all(d).unwrap();
        }
        script(&xdg, "xdg-open", 0o755);
        script(&gio, "gio", 0o755);
        let path = |dirs: &[&Path]| std::env::join_paths(dirs).unwrap();

        // xdg-open wins wherever it is on the path, gio or not.
        let p: OsString = path(&[&gio, &xdg]);
        let (program, args) = opener(Some(&p)).unwrap();
        assert_eq!(program, xdg.join("xdg-open"));
        assert!(args.is_empty());

        // gio open where xdg-open is missing.
        let p = path(&[&empty, &gio]);
        let (program, args) = opener(Some(&p)).unwrap();
        assert_eq!(program, gio.join("gio"));
        assert_eq!(args, ["open"]);

        // Neither: webbrowser's turn, as with no PATH at all.
        assert!(opener(Some(&path(&[&empty]))).is_none());
        assert!(opener(None).is_none());

        // A file of that name that will not run is not an opener.
        script(&empty, "xdg-open", 0o644);
        let p = path(&[&empty, &gio]);
        assert_eq!(opener(Some(&p)).unwrap().0, gio.join("gio"));

        crate::testing::remove_dir_retry(&root);
    }
}
