//! A frame by what names it across lists and threads: a file's path
//! and which version of it. Apart from the versions module, which lists
//! the window's cells, since the worker, the thumbnails' pool, and the
//! edit's pictures key by it too.

use std::path::PathBuf;
use std::sync::Arc;

/// A frame by what names it across lists and threads: its file's path
/// and its version's id, `None` for the file's own. What the thumbnails'
/// pool, the edit's pictures and a request for sidecars are keyed by.
/// The version's name rides along for the log and the status line,
/// and is no part of the key: two keys are the same frame by path and
/// id whatever name each was made with.
#[derive(Debug, Clone)]
pub(crate) struct Key {
    pub(crate) path: PathBuf,
    pub(crate) version: Option<Arc<str>>,
    pub(crate) name: Option<Arc<str>>,
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.version == other.version
    }
}

impl Eq for Key {}

impl std::hash::Hash for Key {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.version.hash(state);
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.path, &self.version).cmp(&(&other.path, &other.version))
    }
}

impl Key {
    /// The file's own frame.
    pub(crate) fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            version: None,
            name: None,
        }
    }

    /// Version `version` of the file at `path`, the file's own for none.
    pub(crate) fn of(path: impl Into<PathBuf>, version: Option<&str>) -> Self {
        Self {
            path: path.into(),
            version: version.map(Arc::from),
            name: None,
        }
    }

    /// The version's id, when the frame is one.
    pub(crate) fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The file's name for the file's own frame, and for a version the
    /// file's stem and the version's name, "IMG_0001 (Warm)", the form
    /// the export names it by (`queue::label_of`); the id stands in for a
    /// name that was not had. What the log and the status line say of
    /// the frame.
    pub(crate) fn said(&self) -> String {
        match &self.version {
            None => greycard_edit_io::on_disk::file_name(&self.path),
            Some(v) => {
                let stem = self
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!(
                    "{stem} ({})",
                    self.name.as_deref().filter(|n| !n.is_empty()).unwrap_or(v)
                )
            }
        }
    }
}
