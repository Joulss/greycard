//! The label's rules, shared by the build script (which includes this
//! file by path) and the crate's tests. Pure: git's output in, the
//! label out.

/// The label shown for `version` and the commit part `commit`:
/// `0.11.0` when there is none, else `0.11.0 (79527a9)`.
pub fn label(version: &str, commit: &str) -> String {
    if commit.is_empty() {
        version.to_string()
    } else {
        format!("{version} ({commit})")
    }
}

/// The commit part: empty for a release or with no hash, else the
/// hash with a `+` when tracked files had changes.
pub fn commit(release: bool, hash: Option<&str>, dirty: bool) -> String {
    match hash {
        Some(h) if !release => format!("{h}{}", if dirty { "+" } else { "" }),
        _ => String::new(),
    }
}

/// `git rev-parse --short=7 HEAD`'s output as a hash, or none for
/// anything that is not one.
pub fn hash(out: &str) -> Option<&str> {
    let h = out.trim();
    (h.len() >= 7 && h.len() <= 64 && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(h)
}

/// Whether `git describe --exact-match --tags` named this version's
/// own tag, `v<version>`.
pub fn is_release_tag(out: &str, version: &str) -> bool {
    out.trim().strip_prefix('v') == Some(version)
}

/// Whether `git status --porcelain --untracked-files=no` listed any
/// change.
pub fn dirty(out: &str) -> bool {
    !out.trim().is_empty()
}

/// A packager's `GREYCARD_COMMIT`, kept only if it is a plain token:
/// letters, digits, and `+._-`, at most 41 characters. Anything else
/// gives no commit rather than odd text in a report.
pub fn overridden(value: &str) -> Option<&str> {
    let v = value.trim();
    (v.len() <= 41
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+._-".contains(&b)))
    .then_some(v)
}
