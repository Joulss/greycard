//! What a run with no window reads of the editor's settings file: the
//! export sheet as it was left, the export presets and the one last
//! chosen, where sidecars go and whether an `.xmp` goes with them,
//! whether a camera's JPEG is a frame of its own, and the last file
//! open. The window's own settings carry this as their export half,
//! so the names in the file are defined once; a headless export and
//! the command line read it alone, and nothing of the window's panes
//! or themes.

use std::path::{Path, PathBuf};

use greycard_render::export::{self, OnExists};
use serde::{Deserialize, Serialize};

use crate::sheet::{ExportPreset, Sheet};

/// The settings the window, a headless export, and the command line
/// share, by the names the file has always had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shared {
    /// The export sheet as it was left, under the `export_` names it
    /// has always had in the file.
    #[serde(flatten)]
    pub export: Sheet,
    /// The export presets, in the order they were first saved.
    pub export_presets: Vec<ExportPreset>,
    /// The preset last chosen or saved, empty for none; the sheet
    /// above may have been edited since.
    pub export_preset: String,
    /// Whether the meta (the rating, the flag, the label, the
    /// keywords and the words) is also written to an `.xmp` beside
    /// the frame, for Lightroom, Bridge and darktable to read. Off
    /// by default: the `.gcd` is the truth and one file beside a
    /// frame is the rule (§117), so a second one is for a folder
    /// shared with another tool and is asked for. Reading an XMP
    /// that is there is not a choice and happens either way.
    pub xmp_sidecars: bool,
    /// Whether a frame's `.gcd` is written under a hidden `.greycard`
    /// folder in the frame's folder rather than beside the frame.
    /// Off by default. Reading looks in both places either way, so
    /// flipping this loses nothing; only where the next write goes
    /// changes. The XMPs stay beside the frame whatever this says,
    /// since beside is where Lightroom and darktable look.
    pub sidecars_in_folder: bool,
    /// Whether a camera's JPEG beside a raw of its name is listed as a
    /// frame of its own, as Lightroom's "treat JPEG files next to raw
    /// files as separate photos" lists it. Off by default: the JPEG is
    /// the raw's companion, marked on its tile, and goes where the raw
    /// goes (`greycard_library::pairs`). The command line's listing and
    /// `--export` read it too. Written as the switch is flipped.
    pub jpegs_apart: bool,
    /// Whether a raw's new edit starts with the lens section on. On by
    /// default; off, a raw with no sidecar starts with the section's
    /// switch off, and a sidecar already written keeps what it holds.
    /// The command line's `--export` reads it too, so a frame with no
    /// sidecar exports as the window shows it. Written as the switch
    /// is flipped.
    #[serde(default = "on", deserialize_with = "bool_on_leniently")]
    pub lens_on_new: bool,
    /// The absolute path of the last file open, developed and all, so
    /// the next run with no path can jump back to it. Written as soon
    /// as it develops, not waited for the window to close; empty
    /// until then.
    pub last_file: String,
    /// The author every export names, over the camera's (EXIF
    /// `Artist`, XMP `dc:creator`); several split by `;`. Empty for
    /// none, and then left out of the file, so a file from before it
    /// writes back as it read. Set by hand in the file for now.
    #[serde(
        skip_serializing_if = "String::is_empty",
        deserialize_with = "text_leniently"
    )]
    pub author: String,
    /// The copyright notice every export carries, over the camera's
    /// (EXIF `Copyright`, XMP `dc:rights`). As `author`.
    #[serde(
        skip_serializing_if = "String::is_empty",
        deserialize_with = "text_leniently"
    )]
    pub copyright: String,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            export: Sheet::default(),
            export_presets: Vec::new(),
            export_preset: String::new(),
            xmp_sidecars: false,
            sidecars_in_folder: false,
            jpegs_apart: false,
            lens_on_new: true,
            last_file: String::new(),
            author: String::new(),
            copyright: String::new(),
        }
    }
}

/// Text of a file edited by hand: anything but a string reads as
/// none, and costs the rest nothing.
fn text_leniently<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Null => Ok(String::new()),
        other => {
            // The kind alone, and once a run: a value pasted in by
            // mistake may be long, and the file is read more than once.
            static SAID: std::sync::Once = std::sync::Once::new();
            let kind = match other {
                serde_json::Value::Bool(_) => "a switch",
                serde_json::Value::Number(_) => "a number",
                serde_json::Value::Array(_) => "a list",
                _ => "an object",
            };
            SAID.call_once(|| {
                tracing::warn!("settings: the author or copyright is {kind}, not text; none")
            });
            Ok(String::new())
        }
    }
}

fn on() -> bool {
    true
}

/// A switch of a file edited by hand that is on by default: anything
/// but true or false reads as on, and costs the rest nothing.
fn bool_on_leniently<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(b) => Ok(b),
        other => {
            tracing::warn!("settings: {other} is not a switch; on");
            Ok(true)
        }
    }
}

impl Shared {
    /// What a raw with no sidecar starts with, by these settings.
    pub fn fresh(&self) -> greycard_edit_io::on_disk::Fresh {
        greycard_edit_io::on_disk::Fresh {
            lens: self.lens_on_new,
        }
    }

    /// The author and copyright every export carries, as the file
    /// has them.
    pub fn rights(&self) -> export::Rights {
        export::Rights::new(&self.author, &self.copyright)
    }

    /// The export sheet to open with, and the preset it is: the last
    /// run's, or the one `preset` names, with `long_edge` and
    /// `on_exists` over either. A preset the settings do not have is an
    /// error, not a quiet export at whatever the sheet was left at.
    pub fn opening_sheet(
        &self,
        preset: Option<&str>,
        long_edge: Option<u32>,
        on_exists: Option<OnExists>,
    ) -> anyhow::Result<(Sheet, Option<String>)> {
        let (mut s, preset) = match preset {
            Some(name) => {
                let p = crate::sheet::lookup(&self.export_presets, name)?;
                (p.sheet.clone(), Some(p.name.clone()))
            }
            None => (
                self.export.clone(),
                (!self.export_preset.is_empty()).then(|| self.export_preset.clone()),
            ),
        };
        if let Some(edge) = long_edge {
            // The flag's size over the sheet's, for this run's export.
            s.size = export::CUSTOM.into();
            s.custom = edge.to_string();
        }
        if let Some(policy) = on_exists {
            // The flag's answer over the sheet's, for this run's export.
            s.on_exists = policy.name().into();
        }
        Ok((s, preset))
    }

    /// What the user's settings file says, or the defaults.
    pub fn load() -> Self {
        path().map(|p| Self::load_from(&p)).unwrap_or_default()
    }

    /// What the file at `path` says, or the defaults: a file that is
    /// not there says nothing, and one that will not read is warned of.
    /// Every other key in it is the window's and passed over, so a
    /// window setting the file has wrong costs this nothing.
    pub fn load_from(path: &Path) -> Self {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!("settings {}: {e}; defaults", path.display());
                return Self::default();
            }
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!("settings {}: {e}; defaults", path.display());
            Self::default()
        })
    }
}

/// `$XDG_CONFIG_HOME/greycard/settings.json` when that is set, else
/// `greycard/settings.json` under the platform's configuration
/// directory: `~/.config` on Linux, `~/Library/Application Support` on
/// macOS, `%APPDATA%` on Windows.
pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(dirs::config_dir)?;
    Some(base.join("greycard").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{remove_dir_retry, scratch_dir};

    /// The switches are read from the editor's file: on only where it
    /// says so, off for a file without the key, one that will not
    /// read, or none; the window's own keys beside them cost nothing,
    /// even one it would refuse.
    #[test]
    fn the_editors_switches_are_read_from_its_settings() {
        let dir = scratch_dir("settings-shared");
        let file = dir.join("settings.json");
        assert_eq!(Shared::load_from(&file), Shared::default());
        std::fs::write(
            &file,
            r#"{"xmp_sidecars": true, "jpegs_apart": true, "theme": 7, "export_size": "2048"}"#,
        )
        .unwrap();
        let read = Shared::load_from(&file);
        assert!(read.jpegs_apart && read.xmp_sidecars);
        assert!(!read.sidecars_in_folder);
        assert_eq!(read.export.size, "2048");
        std::fs::write(&file, r#"{"jpegs_apart": false}"#).unwrap();
        assert!(!Shared::load_from(&file).jpegs_apart);
        std::fs::write(&file, b"not json").unwrap();
        assert_eq!(Shared::load_from(&file), Shared::default());
        remove_dir_retry(&dir);
    }

    /// The switch for a new edit's lens section: on for a file without
    /// it or with a value of the wrong kind, the rest of the file
    /// standing; off only where the file says so, and written back as
    /// it reads.
    #[test]
    fn the_lens_switch_for_new_edits_is_read_leniently() {
        assert!(Shared::default().lens_on_new);
        let read: Shared = serde_json::from_str("{}").unwrap();
        assert!(read.lens_on_new && read.fresh().lens);
        let read: Shared = serde_json::from_str(r#"{"lens_on_new": false}"#).unwrap();
        assert!(!read.lens_on_new && !read.fresh().lens);
        let again: Shared = serde_json::from_str(&serde_json::to_string(&read).unwrap()).unwrap();
        assert_eq!(again, read, "round trip");
        for text in [
            r#"{"jpegs_apart": true, "lens_on_new": "no"}"#,
            r#"{"jpegs_apart": true, "lens_on_new": null}"#,
            r#"{"jpegs_apart": true, "lens_on_new": 0}"#,
        ] {
            let read: Shared = serde_json::from_str(text).expect(text);
            assert!(read.jpegs_apart, "{text}");
            assert!(read.lens_on_new, "{text}");
        }
    }

    /// The author and copyright: absent from a file that never had
    /// them, which writes back byte for byte; read where set, a value
    /// of the wrong kind costing the rest nothing; and made the export's
    /// rights, blank as none.
    #[test]
    fn the_author_and_copyright_are_read_and_left_out_when_unset() {
        let before = serde_json::to_string_pretty(&Shared::default()).unwrap();
        assert!(!before.contains("author") && !before.contains("copyright"));
        let read: Shared = serde_json::from_str(&before).unwrap();
        assert_eq!(serde_json::to_string_pretty(&read).unwrap(), before);
        assert!(read.rights().is_empty());

        let read: Shared = serde_json::from_str(
            r#"{"author": " Zoë Ångström ", "copyright": "© 2026 Zoë", "jpegs_apart": true}"#,
        )
        .unwrap();
        assert_eq!(read.rights().author.as_deref(), Some("Zoë Ångström"));
        assert_eq!(read.rights().copyright.as_deref(), Some("© 2026 Zoë"));
        let again: Shared = serde_json::from_str(&serde_json::to_string(&read).unwrap()).unwrap();
        assert_eq!(again, read, "round trip");

        for text in [
            r#"{"jpegs_apart": true, "author": 5, "copyright": null}"#,
            r#"{"jpegs_apart": true, "author": ["a"], "copyright": "  "}"#,
        ] {
            let read: Shared = serde_json::from_str(text).expect(text);
            assert!(read.jpegs_apart, "{text}");
            assert!(read.rights().is_empty(), "{text}");
        }
    }

    #[test]
    fn the_path_is_under_the_configuration_directory() {
        let path = path().expect("a configuration directory in the test environment");
        assert!(path.ends_with("greycard/settings.json"), "{path:?}");
        assert!(path.is_absolute());
    }
}
