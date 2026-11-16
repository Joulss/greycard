//! Stamps the build with its commit, as `GREYCARD_COMMIT` and
//! `GREYCARD_LABEL` for the crate to read. Never fails: no git, no
//! repository, or a repository that is not this workspace's gives no
//! commit, and the label is the plain version.

use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "src/stamp.rs"]
mod stamp;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/stamp.rs");
    println!("cargo:rerun-if-env-changed=GREYCARD_COMMIT");
    println!("cargo:rerun-if-env-changed=GREYCARD_RELEASE");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let commit = commit(&version);
    println!("cargo:rustc-env=GREYCARD_COMMIT={commit}");
    println!(
        "cargo:rustc-env=GREYCARD_LABEL={}",
        stamp::label(&version, &commit)
    );
}

/// The commit part of the label: a packager's `GREYCARD_COMMIT` if
/// set, nothing for a release, else git's word on HEAD.
fn commit(version: &str) -> String {
    if let Some(v) = std::env::var_os("GREYCARD_COMMIT") {
        return stamp::overridden(&v.to_string_lossy())
            .unwrap_or_default()
            .to_string();
    }
    let release = std::env::var("GREYCARD_RELEASE").is_ok_and(|v| !v.is_empty() && v != "0");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let root = manifest.join("../..");
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    // A source tarball unpacked inside some other repository must not
    // take that repository's commit.
    let ours = git(&manifest, &["rev-parse", "--show-toplevel"])
        .and_then(|t| std::fs::canonicalize(t.trim()).ok())
        .is_some_and(|t| t == root);
    if !ours {
        return String::new();
    }
    watch(&manifest);
    let Some(out) = git(&manifest, &["rev-parse", "--short=7", "HEAD"]) else {
        return String::new();
    };
    let dirty = git(
        &manifest,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .is_some_and(|s| stamp::dirty(&s));
    // At the tag with changes on top is not the release.
    let release = release
        || (!dirty
            && git(
                &manifest,
                &[
                    "describe",
                    "--exact-match",
                    "--tags",
                    "--match",
                    &format!("v{version}"),
                    "HEAD",
                ],
            )
            .is_some_and(|t| stamp::is_release_tag(&t, version)));
    stamp::commit(release, stamp::hash(&out), dirty)
}

/// The files that move with this worktree's HEAD, and nothing else:
/// its `HEAD` and its reflog `logs/HEAD`, both under the per-worktree
/// git dir; the reflog is appended at every commit, checkout, reset,
/// merge, and rebase. No source file, no index, nothing shared with
/// another worktree, so an edit or another worktree's commit never
/// reruns this. The `+` is therefore the tree as it stood at the first
/// build after HEAD moved; a later uncommitted edit does not add it.
/// Only where there is no reflog (reflogs off, some CI clones, whose
/// builds are fresh anyway) are the branch HEAD names and the shared
/// packed-refs watched instead. Only paths that exist, since cargo
/// reruns at every build for a missing one.
fn watch(manifest: &Path) {
    let dir = |arg: &str| {
        git(manifest, &["rev-parse", arg]).map(|d| {
            let d = PathBuf::from(d.trim());
            if d.is_absolute() { d } else { manifest.join(d) }
        })
    };
    let mut paths = Vec::new();
    if let Some(git_dir) = dir("--git-dir") {
        paths.push(git_dir.join("HEAD"));
        let reflog = git_dir.join("logs/HEAD");
        if reflog.exists() {
            paths.push(reflog);
        } else if let Some(common) = dir("--git-common-dir") {
            paths.push(common.join("packed-refs"));
            if let Some(r) = git(manifest, &["symbolic-ref", "-q", "HEAD"]) {
                let loose = common.join(r.trim());
                // A packed branch gets its loose file at the next
                // commit, in a folder that already exists.
                match loose.exists() {
                    true => paths.push(loose),
                    false => paths.extend(loose.parent().map(Path::to_path_buf)),
                }
            }
        }
    }
    for p in paths.into_iter().filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", p.display());
    }
}

/// `git <args>` in `dir`, its output if it ran and succeeded. The
/// variables a hook sets are dropped so git finds the repository from
/// `dir`, and optional locks are off so `status` does not rewrite the
/// index under a build.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
