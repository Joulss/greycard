//! A rename into place that waits out a moment's hold: on Windows a
//! scanner, a sync client, or an indexer that has the old file open
//! refuses the rename over it for as long as it holds it, often a few
//! milliseconds. Elsewhere a rename over an open file succeeds, so
//! there it is a plain rename.

use std::io;
use std::path::Path;
use std::time::Duration;

/// `ERROR_ACCESS_DENIED`: what Windows returns for a rename over a
/// file someone holds open without delete sharing, and for a folder
/// renamed while a file under it is open.
pub(crate) const ERROR_ACCESS_DENIED: i32 = 5;
/// `ERROR_SHARING_VIOLATION`: the file is open in a way that does not
/// share what the rename needs.
pub(crate) const ERROR_SHARING_VIOLATION: i32 = 32;
/// `ERROR_LOCK_VIOLATION`: a part of the file is locked.
pub(crate) const ERROR_LOCK_VIOLATION: i32 = 33;

/// The waits between tries: ten tries, the waits growing by 40 ms, so
/// 1.8 s in all before the last error is given up.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) const WAITS: [Duration; 9] = {
    let mut w = [Duration::ZERO; 9];
    let mut i = 0;
    while i < 9 {
        w[i] = Duration::from_millis(40 * (i as u64 + 1));
        i += 1;
    }
    w
};

/// Rename `from` over `to`. On Windows a refusal because another
/// process holds `to` (or `from`) open is tried again, ten tries over
/// 1.8 s in all; any other error, or the hold lasting past the last
/// try, is returned as the rename gave it.
pub fn rename_into_place(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        retry_held(|| std::fs::rename(from, to), &WAITS, to)
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(from, to)
    }
}

/// Whether `e` is Windows saying another process holds the file: one
/// of the three codes above. The codes are Windows' own, so this is
/// only asked there (and in the tests).
pub(crate) fn is_held(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
    )
}

/// `op` tried, then tried again after each of `waits` while it fails
/// with [`is_held`]. Logs once, at the end, how long it waited when it
/// waited at all. `what` names the file for the log.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) fn retry_held(
    op: impl FnMut() -> io::Result<()>,
    waits: &[Duration],
    what: &Path,
) -> io::Result<()> {
    retry(op, waits, is_held, what)
}

/// `op` tried, then tried again after each of `waits` while it fails
/// with an error `again` admits; the last error when none is left.
#[cfg_attr(not(any(windows, test, feature = "testing")), allow(dead_code))]
pub(crate) fn retry(
    mut op: impl FnMut() -> io::Result<()>,
    waits: &[Duration],
    again: impl Fn(&io::Error) -> bool,
    what: &Path,
) -> io::Result<()> {
    let mut waited = Duration::ZERO;
    let mut last = None;
    let mut waits = waits.iter();
    let result = loop {
        match op() {
            Err(e) if again(&e) => match waits.next() {
                Some(&w) => {
                    std::thread::sleep(w);
                    waited += w;
                    last = Some(e);
                }
                None => break Err(e),
            },
            other => break other,
        }
    };
    if !waited.is_zero() {
        match &result {
            Ok(()) => log::info!(
                "{}: renamed after {} ms of refusals, the last: {}",
                what.display(),
                waited.as_millis(),
                last.map(|e| e.to_string()).unwrap_or_default()
            ),
            Err(e) => log::warn!(
                "{}: still refused after {} ms: {e}",
                what.display(),
                waited.as_millis()
            ),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held() -> io::Error {
        io::Error::from_raw_os_error(ERROR_SHARING_VIOLATION)
    }

    const QUICK: [Duration; 9] = [Duration::from_millis(1); 9];

    #[test]
    fn the_schedule_is_ten_tries_over_under_two_seconds() {
        assert_eq!(WAITS.len() + 1, 10);
        assert_eq!(WAITS.iter().sum::<Duration>(), Duration::from_millis(1800));
        assert!(WAITS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_hold_let_go_is_renamed() {
        let mut tries = 0;
        let r = retry_held(
            || {
                tries += 1;
                if tries <= 2 { Err(held()) } else { Ok(()) }
            },
            &QUICK,
            Path::new("x"),
        );
        assert!(r.is_ok());
        assert_eq!(tries, 3);
    }

    #[test]
    fn a_hold_kept_gives_the_last_error_after_the_bound() {
        let mut tries = 0;
        let r = retry_held(
            || {
                tries += 1;
                Err(io::Error::from_raw_os_error(if tries == 10 {
                    ERROR_LOCK_VIOLATION
                } else {
                    ERROR_SHARING_VIOLATION
                }))
            },
            &QUICK,
            Path::new("x"),
        );
        assert_eq!(tries, 10);
        assert_eq!(r.unwrap_err().raw_os_error(), Some(ERROR_LOCK_VIOLATION));
    }

    #[test]
    fn another_error_is_not_tried_again() {
        let mut tries = 0;
        let r = retry_held(
            || {
                tries += 1;
                Err(io::Error::from(io::ErrorKind::NotFound))
            },
            &QUICK,
            Path::new("x"),
        );
        assert_eq!(tries, 1);
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn the_three_codes_are_held_and_no_other() {
        for code in [
            ERROR_ACCESS_DENIED,
            ERROR_SHARING_VIOLATION,
            ERROR_LOCK_VIOLATION,
        ] {
            assert!(is_held(&io::Error::from_raw_os_error(code)));
        }
        assert!(!is_held(&io::Error::from_raw_os_error(2)));
        assert!(!is_held(&io::Error::other("no code")));
    }

    #[test]
    fn a_real_rename_replaces_the_file() {
        let dir = std::env::temp_dir().join(format!("greycard-fs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("new"), dir.join("old"));
        std::fs::write(&from, b"new").unwrap();
        std::fs::write(&to, b"old").unwrap();
        rename_into_place(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"new");
        assert!(!from.exists());
        crate::testing::remove_dir_retry(&dir);
    }
}
