//! What a develop reads by name from outside the sidecar: the camera
//! profile an edit names and its look's tables. This crate holds the
//! edit as data and turns it into the engine's settings; finding and
//! reading those files is the caller's, handed in as a [`Loader`].
//! greycard-edit-io's stores read the user's directories.

use std::sync::Arc;

use greycard_core::color::Profile;
use greycard_core::lut;

use crate::DisplayCurve;

/// The files an edit names, read and kept by whoever hands this in.
pub trait Loader {
    /// The camera profile of this name, or `None` for one that is not
    /// there or will not read, which is the file's own calibrations.
    fn profile(&self, name: &str) -> Option<Arc<Profile>>;

    /// The look of this name at `strength` as a picture under `curve`
    /// finishes through it ([`crate::LookLut::look_under`]), or `None`
    /// for no look.
    fn look(&self, name: &str, strength: f32, curve: DisplayCurve) -> Option<lut::Look>;
}

/// No profile and no look directory: every profile is the file's own
/// and every look is none.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoStores;

impl Loader for NoStores {
    fn profile(&self, _: &str) -> Option<Arc<Profile>> {
        None
    }

    fn look(&self, _: &str, _: f32, _: DisplayCurve) -> Option<lut::Look> {
        None
    }
}
