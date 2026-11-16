//! The build's version, as data and as a label.
//!
//! [`VERSION`] is the crate version, `0.11.0`: what the update check
//! compares and what sidecars, exports, and caches record. [`LABEL`]
//! is what a person reads (the log's first line, `--version`, About,
//! a problem report): the same for a release, and for any other build
//! from a git checkout the commit after it, `0.11.0 (79527a9)`, with a
//! `+` when tracked files had changes at the first build after HEAD
//! last moved (a later uncommitted edit does not add it; the stamp
//! reruns only when HEAD moves). The build script stamps it; a
//! packager may set `GREYCARD_COMMIT` at build time instead, and the
//! release workflow sets `GREYCARD_RELEASE`. Kept in a crate of its
//! own so a new commit rebuilds only it and the two binaries' crates.

// The rest of it is the build script's, tested here.
#[allow(dead_code)]
mod stamp;

pub use stamp::label;

/// The crate version, for comparing and recording.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit part of the label, empty for a release or a build with
/// no git.
pub const COMMIT: &str = env!("GREYCARD_COMMIT");

/// The version as shown to a person.
pub const LABEL: &str = env!("GREYCARD_LABEL");

#[cfg(test)]
mod tests {
    use super::stamp::*;
    use super::*;

    #[test]
    fn a_release_is_the_plain_version() {
        assert_eq!(
            label("0.11.0", &commit(true, Some("79527a9"), false)),
            "0.11.0"
        );
    }

    #[test]
    fn a_release_said_by_the_workflow_ignores_changes() {
        assert_eq!(commit(true, Some("79527a9"), true), "");
        assert_eq!(commit(true, None, false), "");
    }

    #[test]
    fn another_build_carries_its_commit() {
        assert_eq!(
            label("0.11.0", &commit(false, Some("79527a9"), false)),
            "0.11.0 (79527a9)"
        );
    }

    #[test]
    fn uncommitted_changes_add_a_plus() {
        assert_eq!(
            label("0.11.0", &commit(false, Some("79527a9"), true)),
            "0.11.0 (79527a9+)"
        );
    }

    #[test]
    fn no_git_is_the_plain_version() {
        assert_eq!(label("0.11.0", &commit(false, None, true)), "0.11.0");
    }

    #[test]
    fn git_output_parses() {
        assert_eq!(hash("79527a9\n"), Some("79527a9"));
        assert_eq!(hash("fatal: not a git repository\n"), None);
        assert_eq!(hash(""), None);
        assert_eq!(hash("79527"), None);
        assert!(is_release_tag("v0.11.0\n", "0.11.0"));
        assert!(!is_release_tag("v0.11.0-1-g79527a9\n", "0.11.0"));
        assert!(!is_release_tag("v0.10.0\n", "0.11.0"));
        assert!(!is_release_tag("", "0.11.0"));
        assert!(dirty(" M crates/greycard-ui/src/log.rs\n"));
        assert!(!dirty("\n"));
    }

    #[test]
    fn a_packagers_commit_is_kept_if_plain() {
        assert_eq!(overridden(" 79527a9 \n"), Some("79527a9"));
        assert_eq!(overridden(""), Some(""));
        assert_eq!(overridden("abc\ndef"), None);
        assert_eq!(overridden("a b"), None);
    }

    /// This build's own stamp follows the rules.
    #[test]
    fn the_label_is_the_version_and_the_commit() {
        assert_eq!(LABEL, label(VERSION, COMMIT));
        assert!(LABEL.starts_with(VERSION));
    }
}
