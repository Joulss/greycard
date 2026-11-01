//! Helpers the test modules share.

use std::path::PathBuf;
use std::sync::Arc;

use greycard_core::color::Profile;
use greycard_core::lut;

use crate::{DisplayCurve, Edit, Loader};

/// A loader that fails the test when asked for anything: for what
/// names no file.
pub(crate) struct Unasked;

impl Loader for Unasked {
    fn profile(&self, name: &str) -> Option<Arc<Profile>> {
        panic!("asked for the profile {name:?}")
    }

    fn look(&self, name: &str, _: f32, _: DisplayCurve) -> Option<lut::Look> {
        panic!("asked for the look {name:?}")
    }
}

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
