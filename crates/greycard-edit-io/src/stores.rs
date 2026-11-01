//! The user's profile and look directories as a develop reads them:
//! the [`Loader`] the window, the engine, and the command line hand to
//! `Edit::settings` and `LookLut::look_under`.

use std::path::PathBuf;
use std::sync::Arc;

use greycard_core::color::Profile;
use greycard_core::lut;
use greycard_edit::{DisplayCurve, Loader};

use crate::{looks, profiles};

/// The profile and look directories, `None` for one there is none of.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stores {
    pub profiles: Option<PathBuf>,
    pub looks: Option<PathBuf>,
}

impl Stores {
    /// The user's directories as they stand: see
    /// [`profiles::store_dir`] and [`looks::store_dir`].
    pub fn user() -> Self {
        Stores {
            profiles: profiles::store_dir(),
            looks: looks::store_dir(),
        }
    }
}

impl Loader for Stores {
    fn profile(&self, name: &str) -> Option<Arc<Profile>> {
        profiles::load(&profiles::path_in(self.profiles.as_deref(), name)?)
    }

    fn look(&self, name: &str, strength: f32, curve: DisplayCurve) -> Option<lut::Look> {
        looks::under_in(self.looks.as_deref()?, name, strength, curve)
    }
}
