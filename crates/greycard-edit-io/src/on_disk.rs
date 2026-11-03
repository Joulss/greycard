//! A frame's sidecar as an open reads it from disk: the `.gcd`, or the
//! default when there is none or it will not read, what an `.xmp`
//! beside the frame says that the sidecar has not taken yet, a
//! Lightroom develop in it taken and written when the reader knows
//! where sidecars go and the frame has no edit of ours
//! ([`crate::lightroom::beside`]), and a sidecar from an older build
//! brought up to date from the frame's shape. Here, below the window,
//! since the reduced develop, the masks, and the headless export read
//! a sidecar as the window's open does.

use std::path::Path;

use greycard_edit::mask::Shape;
use greycard_edit::{Edit, Line, Placement, Sidecar};

use crate::xmp;

pub fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A turn in words, for the log: which way and how far.
fn turn_words(quarters: i32) -> &'static str {
    match quarters.rem_euclid(4) {
        1 => "a quarter turn to the right of its tag",
        2 => "a half turn from its tag",
        3 => "a quarter turn to the left of its tag",
        _ => "as its tag says",
    }
}

/// What went wrong reading a sidecar from disk, the default edit
/// standing in for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// No `.gcd` beside the frame.
    Missing,
    /// There is one, and it could not be read: the error.
    Unreadable(String),
}

/// One file's sidecar, read from disk: what the library's `held::from_disk`
/// reads for a frame the index has no row for, and for a frame whose
/// row stood in until now. Beside it, what went wrong when the `.gcd`
/// was not there or could not be read and the default stands in for it,
/// so a frame whose row says it has a develop is not given the default
/// in its place. Whether a line of it still waits for its ISO's blend
/// is the line's to say (`rows::wants_seed`).
///
/// `sidecars` is where the editor's setting puts a sidecar, none with
/// sidecars off, when nothing is read and the default stands in. A
/// Lightroom develop beside a raw with no edit of ours is taken and the
/// sidecar written there ([`crate::lightroom::beside`]).
pub fn load_sidecar_said(f: &Path, sidecars: Option<Placement>) -> (Sidecar, Option<Trouble>) {
    match sidecars {
        Some(placement) => load(f, Some(placement)),
        None => (Sidecar::default(), None),
    }
}

/// [`load_sidecar_said`] for a reader that does not know where
/// sidecars go (the reduced develop, the masks, a record of an export):
/// the sidecar as it is, nothing taken and nothing written.
pub fn read_sidecar_said(f: &Path) -> (Sidecar, Option<Trouble>) {
    load(f, None)
}

/// The sidecar read, and with `takes` a Lightroom develop beside it
/// taken and written there.
fn load(f: &Path, takes: Option<Placement>) -> (Sidecar, Option<Trouble>) {
    let mut trouble = None;
    // A picture that is not a raw starts from its own default.
    let fresh = || {
        if greycard_core::picture::is_picture_path(f) {
            Sidecar {
                line: Line::of(Edit::for_picture()),
                ..Sidecar::default()
            }
        } else {
            Sidecar::default()
        }
    };
    let mut sidecar = match Sidecar::load(f) {
        Ok(Some(s)) => {
            // A shape from a later build: loaded as nothing, the
            // rest of the edit kept. One of a kind this build has
            // not heard of is gone on the next save; a known kind
            // with a route or a group it has not (a Part, a
            // Landscape) is written back as it came.
            let unknown: usize = s.current.adjustments.iter().map(|a| a.mask.unknown()).sum();
            let dropped: usize = s
                .current
                .adjustments
                .iter()
                .flat_map(|a| &a.mask.components)
                .filter(|c| c.shape == Shape::Unknown)
                .count();
            if unknown > 0 {
                log::warn!(
                    "{}: sidecar: {unknown} mask shape(s) this build does not know, left out of \
                     their masks; {dropped} of a kind it has not heard of, dropped when the edit \
                     is saved, and {} kept as they came",
                    file_name(f),
                    unknown - dropped
                );
            }
            s
        }
        Ok(None) => {
            trouble = Some(Trouble::Missing);
            fresh()
        }
        Err(e) => {
            log::warn!(
                "{}: sidecar: {e}; starting from the default edit",
                file_name(f)
            );
            trouble = Some(Trouble::Unreadable(format!("{e}")));
            fresh()
        }
    };
    // What an XMP beside the frame has to say, when it has
    // changed since the sidecar last took its word. Reading
    // one is not behind the setting: writing an XMP is a
    // choice about somebody else's folder, believing one
    // that is already there is not. Nothing is written
    // here — the meta and the mark ride in memory until the
    // frame's next real save — so opening a folder of
    // somebody else's raws deposits nothing in it.
    // The camera's tag, for the one field of a packet that
    // is measured against it. The closure is called only for
    // a packet that carries `tiff:Orientation` at all, so a
    // folder whose XMPs are silent about it opens without
    // reading a byte of any raw.
    let took = xmp::adopt(f, &mut sidecar, || {
        greycard_core::decode::orientation_path(f)
            .inspect_err(|e| log::warn!("{}: orientation: {e}", file_name(f)))
            .ok()
    });
    // A turn taken from somebody else's file turns the
    // picture under the user without their having pressed
    // anything, and records no state to undo it with, so it
    // is said out loud and by name.
    if took.turned.is_some() {
        log::info!(
            "{}: the XMP beside it says the frame stands {}; turned to match",
            file_name(f),
            // Where it now stands, not how far it moved to
            // get there: a reader of the log wants the frame,
            // not the delta.
            turn_words(i32::from(sidecar.turn))
        );
        if took.left_placed {
            log::warn!(
                "{}: its masks and repair patches stay where they are on the \
                 screen, since nothing is decoded at a folder's opening to \
                 measure the frame by",
                file_name(f)
            );
        }
    }
    // A Lightroom develop in the XMP beside a raw with no edit of ours,
    // taken as a catalog's is and written, so every reader after this
    // one finds it. Never over a `.gcd` that will not read.
    if !matches!(trouble, Some(Trouble::Unreadable(_)))
        && let Some(placement) = takes
    {
        use crate::lightroom::beside;
        beside::take_and_save(f, &mut sidecar, placement);
        // Taken here, or by another reader while this one waited.
        if trouble == Some(Trouble::Missing)
            && !beside::no_edit_of_ours(&sidecar)
            && Sidecar::find(f).is_some()
        {
            trouble = None;
        }
    }
    (sidecar, trouble)
}

/// A raw's starting edit, its learned blend at its ISO, and the body
/// that shot it as the library names one, from its probe (none for a
/// file that would not probe): what an import lays a develop over.
pub fn starting_of(probe: Option<&greycard_core::decode::Probe>) -> (Edit, Option<String>) {
    let mut edit = Edit::default();
    edit.noise.learned_strength = greycard_edit::Noise::blend_for_iso(probe.and_then(|p| p.iso));
    let camera = probe.map(|p| greycard_core::raw::camera_name(&p.make, &p.model));
    (edit, camera)
}

/// Whether a raw's sidecar has nothing in it that a develop put
/// there: the edit is the default, and there is no history and no
/// snapshot behind it. A file rated but never developed has a
/// sidecar, and that is not the same as having been developed —
/// its learned blend is still waiting for its ISO.
pub fn never_developed(sidecar: &Sidecar) -> bool {
    line_never_developed(&sidecar.line)
}

/// [`never_developed`] of one line of a sidecar: a version made from the
/// original before the raw was opened is at the default too, and waits
/// for its ISO on its own; one made from an edited version, or edited
/// since, is not.
pub fn line_never_developed(line: &Line) -> bool {
    line.current == Edit::default() && line.history.is_empty() && line.snapshots.is_empty()
}

/// A sidecar from an older build brought up to date from the shape of
/// `frame` as its file says it stands, its turn applied: what the
/// window does with the frame it opens (`panel::browser::migrate_frame`),
/// for a sidecar read with no window holding the frame. Left as it is
/// when the file will not say its shape.
pub fn migrate_from_file(sidecar: &mut Sidecar, frame: &Path) {
    if !sidecar.needs_frame() {
        return;
    }
    let shape = greycard_core::decode::stance_path(frame)
        .ok()
        .and_then(|s| s.shown_size(sidecar.turn))
        .filter(|&(w, h)| w > 0 && h > 0);
    match shape {
        Some((w, h)) => sidecar.migrate_with_frame(w < h),
        None => log::warn!(
            "{}: nothing can say which way up it is, so its Original crop \
             waits before it is brought up to date",
            frame.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A star is not a develop: the sidecar a rating makes still has
    /// nothing of a develop in it, so the frame's learned blend is
    /// still waiting for its ISO.
    #[test]
    fn a_rated_sidecar_is_still_undeveloped() {
        let mut sidecar = Sidecar::default();
        assert!(never_developed(&sidecar));
        sidecar.meta.rating = 3;
        assert!(never_developed(&sidecar));
        let mut edit = Edit::default();
        edit.light.exposure = 0.5;
        assert!(sidecar.record(edit));
        assert!(!never_developed(&sidecar));
    }
}
