//! A preset as it reaches one frame, with no window: whether its camera
//! profile fits the body that took the frame, and the preset laid as
//! one named step over a sidecar. The panel's sync and the start ask
//! these on the window's thread, the import on its own.

use std::path::Path;

use greycard_edit::camera::{self, ProfileChoice};
use greycard_edit::preset::Section;
use greycard_edit::{Preset, Sidecar};

/// Whether a camera profile may go to a frame of `body` (make and
/// model). A profile the directory has not got, or one that names no
/// camera, cannot be checked and goes, as it would in a preset; one
/// made for a camera goes only to that camera's frames, and a frame
/// that cannot say what took it does not get it.
pub fn profile_fits(entry: Option<&camera::Entry>, body: Option<camera::Body>) -> bool {
    let Some(entry) = entry else {
        return true;
    };
    if entry
        .unique_camera_model
        .as_deref()
        .is_none_or(|u| u.trim().is_empty())
    {
        return true;
    }
    body.is_some_and(|body| entry.fits(body))
}

/// A probed file's camera, as a profile's fit reads it.
pub fn probed_body(p: &greycard_core::decode::Probe) -> camera::Body<'_> {
    camera::Body::new(&p.make, &p.model, &p.other_names)
}

/// `panel::sync::preset_for_body` with no state: `body` says what took
/// the frame (make and model), asked only when the preset names a
/// profile. The import runs this on its own thread, where there is no
/// `State`.
pub fn preset_fitted(
    preset: &Preset,
    profiles: &[camera::Entry],
    body: impl FnOnce() -> Option<greycard_core::decode::Probe>,
) -> (Preset, bool) {
    let named = match &preset.edit.camera.profile {
        ProfileChoice::Named(name) if preset.sections.contains(&Section::Camera) => {
            profiles.iter().find(|e| &e.name == name)
        }
        _ => None,
    };
    let Some(entry) = named else {
        return (preset.clone(), false);
    };
    let body = body();
    if profile_fits(Some(entry), body.as_ref().map(probed_body)) {
        (preset.clone(), false)
    } else {
        let mut without = preset.clone();
        without.sections.retain(|s| *s != Section::Camera);
        (without, true)
    }
}

/// `preset` as it reaches or leaves `file`: itself on a raw, and
/// without the display curve on a picture (a JPEG, a TIFF). A picture
/// shows no switch for the curve, and its own is the default, so a
/// curve laid on one could not be seen or set back (yet a fitted look
/// would still go by it), and one taken from one is no choice anybody
/// made.
pub fn curve_fitted(preset: &Preset, file: &Path) -> Preset {
    let mut out = preset.clone();
    if greycard_core::picture::is_picture_path(file) {
        out.sections.retain(|s| *s != Section::DisplayCurve);
    }
    out
}

/// `--preset`: lay `preset` over `file`'s edit as one step named for
/// it, and write the sidecar where `placement` says (`None`: the run
/// writes no sidecars). Written here and not left to the quit's
/// `save_edit`, which finds the panel equal to the step already
/// recorded and so writes nothing: the step would be lost on close.
///
/// The preset goes on the line of `version` (the file's own for none);
/// a line that is not there takes nothing. A raw's line still waiting on
/// its ISO's learned-denoiser blend (never developed,
/// `on_disk::line_never_developed`) is given it first, as
/// `panel::sync::lay_over_targets` gives a target: the step makes the
/// edit no longer the default, and once written the next launch would
/// take that to mean the blend was seeded already. A preset that
/// changes nothing records and writes nothing. True when the sidecar
/// was written.
pub fn preset_at_start(
    sidecar: &mut Sidecar,
    version: Option<&str>,
    file: &Path,
    preset: &Preset,
    placement: Option<greycard_edit::Placement>,
) -> bool {
    let preset = &curve_fitted(preset, file);
    let Some(line) = sidecar.line_mut(version) else {
        return false;
    };
    let applied = preset.applied(&line.current);
    if applied == line.current {
        return false;
    }
    let mut over = line.current.clone();
    if greycard_edit_io::on_disk::line_never_developed(line)
        && !greycard_core::picture::is_picture_path(file)
        && let Ok(p) = greycard_core::decode::probe_path(file)
    {
        over.noise.learned_strength = greycard_edit::Noise::blend_for_iso(p.iso);
        line.current.noise.learned_strength = over.noise.learned_strength;
    }
    let label = greycard_edit::history::preset_label(&preset.name);
    if !line.record_as(preset.applied(&over), Some(label)) {
        return false;
    }
    match placement.map(|p| sidecar.save_in(file, p)) {
        Some(Ok(())) => true,
        Some(Err(e)) => {
            tracing::warn!("{}: sidecar not saved: {e}", file.display());
            false
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use greycard_edit::preset::Choice;

    #[test]
    fn a_picture_neither_takes_nor_gives_a_display_curve() {
        let preset = Preset::from_choice(
            "p",
            &greycard_edit::Edit::default(),
            &Choice::whole(&[Section::Light, Section::DisplayCurve]),
        );
        for raw in ["/a/IMG_0001.CR3", "/a/PXL_0001.dng"] {
            assert_eq!(
                curve_fitted(&preset, Path::new(raw)).sections,
                preset.sections
            );
        }
        for picture in ["/a/IMG_0001.JPG", "/a/scan.tif"] {
            assert_eq!(
                curve_fitted(&preset, Path::new(picture)).sections,
                [Section::Light]
            );
        }
    }
}
