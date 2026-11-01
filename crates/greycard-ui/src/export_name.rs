//! The export sheet's Name: each frame's file name, without its
//! extension, from the pattern, the tokens filled from the frame.
//!
//! `{name}` and `{seq}` need nothing but the frame's place; `{date}`
//! and `{camera}` need the frame's [`Shot`], which the window takes
//! from the library's index and reads from the file only for a frame
//! the index has no row for ([`read_shot`]). The pattern `{name}`
//! names each file exactly as its source is named, as exports always
//! were.

use crate::naming::{self, Day, EXPORT_TOKENS, Fields, PatternError};
use greycard_engine::sheet::NAME;
use std::path::{Path, PathBuf};

/// What `{date}` and `{camera}` fill from: the camera's own capture
/// time, in EXIF's or the index's form, and the camera as the panel
/// names it (make and model, the make not said twice).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shot {
    pub taken: Option<String>,
    pub camera: String,
}

/// One frame of an export as its name is made: its file, its
/// [`Shot`] (none when the pattern needs none), and the shift of its
/// capture time (`Meta::time_shift`, in seconds).
pub type Frame = (PathBuf, Option<Shot>, i64);

/// Whether the pattern needs a frame's [`Shot`].
pub fn wants_shot(pattern: &str) -> bool {
    wants_date(pattern) || pattern.contains("{camera")
}

/// Whether the pattern needs a frame's capture day, and so its shift.
pub fn wants_date(pattern: &str) -> bool {
    pattern.contains("{date")
}

/// A frame's [`Shot`] from the file itself, for one the index has no
/// row for: a raw's metadata from the pieces of the file it is in (a
/// few hundred kilobytes, not the file), a picture's from its header.
/// A file that will not read has none, and its day is the file's.
pub fn read_shot(path: &Path) -> Shot {
    let read = std::panic::catch_unwind(|| {
        if greycard_core::picture::is_picture_path(path) {
            greycard_core::picture::probe_path(path)
        } else {
            greycard_core::decode::index_read_path(path).map(|(probe, _, _)| probe)
        }
    });
    match read {
        Ok(Ok(p)) => Shot {
            taken: p.taken,
            camera: greycard_core::raw::camera_name(&p.make, &p.model),
        },
        _ => Shot::default(),
    }
}

/// What a pattern fills from one frame, the `seq`th of the export. The
/// day is the capture time moved by the shift, as the export's own
/// dates are; with none, the day the file was last written, and with
/// no file either, today.
fn fields(source: &Path, shot: Option<&Shot>, shift: i64, seq: usize) -> Fields {
    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(shot) = shot else {
        let day = Day {
            year: 1970,
            month: 1,
            day: 1,
        };
        return Fields {
            day,
            name,
            camera: String::new(),
            seq,
            folder: String::new(),
        };
    };
    let day = shot
        .taken
        .as_deref()
        .and_then(|t| greycard_core::exif::shifted(t, shift))
        .and_then(|t| Day::from_exif(&t))
        .unwrap_or_else(|| crate::import::modified_day(source));
    Fields {
        day,
        name,
        camera: shot.camera.clone(),
        seq,
        folder: String::new(),
    }
}

/// The pattern as the export reads it: blank is `{name}`.
fn read(pattern: &str) -> &str {
    match pattern.trim() {
        "" => NAME,
        p => p,
    }
}

/// The frame's stem when the pattern names it no other way.
fn own(source: &Path) -> String {
    source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into())
}

/// The stem of `source`, the `seq`th of `count` frames, from
/// `pattern`; the error when the pattern makes none. `shot` is the
/// frame's [`Shot`] when the pattern [`wants_shot`].
pub fn stem(
    pattern: &str,
    source: &Path,
    shot: Option<&Shot>,
    shift: i64,
    seq: usize,
    count: usize,
) -> Result<String, PatternError> {
    let pattern = read(pattern);
    if pattern == NAME {
        return Ok(own(source));
    }
    naming::export_stem(pattern, &fields(source, shot, shift, seq), count)
}

/// Each frame's stem from `pattern`, in the set's order: `{seq}` is
/// the frame's place in it. A frame the pattern makes no name for is
/// named as its source is, and the first reason is handed back for
/// the log.
pub fn stems(pattern: &str, frames: &[Frame]) -> (Vec<String>, Option<String>) {
    let mut problem = None;
    let stems = frames
        .iter()
        .enumerate()
        .map(|(i, (source, shot, shift))| {
            stem(pattern, source, shot.as_ref(), *shift, i + 1, frames.len()).unwrap_or_else(|e| {
                problem.get_or_insert_with(|| said(pattern, &e));
                own(source)
            })
        })
        .collect();
    (stems, problem)
}

/// Whether the pattern itself is refused: an unknown or unclosed
/// token, or nothing a file can be named, whatever the frame.
pub fn refused(pattern: &str) -> bool {
    let sample = Fields {
        day: Day {
            year: 2026,
            month: 1,
            day: 1,
        },
        name: "IMG_0001".into(),
        camera: "Camera".into(),
        seq: 1,
        folder: String::new(),
    };
    naming::export_stem(read(pattern), &sample, 1).is_err()
}

/// The log's line for a pattern that names no file.
fn said(pattern: &str, e: &PatternError) -> String {
    let why = match e {
        PatternError::Unknown(t) => {
            format!("{{{t}}} is not a token; {}", EXPORT_TOKENS.join(" "))
        }
        e => e.to_string(),
    };
    format!("export name {pattern}: {why}; named as {NAME} instead")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_the_source_s_own_and_reads_nothing() {
        // A file that is not there: nothing is read for these tokens.
        let p = Path::new("/nowhere/IMG_0042.CR3");
        assert_eq!(stem("{name}", p, None, 0, 1, 1).unwrap(), "IMG_0042");
        assert_eq!(stem("  ", p, None, 0, 1, 1).unwrap(), "IMG_0042");
        assert_eq!(
            stem("trip {seq:101}-{name}", p, None, 0, 3, 12).unwrap(),
            "trip 103-IMG_0042"
        );
        assert!(!wants_shot("trip {seq:101}-{name}"));
        assert!(wants_shot("{camera}") && !wants_date("{camera}"));
        assert!(wants_date("{date:%Y}"));
        // `{name}` alone keeps a name `safe` would change.
        let odd = Path::new("/nowhere/.CON.CR3");
        assert_eq!(stem("{name}", odd, None, 0, 1, 1).unwrap(), ".CON");
    }

    /// The day is the capture time moved by the shift, in either
    /// spelling; with none, the file's day, and with no file, today's.
    #[test]
    fn the_date_is_the_shifted_capture_day_else_the_file_s() {
        let p = Path::new("/nowhere/IMG_0042.CR3");
        let shot = Shot {
            taken: Some("2026:10:30 23:30:00".into()),
            camera: "Canon EOS R5".into(),
        };
        let at = |shot: &Shot, shift| stem("{date} {camera}", p, Some(shot), shift, 1, 1);
        assert_eq!(at(&shot, 0).unwrap(), "2026-10-30 Canon EOS R5");
        assert_eq!(at(&shot, 3600).unwrap(), "2026-10-31 Canon EOS R5");
        let indexed = Shot {
            taken: Some("2026-10-30 23:30:00".into()),
            ..shot.clone()
        };
        assert_eq!(at(&indexed, 3600).unwrap(), "2026-10-31 Canon EOS R5");
        let today = crate::import::modified_day(p);
        let none = Shot::default();
        assert_eq!(
            stem("{date}", p, Some(&none), 0, 1, 1).unwrap(),
            format!("{:04}-{:02}-{:02}", today.year, today.month, today.day)
        );
    }

    #[test]
    fn a_bad_pattern_names_as_the_source_and_says_why() {
        let frames: Vec<Frame> = vec![
            (PathBuf::from("/nowhere/A.CR3"), None, 0),
            (PathBuf::from("/nowhere/B.CR3"), None, 0),
        ];
        let (stems_got, problem) = stems("{name}-{lens}", &frames);
        assert_eq!(stems_got, ["A", "B"]);
        let problem = problem.unwrap();
        assert!(problem.contains("{lens} is not a token"), "{problem}");
        assert!(problem.contains("{camera}"), "{problem}");
        let (_, problem) = stems("{name", &frames);
        assert!(problem.unwrap().contains("no }"));
        let (stems_got, problem) = stems("...", &frames);
        assert_eq!(stems_got, ["A", "B"]);
        assert!(problem.unwrap().contains("names nothing"));
        // A good one says nothing.
        let (stems_got, problem) = stems("{seq}_{name}", &frames);
        assert_eq!(stems_got, ["1_A", "2_B"]);
        assert_eq!(problem, None);
        assert!(refused("{yyyy}") && refused("{name") && refused(" . "));
        assert!(!refused("") && !refused("{date:%Y%m%d}-{seq:7}"));
    }
}
