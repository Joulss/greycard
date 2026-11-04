//! Helpers every crate's tests share: a scratch folder's removal that
//! Windows would also allow. Behind `cfg(test)` or the `testing`
//! feature, which the other crates' dev-dependencies turn on, so it
//! never reaches a release build.

/// A test's folder removed, tried again for a while when the system
/// refuses: on Windows a file a thread has just let go of can still be
/// held for a moment after the thread was joined. The last refusal is
/// the panic.
///
/// Linux removes a file that is still open, Windows does not, and a
/// test that leaves the index's reader or a mapped raw under its
/// folder passes here and fails there, twenty minutes later and
/// without a name. So on Linux the removal first waits for what
/// Windows would refuse, the SQLite files and the memory maps this
/// process still holds under the folder, and names them when they
/// are still held at the end of the wait. Plain open files are not
/// counted: Rust opens them with delete sharing, and Windows removes
/// those as Linux does.
///
/// Called while the thread unwinds (from a drop guard in a failing
/// test) the removal is tried once and never panics: a second panic
/// would abort the run and hide the first failure.
pub fn remove_dir_retry(dir: &std::path::Path) {
    if std::thread::panicking() {
        // remove_dir_all allowed: best effort while unwinding.
        let _ = std::fs::remove_dir_all(dir);
        return;
    }
    let start = std::time::Instant::now();
    let limit = std::time::Duration::from_secs(20);
    loop {
        let held = still_held_under(dir);
        if !held.is_empty() {
            if start.elapsed() < limit {
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
            panic!(
                "{} is still held by this process, which Windows would refuse to remove: \
                 {held:?}; close the index reader (testing::remove_scratch) or stop the \
                 worker before removing the folder",
                dir.display()
            );
        }
        // remove_dir_all allowed: the helper itself.
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) if start.elapsed() < limit => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => panic!("{} could not be removed: {e}", dir.display()),
        }
    }
}

/// The files under `dir` this process holds in a way Windows would
/// refuse to delete: SQLite databases and their journals open as
/// descriptors (SQLite opens without delete sharing), and any file
/// mapped into memory (a mapped file cannot be deleted there). From
/// `/proc`, so Linux only; elsewhere the removal itself is the check.
#[cfg(target_os = "linux")]
fn still_held_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut held = Vec::new();
    if let Ok(fds) = std::fs::read_dir("/proc/self/fd") {
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let name = target.to_string_lossy();
            let sqlite = [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"]
                .iter()
                .any(|end| name.ends_with(end));
            if sqlite && target.starts_with(&dir) && !held.contains(&target) {
                held.push(target);
            }
        }
    }
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        for line in maps.lines() {
            let Some(at) = line.find('/') else { continue };
            let path = std::path::PathBuf::from(line[at..].trim_end_matches(" (deleted)"));
            if path.starts_with(&dir) && !held.contains(&path) {
                held.push(path);
            }
        }
    }
    held
}

#[cfg(not(target_os = "linux"))]
fn still_held_under(_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    Vec::new()
}

/// A file or a folder renamed, tried again for a while when the system
/// refuses: on Windows a folder cannot be renamed while any file under
/// it is open, and a test's thumbnail pool or worker may still be
/// reading one when the test gets here. The same retry as
/// [`crate::fs::rename_into_place`], on any error and for 20 s rather
/// than two, since a test's own threads can hold a file longer than
/// another process's moment. The last refusal is the panic.
pub fn rename_retry(from: impl AsRef<std::path::Path>, to: impl AsRef<std::path::Path>) {
    let (from, to) = (from.as_ref(), to.as_ref());
    let waits = [std::time::Duration::from_millis(50); 400];
    if let Err(e) = crate::fs::retry(|| std::fs::rename(from, to), &waits, |_| true, from) {
        panic!("{} could not be renamed: {e}", from.display());
    }
}
