//! The camera profile an edit develops with: the file's own, or a DCP
//! from the user's profile directory.
//!
//! The choice lives in the edit because it changes the picture and a
//! preset should be able to carry it (notes §78). The engine takes the
//! resolved profile, not a name: reading the file is the
//! [`crate::Loader`]'s the caller hands in (greycard-edit-io's stores,
//! which list the directory too), done once per file and kept, so every
//! develop of the same edit — the viewport's and the export's — is
//! handed the same profile.
//!
//! A name that resolves to nothing is a warning in the log and the
//! file's own calibrations. A missing profile never fails a develop.

use std::path::PathBuf;
use std::sync::Arc;

use greycard_core::color::Profile;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Loader;

/// The file extension of a camera profile.
pub const EXTENSION: &str = "dcp";

/// The word for the file's own calibrations, in the sidecar and on the
/// panel.
pub const EMBEDDED: &str = "embedded";

/// Which camera profile develops the picture.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProfileChoice {
    /// The matrices the raw file carries.
    #[default]
    Embedded,
    /// A DCP in the profile directory, by file name without the
    /// extension.
    Named(String),
}

impl ProfileChoice {
    /// The name as the sidecar writes it.
    pub fn name(&self) -> &str {
        match self {
            ProfileChoice::Embedded => EMBEDDED,
            ProfileChoice::Named(name) => name,
        }
    }

    /// A name from the panel or a sidecar. Anything empty, or the word
    /// for the file's own, is [`ProfileChoice::Embedded`].
    pub fn from_name(name: &str) -> Self {
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case(EMBEDDED) {
            ProfileChoice::Embedded
        } else {
            ProfileChoice::Named(name.to_string())
        }
    }

    pub fn is_embedded(&self) -> bool {
        matches!(self, ProfileChoice::Embedded)
    }
}

impl Serialize for ProfileChoice {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for ProfileChoice {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(ProfileChoice::from_name(&String::deserialize(d)?))
    }
}

/// The CAMERA section of an edit: what the sensor's color is taken
/// through, before everything else.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera {
    pub profile: ProfileChoice,
}

impl Camera {
    /// The engine's profile for this choice: the DCP `loader` reads
    /// and keeps, or `None` for the file's own. A profile that will not
    /// read warns once and comes back `None`, which is the file's own
    /// again.
    pub fn profile(&self, loader: &dyn Loader) -> Option<Arc<Profile>> {
        let ProfileChoice::Named(name) = &self.profile else {
            return None;
        };
        loader.profile(name)
    }
}

/// A profile in the directory, as the panel lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The file name without its extension: what the edit stores.
    pub name: String,
    pub path: PathBuf,
    /// The profile's own name, when it carries one.
    pub title: Option<String>,
    /// The camera it was made for, when it says.
    pub unique_camera_model: Option<String>,
    pub copyright: Option<String>,
    /// The entry this file is the same profile as, byte for byte under
    /// another file name: that one stands for both on the panel. The
    /// file is still an entry, so an edit naming it still finds it.
    pub same_as: Option<String>,
}

impl Entry {
    /// What the panel shows: the profile's own name when it has one
    /// and it is not simply the file's name again. Two that differ but
    /// share it are told apart by the panel, which knows which rows
    /// stand side by side.
    pub fn label(&self) -> &str {
        match &self.title {
            Some(title) if !title.is_empty() && title != &self.name => title,
            _ => &self.name,
        }
    }

    /// Whether this profile was made for `body`: its folded
    /// `UniqueCameraModel` is one of the folded [`Body::names`].
    ///
    /// DCPs name their camera with DNG's `UniqueCameraModel`, which is
    /// the maker's name and the model together ("Canon EOS 6D Mark
    /// II"), while a decoded frame carries them apart and cleaned up,
    /// and some decoders leave the make on the front of the model.
    /// Both sides are folded to their letters and digits alone, and
    /// the profile fits when the folded unique model is any of the
    /// body's names, its own or one it is sold under elsewhere.
    /// Equality, not containment: "CANON EOS R6" would otherwise fit
    /// an EOS R6 Mark II. A profile that names no camera fits
    /// everything, since it cannot say otherwise; one whose maker
    /// writes the model its own way does not fit, and is shown with
    /// the warning rather than hidden.
    pub fn fits(&self, body: Body) -> bool {
        let unique = self.folded_unique();
        unique.is_empty() || body.names().iter().any(|n| folded(n) == unique)
    }

    /// The camera the profile names, folded to its letters and digits
    /// alone: empty for one that names none.
    pub fn folded_unique(&self) -> String {
        self.unique_camera_model
            .as_deref()
            .map(folded)
            .unwrap_or_default()
    }
}

/// A camera as a profile's fit reads it: its make and model, and the
/// other names the same body is sold under (an EOS 850D is also an
/// EOS Rebel T8i).
#[derive(Debug, Clone, Copy, Default)]
pub struct Body<'a> {
    pub make: &'a str,
    pub model: &'a str,
    pub other_names: &'a [String],
}

impl<'a> Body<'a> {
    pub fn new(make: &'a str, model: &'a str, other_names: &'a [String]) -> Body<'a> {
        Body {
            make,
            model,
            other_names,
        }
    }

    /// The names a profile is matched against exactly, in order: the
    /// make and model, the model, then the make and each other name,
    /// and each other name.
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for m in std::iter::once(self.model).chain(self.other_names.iter().map(String::as_str)) {
            out.push(format!("{} {m}", self.make));
            out.push(m.to_string());
        }
        out
    }
}

fn folded(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The entries the panel lists, one a profile: an entry that is the
/// same profile as another is left out, unless it is the one `chosen`
/// names, which then stands for the pair in the other's place so the
/// list shows what the edit names.
pub fn listed<'a>(entries: &'a [Entry], chosen: &str) -> impl Iterator<Item = &'a Entry> {
    let instead = entries
        .iter()
        .find(|e| e.name == chosen)
        .and_then(|e| e.same_as.as_deref());
    entries.iter().filter(move |e| match &e.same_as {
        Some(_) => e.name == chosen,
        None => Some(e.name.as_str()) != instead,
    })
}

/// What is wrong with the chosen profile, if anything: that it is not
/// there, or that it was made for another camera. The develop falls
/// back to the file's own calibrations either way.
pub fn warning(profiles: &[Entry], camera: Body, chosen: &str) -> String {
    let name = chosen;
    if chosen == EMBEDDED {
        return String::new();
    }
    let Some(entry) = profiles.iter().find(|e| e.name == name) else {
        return format!("{name} is not in the profile directory; the file's own colors are used.");
    };
    let (make, model) = (camera.make, camera.model);
    if make.is_empty() || entry.fits(camera) {
        return String::new();
    }
    let made_for = entry
        .unique_camera_model
        .as_deref()
        .unwrap_or("another camera");
    format!("Made for {made_for}, not {make} {model}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the panel says under the profile list: nothing for the
    /// file's own colors or a profile that fits, a line for one the
    /// directory has not got, and a line for one made elsewhere.
    #[test]
    fn a_profile_that_does_not_fit_says_so_and_a_missing_one_says_so() {
        let camera = Body::new("Canon", "EOS R5", &[]);
        let mine = Entry {
            name: "mine".into(),
            path: std::path::PathBuf::new(),
            title: None,
            copyright: None,
            unique_camera_model: Some("Canon EOS R5".into()),
            same_as: None,
        };
        let theirs = Entry {
            name: "theirs".into(),
            unique_camera_model: Some("NIKON Z 8".into()),
            ..mine.clone()
        };
        let profiles = vec![mine.clone(), theirs.clone()];
        // The file's own colors: nothing to say.
        assert!(warning(&profiles, camera, EMBEDDED).is_empty());
        // A profile for this camera: nothing to say.
        assert!(warning(&profiles, camera, "mine").is_empty());
        // One made for another: the panel still shows it chosen, and
        // says what it was made for.
        let said = warning(&profiles, camera, "theirs");
        assert!(
            said.contains("NIKON Z 8") && said.contains("EOS R5"),
            "{said}"
        );
        // One the directory has not got at all: a sidecar or a preset
        // from another machine.
        let said = warning(&profiles, camera, "elsewhere");
        assert!(said.contains("not in the profile directory"), "{said}");
        // No camera named yet: nothing can be said about the fit.
        assert!(warning(&profiles, Body::default(), "theirs").is_empty());
    }

    #[test]
    fn a_choice_reads_and_writes_as_a_name() {
        assert_eq!(ProfileChoice::default(), ProfileChoice::Embedded);
        assert_eq!(ProfileChoice::Embedded.name(), "embedded");
        assert_eq!(ProfileChoice::from_name(""), ProfileChoice::Embedded);
        assert_eq!(
            ProfileChoice::from_name("Embedded"),
            ProfileChoice::Embedded
        );
        assert_eq!(
            ProfileChoice::from_name(" Canon EOS R6 "),
            ProfileChoice::Named("Canon EOS R6".into())
        );
        let json = serde_json::to_string(&ProfileChoice::Named("Adobe Standard".into())).unwrap();
        assert_eq!(json, "\"Adobe Standard\"");
        let back: ProfileChoice = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ProfileChoice::Named("Adobe Standard".into()));
        let camera: Camera = serde_json::from_str("{}").unwrap();
        assert_eq!(camera, Camera::default());
    }

    fn entry(unique: Option<&str>) -> Entry {
        Entry {
            name: "Some Profile".into(),
            path: PathBuf::new(),
            title: None,
            unique_camera_model: unique.map(str::to_string),
            copyright: None,
            same_as: None,
        }
    }

    #[test]
    fn a_profile_fits_the_camera_it_names() {
        let fits = |unique: Option<&str>, make: &str, model: &str| {
            entry(unique).fits(Body::new(make, model, &[]))
        };
        assert!(fits(Some("CANON EOS R6"), "Canon", "EOS R6"));
        assert!(fits(
            Some("Canon EOS 6D Mark II"),
            "Canon",
            "EOS 6D Mark II"
        ));
        // The frame's own make and model concatenated are the same
        // name whether or not the decoder repeats the make.
        assert!(fits(Some("NIKON Z 6"), "Nikon", "NIKON Z 6"));
        assert!(!fits(Some("Canon EOS R6"), "Canon", "EOS R6 Mark II"));
        assert!(!fits(Some("Canon EOS R6"), "Nikon", "Z 6"));
        // A profile that names no camera cannot be said to be wrong.
        assert!(fits(None, "Canon", "EOS R6"));
    }

    /// A body sold under other names fits a profile made under any of
    /// them: Adobe names the EOS 850D's profiles for the Rebel T8i.
    #[test]
    fn a_profile_fits_the_body_by_any_of_its_names() {
        let aliases = ["EOS Rebel T8i".to_string(), "EOS Kiss X10i".to_string()];
        let body = Body::new("Canon", "EOS 850D", &aliases);
        for unique in [
            "Canon EOS 850D",
            "Canon EOS Rebel T8i",
            "EOS Rebel T8i",
            "Canon EOS Kiss X10i",
        ] {
            let e = entry(Some(unique));
            assert!(e.fits(body), "{unique}");
        }
        assert!(!entry(Some("Canon EOS Rebel T7i")).fits(body));
        // Without the other names, the Rebel's profile is another body's.
        let bare = Body::new("Canon", "EOS 850D", &[]);
        assert!(!entry(Some("Canon EOS Rebel T8i")).fits(bare));
        assert_eq!(
            body.names(),
            [
                "Canon EOS 850D",
                "EOS 850D",
                "Canon EOS Rebel T8i",
                "EOS Rebel T8i",
                "Canon EOS Kiss X10i",
                "EOS Kiss X10i",
            ]
        );
    }

    /// A short model never fits a longer one's profile.
    #[test]
    fn a_profile_for_a_longer_model_does_not_fit() {
        let r5 = entry(Some("Canon EOS R5"));
        assert!(!r5.fits(Body::new("Canon", "EOS R", &[])));
        assert!(!r5.fits(Body::new("Canon", "Canon EOS R", &[])));
        assert!(!entry(Some("Google Pixel 9 Pro")).fits(Body::new("Google", "Pixel 9", &[])));
    }

    #[test]
    fn a_label_prefers_the_profiles_own_name() {
        let mut entry = Entry {
            name: "canon-r6-neutral".into(),
            path: PathBuf::new(),
            title: Some("Canon EOS R6 Neutral".into()),
            unique_camera_model: None,
            copyright: None,
            same_as: None,
        };
        assert_eq!(entry.label(), "Canon EOS R6 Neutral");
        entry.title = None;
        assert_eq!(entry.label(), "canon-r6-neutral");
    }

    #[test]
    fn an_embedded_choice_asks_the_engine_for_nothing() {
        use crate::testing::Unasked;
        assert!(Camera::default().profile(&Unasked).is_none());
        assert!(crate::Edit::default().settings(&Unasked).profile.is_none());
    }
}
