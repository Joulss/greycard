//! A frame's copies on the archives, found by the index alone, and a
//! save made with no window open noted as waiting in the index for the
//! next window's catch-up to write to them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use greycard_edit_io::on_disk::file_name;

use crate::Library;

/// A frame's copy on one archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub archive: PathBuf,
    pub copy: PathBuf,
}

/// A frame's copies on the archives, by the index alone: its content
/// hash, exactly one copy per archive that has one, and the archives
/// where more than one candidate stands and none is known to be this
/// frame's. One content hash can stand at two paths on one archive (a
/// shoot and its selects, both backed up); the copy of *this* frame is
/// the one under the folder its own folder was backed up to, by the
/// roots' pairings, and failing that the only candidate. Nothing is
/// asked of the disk.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Paired {
    pub hash: String,
    pub copies: Vec<Copy>,
    /// The archives with more than one candidate, and how many.
    pub ambiguous: Vec<(PathBuf, usize)>,
}

pub fn pair(lib: &Library, roots: &crate::Roots, frame: &Path) -> Option<Paired> {
    let archives = roots.archives();
    if archives.is_empty() {
        return None;
    }
    let row = lib.by_path(frame).ok().flatten()?;
    let rows = lib.by_hash_under(&row.hash, archives).ok()?;
    // The frame as the index spells it: the window's path may run
    // through a symlink (a folder opened by one), the index's is
    // canonical, and a twin is any other row of the hash.
    let this = row.path;
    let mut paired = Paired {
        hash: row.hash,
        ..Paired::default()
    };
    for archive in archives {
        let candidates: Vec<&Path> = rows
            .iter()
            .filter(|e| e.path != frame && e.path.starts_with(archive))
            .map(|e| e.path.as_path())
            .collect();
        if candidates.is_empty() {
            continue;
        }
        // The folder this frame's folder was backed up to under this
        // archive names its copy; the oldest pairing first, as the
        // roots keep them.
        let pairings: Vec<&crate::Pairing> = roots
            .backups()
            .iter()
            .filter(|p| p.dest.starts_with(archive))
            .collect();
        let expected: Vec<PathBuf> = pairings
            .iter()
            .filter_map(|p| {
                let rest = this.strip_prefix(&p.source).ok()?;
                Some(p.dest.join(rest))
            })
            .collect();
        // A sole hit is this frame's only when nothing says otherwise:
        // no pairing under this archive claims it for a folder this
        // frame is not in, and the hash stands at one path among the
        // local roots, this frame's. One raw in a shoot and its
        // selects, the shoot alone backed up, leaves the selects frame
        // with the shoot's copy as its sole hit; it is not its own.
        let sole = |c: &Path| {
            let claimed = pairings
                .iter()
                .any(|p| c.starts_with(&p.dest) && !this.starts_with(&p.source));
            let local_twins = lib
                .by_hash_under(&paired.hash, &roots.locals())
                .map(|rows| rows.iter().filter(|e| e.path != this).count())
                .unwrap_or(1);
            !claimed && local_twins == 0
        };
        let chosen = expected
            .iter()
            .find_map(|e| candidates.iter().find(|c| **c == e.as_path()).copied())
            .or_else(|| (candidates.len() == 1 && sole(candidates[0])).then(|| candidates[0]));
        match chosen {
            Some(copy) => paired.copies.push(Copy {
                archive: archive.to_path_buf(),
                copy: copy.to_path_buf(),
            }),
            None => paired
                .ambiguous
                .push((archive.to_path_buf(), candidates.len())),
        }
    }
    Some(paired)
}

/// A save made with no window open (a headless export's record,
/// §236): the frame's copies are not written now, but each is noted as
/// waiting in `index`, so the next window's catch-up writes them. The
/// index is opened for writing here, on the caller's thread; nothing
/// is created when there is no index. How many rows were noted.
pub fn note_disk_save(index: &Path, roots: &crate::Roots, frame: &Path) -> usize {
    note_waiting(index, roots, frame, "saved with no window open")
}

/// [`note_disk_save`], the rows given `reason`.
pub fn note_waiting(
    index: &Path,
    roots: &crate::Roots,
    frame: &Path,
    reason: &'static str,
) -> usize {
    if !index.is_file() || roots.archive_of(frame).is_some() {
        return 0;
    }
    // Opened only at this build's schema, never made, brought up or
    // rebuilt from here: the window does that, with the user watching.
    // A lock held by a window is not waited on for long: the rows are
    // skipped, and said; the frame's next save in a window queues them.
    let mut lib = match Library::open_current(index, Duration::from_millis(250)) {
        Ok(lib) => lib,
        Err(e) => {
            log::warn!(
                "sync: {}: the index not written to: {e}; the archive copy waits for a window's save",
                file_name(frame)
            );
            return 0;
        }
    };
    let Some(paired) = pair(&lib, roots, frame) else {
        return 0;
    };
    let mut noted = 0;
    let rows = paired
        .copies
        .iter()
        .map(|c| (c.archive.clone(), c.copy.clone(), reason))
        .chain(paired.ambiguous.iter().map(|(a, _)| {
            (
                a.clone(),
                PathBuf::new(),
                "more than one copy of this frame on the archive: which is its own is not known",
            )
        }));
    for (archive, copy, reason) in rows {
        match lib.add_pending(&paired.hash, &archive, &copy, frame, reason) {
            Ok(()) => noted += 1,
            Err(e) => log::warn!("sync: {}: pending not noted: {e}", file_name(frame)),
        }
    }
    noted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{R5, write_frame};

    fn scratch(what: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "greycard-sync-ui-{what}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dunce::canonicalize(&dir).unwrap()
    }

    /// Which copy on an archive is this frame's: the one under the
    /// folder its own folder was backed up to; with no pairing, the
    /// only one; with two and no pairing, not known.
    #[test]
    fn a_frame_is_paired_with_exactly_one_copy_or_none() {
        let dir = scratch("pair");
        let (local, nas) = (dir.join("local"), dir.join("nas"));
        let shoot = local.join("shoot");
        let selects = local.join("selects");
        std::fs::create_dir_all(&shoot).unwrap();
        std::fs::create_dir_all(&selects).unwrap();
        let a = shoot.join("a.tif");
        write_frame(&a, &R5, 1);
        let a2 = selects.join("a.tif");
        std::fs::copy(&a, &a2).unwrap();
        for sub in ["shoot", "selects"] {
            std::fs::create_dir_all(nas.join(sub)).unwrap();
            std::fs::copy(&a, nas.join(sub).join("a.tif")).unwrap();
        }
        let db = dir.join("library.sqlite");
        {
            let mut lib = Library::open(&db).unwrap();
            lib.index_tree(&local, &mut |_| {}).unwrap();
            lib.index_tree(&nas, &mut |_| {}).unwrap();
        }
        let lib = Library::open_read_only(&db).unwrap();
        let mut roots = crate::Roots::default();
        roots.add(&local).unwrap();
        roots.add(&nas).unwrap();
        roots.set_archive(&nas, true);
        // No pairing: two candidates, none known to be this frame's.
        let p = pair(&lib, &roots, &a).unwrap();
        assert!(p.copies.is_empty());
        assert_eq!(p.ambiguous, vec![(nas.clone(), 2)]);
        // The pairings say: each frame to its own.
        roots.set_backup_folder(&shoot, &nas, &nas.join("shoot"));
        roots.set_backup_folder(&selects, &nas, &nas.join("selects"));
        let p = pair(&lib, &roots, &a).unwrap();
        assert_eq!(
            p.copies,
            vec![Copy {
                archive: nas.clone(),
                copy: nas.join("shoot").join("a.tif")
            }]
        );
        let p2 = pair(&lib, &roots, &a2).unwrap();
        assert_eq!(p2.copies[0].copy, nas.join("selects").join("a.tif"));
        assert!(p2.ambiguous.is_empty());
        // One candidate needs no pairing.
        std::fs::remove_file(nas.join("selects").join("a.tif")).unwrap();
        // Each reader closed before the next opens: a shadowed one
        // stays open, and Windows will not remove the folder under it.
        drop(lib);
        let mut w = Library::open(&db).unwrap();
        w.forget(&[nas.join("selects").join("a.tif")]).unwrap();
        drop(w);
        let lib = Library::open_read_only(&db).unwrap();
        let bare = {
            let mut r = crate::Roots::default();
            r.add(&local).unwrap();
            r.add(&nas).unwrap();
            r.set_archive(&nas, true);
            r
        };
        // Not while the shoot's frame, the same hash, is here too: the
        // sole hit may be its copy.
        let p = pair(&lib, &bare, &a2).unwrap();
        assert!(p.copies.is_empty(), "{p:?}");
        assert_eq!(p.ambiguous, vec![(nas.clone(), 1)]);
        // Nor when a pairing claims the hit for the shoot's folder.
        let p = pair(&lib, &roots, &a2).unwrap();
        assert!(p.copies.is_empty(), "{p:?}");
        // The shoot's frame gone, the selects frame is the hash's one
        // local frame, and the sole hit is its own.
        drop(lib);
        let mut w = Library::open(&db).unwrap();
        w.forget(std::slice::from_ref(&a)).unwrap();
        drop(w);
        let lib = Library::open_read_only(&db).unwrap();
        let p = pair(&lib, &bare, &a2).unwrap();
        assert_eq!(p.copies.len(), 1);
        drop(lib);
        greycard_core::testing::remove_dir_retry(&dir);
    }
}
