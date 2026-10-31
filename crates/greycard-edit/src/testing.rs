//! Helpers the test modules share.

use std::path::PathBuf;

use crate::Edit;

/// A fresh folder a test can write into, its own by name and pid.
pub(crate) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "greycard-edit-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The default edit at an exposure of `stops`.
pub(crate) fn exposed(stops: f32) -> Edit {
    let mut e = Edit::default();
    e.light.exposure = stops;
    e
}
