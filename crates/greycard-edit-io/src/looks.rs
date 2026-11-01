//! The look directory: where it is, what it holds, and the tables
//! read from it and kept. What a look is, how the list groups, and
//! what the panel says are `greycard_edit::look`'s.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use greycard_core::lut::{self, Lut3d};
use greycard_edit::DisplayCurve;
use greycard_edit::look::{Entry, LookLut, LutChoice, MadeFor, entry_of, gate, is_a_name};

/// Where the look tables live: `$XDG_DATA_HOME/greycard/looks` when
/// that is set, else `greycard/looks` under the platform's data
/// directory — `~/.local/share` on Linux, `~/Library/Application
/// Support` on macOS, `%APPDATA%` on Windows (notes §106). The same
/// logic the profile directory uses, one name along.
pub fn store_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(dirs::data_dir)?;
    Some(base.join("greycard").join("looks"))
}

/// Where a table of this name is kept. A name that is a path, or tries
/// to climb out of the directory, is refused.
///
/// Two extensions are read, so the one that is there wins; with
/// neither there the `.cube` path comes back, to be named in the
/// warning that follows.
pub fn path_for(name: &str) -> Option<PathBuf> {
    if !is_a_name(name) {
        log::warn!("look {name:?} is not a name in the look directory");
        return None;
    }
    Some(path_in(&store_dir()?, name))
}

/// Where a look's variant for `curve` would be in `dir`:
/// `<name>.<curve>.cube`, whichever curve, so a per-channel variant
/// written by hand is read too.
fn variant_in(dir: &Path, name: &str, curve: DisplayCurve) -> PathBuf {
    dir.join(format!("{name}.{}.cube", curve.key()))
}

/// Every table of the look `name` in `dir` that a picture under
/// `curve` may be finished through: the look's own file, whichever
/// extension is there, and its variant for that curve. For the develop
/// key, which takes their contents.
pub(crate) fn tables_in(dir: &Path, name: &str, curve: DisplayCurve) -> Vec<PathBuf> {
    if !is_a_name(name) {
        return Vec::new();
    }
    vec![path_in(dir, name), variant_in(dir, name, curve)]
}

/// [`path_for`] in the directory `dir`, the name already checked.
fn path_in(dir: &Path, name: &str) -> PathBuf {
    let mut first = None;
    for extension in lut::EXTENSIONS {
        let path = dir.join(format!("{name}.{extension}"));
        if path.is_file() {
            return path;
        }
        first.get_or_insert(path);
    }
    first.expect("at least one extension")
}

/// The look `look` names as a picture under `curve` finishes through
/// it, from the look directory `dir`: [`LookLut::look_under`] with the
/// look directory there.
pub fn look_under_in(dir: &Path, look: &LookLut, curve: DisplayCurve) -> Option<lut::Look> {
    let LutChoice::Named(name) = &look.lut else {
        return None;
    };
    under_in(dir, name, look.strength, curve)
}

/// The look `name` at `strength` under `curve` in `dir`: its variant
/// for the curve when one there declares it, else its own file through
/// [`gate`].
pub(crate) fn under_in(
    dir: &Path,
    name: &str,
    strength: f32,
    curve: DisplayCurve,
) -> Option<lut::Look> {
    if !is_a_name(name) {
        return None;
    }
    let with = |path: &Path, table: Arc<Lut3d>| match lut::Look::new(table, strength) {
        Ok(look) => Some(look),
        Err(e) => {
            log::warn!("look {}: {e}", path.display());
            None
        }
    };
    let variant = variant_in(dir, name, curve);
    if variant.is_file()
        && let Some(table) = load(&variant)
        && MadeFor::read(&table.comments) == MadeFor::Curve(curve)
    {
        return with(&variant, table);
    }
    let own = path_in(dir, name);
    gate(load(&own).and_then(|t| with(&own, t)), curve)
}

/// Whether the look directory has any table of this look's name,
/// its own file or a variant for a curve: what tells a look that
/// is missing from one that is off for a picture's curve.
///
/// A variant counts only when its header declares its curve, the
/// test the listing ([`entry_of`]) and [`LookLut::look_under`]
/// apply: `Ghost.agx.cube` declaring per channel is not a table of
/// "Ghost", so a picture naming "Ghost" with no `Ghost.cube` is a
/// look that is not there.
pub fn is_there(look: &LookLut) -> bool {
    store_dir().is_some_and(|dir| is_there_in(&dir, look))
}

/// [`is_there`] in the look directory `dir`.
pub fn is_there_in(dir: &Path, look: &LookLut) -> bool {
    let LutChoice::Named(name) = &look.lut else {
        return false;
    };
    if !is_a_name(name) {
        return false;
    }
    path_in(dir, name).is_file()
        || DisplayCurve::ALL.into_iter().any(|c| {
            let path = variant_in(dir, name, c);
            path.is_file()
                && Lut3d::info(&path).is_ok_and(|i| MadeFor::read(&i.comments) == MadeFor::Curve(c))
        })
}

/// Every look table in the directory, by name.
///
/// Only each file's header is read — its size and what it says it is —
/// not its table: a directory of film stocks at 64 nodes an axis is
/// megabytes of floats nobody has asked for yet, and this runs on the
/// thread that draws the panel. The table that is chosen is read in
/// full by [`load`], once. A file that will not read is passed over
/// with a word in the log.
pub fn list() -> Vec<Entry> {
    // Nothing is forgotten here. [`load`] reads a file again when its
    // size or its clock moved, so a table replaced in place is picked
    // up anyway, and dropping the cache on every listing would re-read
    // the open picture's table and re-upload it to the GPU each time
    // the section is opened.
    let Some(dir) = store_dir() else {
        return Vec::new();
    };
    lut::list_dir(&dir)
        .into_iter()
        .filter_map(|(stem, path, info)| match info {
            Ok(info) => Some(entry_of(&stem, path, info)),
            Err(e) => {
                log::warn!("look {}: {e}", path.display());
                None
            }
        })
        .collect()
}

/// What a path read to last time.
enum Cached {
    /// There was no file to read, and it has been said once.
    Missing,
    /// A file of this age and size read to this, or to nothing.
    Read {
        stamp: Option<std::time::SystemTime>,
        len: u64,
        table: Option<Arc<Lut3d>>,
    },
}

/// How many tables are kept at once. One edit names one, and a
/// filmstrip of files naming different ones is the only way to reach
/// even a handful; past this the lot is dropped rather than grown.
const KEEP: usize = 4;

static CACHE: LazyLock<Mutex<HashMap<PathBuf, Cached>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The table in a file, read once and kept until the file changes.
///
/// A file that is missing, or that will not read, is remembered as
/// such, so the warning is said once rather than on every develop.
pub fn load(path: &Path) -> Option<Arc<Lut3d>> {
    let stamped = match std::fs::metadata(path) {
        Ok(m) => Some((m.modified().ok(), m.len())),
        Err(_) => None,
    };
    let mut cache = CACHE.lock().ok()?;
    match (cache.get(path), &stamped) {
        (Some(Cached::Missing), None) => return None,
        (Some(Cached::Read { stamp, len, table }), Some((now, size)))
            if stamp == now && len == size =>
        {
            return table.clone();
        }
        _ => {}
    }
    if cache.len() >= KEEP {
        cache.clear();
    }
    let Some((stamp, len)) = stamped else {
        log::warn!("look {}: it is not there", path.display());
        cache.insert(path.to_path_buf(), Cached::Missing);
        return None;
    };
    let table = match Lut3d::load(path) {
        Ok(table) => Some(Arc::new(table)),
        Err(e) => {
            log::warn!("look {}: {e}", path.display());
            None
        }
    };
    cache.insert(
        path.to_path_buf(),
        Cached::Read {
            stamp,
            len,
            table: table.clone(),
        },
    );
    table
}

/// Forget what has been read, so the next develop reads it again.
pub fn forget() {
    if let Ok(mut cache) = CACHE.lock() {
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use greycard_edit::look::{FITTED, table_for};

    /// A `.cube`'s text: a table that takes everything to one color,
    /// so a strength is easy to read off.
    fn flat_cube(to: [f32; 3]) -> String {
        let mut text = String::from("TITLE \"Flat\"\nLUT_3D_SIZE 2\n");
        for _ in 0..8 {
            text.push_str(&format!("{} {} {}\n", to[0], to[1], to[2]));
        }
        text
    }

    #[test]
    fn a_name_that_is_a_path_is_refused() {
        assert!(path_for("../../etc/passwd").is_none());
        assert!(path_for("sub/dir").is_none());
        assert!(path_for("/etc/passwd").is_none());
        assert!(path_for("").is_none());
    }

    /// The cache is one static for the whole process, and these tests
    /// say what is in it, so they take their turn rather than run
    /// alongside each other.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    fn scratch(what: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("greycard-look-{what}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_table_file_reads_once_and_is_kept() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("ok");
        let path = dir.join("Flat.cube");
        std::fs::write(&path, flat_cube([1.0, 0.0, 0.0])).unwrap();
        let table = load(&path).expect("it reads");
        assert_eq!(table.title.as_deref(), Some("Flat"));
        assert!(Arc::ptr_eq(&table, &load(&path).unwrap()));
        // Rewritten, it is read afresh: the length alone says so.
        std::fs::write(&path, flat_cube([0.125, 0.0, 0.0])).unwrap();
        let again = load(&path).expect("the new one reads");
        assert!(!Arc::ptr_eq(&table, &again));
        forget();
        assert!(!Arc::ptr_eq(&again, &load(&path).unwrap()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_that_is_not_a_table_resolves_to_nothing() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("junk");
        let path = dir.join("not-a-lut.cube");
        std::fs::write(&path, b"this is not a lookup table").unwrap();
        assert!(load(&path).is_none());
        // Twice: the second answer comes from the cache.
        assert!(load(&path).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_table_is_remembered_as_missing() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("missing");
        let path = dir.join("Not Here Yet.cube");
        assert!(load(&path).is_none());
        assert!(matches!(
            CACHE.lock().unwrap().get(&path),
            Some(Cached::Missing)
        ));
        assert!(load(&path).is_none());
        std::fs::write(&path, flat_cube([0.5, 0.5, 0.5])).unwrap();
        assert!(load(&path).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// On disk: a look's own file and its AgX variant beside it, each
    /// taking everything to one color. A picture on each curve takes
    /// its own table; a look with only a per-channel table is no look
    /// under AgX; a film preset applies under both; a stray `.agx`
    /// file that does not declare AgX is not taken as the variant.
    #[test]
    fn a_look_resolves_to_its_table_for_the_pictures_curve() {
        let _turn = ONE_AT_A_TIME.lock();
        use DisplayCurve::*;
        let dir = scratch("curves");
        let table = |file: &str, header: &str, to: [f32; 3]| {
            let text = flat_cube(to).replacen("LUT_3D_SIZE", &format!("{header}LUT_3D_SIZE"), 1);
            std::fs::write(dir.join(file), text).unwrap();
        };
        let fit = format!("# {FITTED}\n");
        table("R6.cube", &fit, [1.0, 0.0, 0.0]);
        table(
            "R6.agx.cube",
            &format!("{fit}# display_curve: agx\n"),
            [0.0, 1.0, 0.0],
        );
        table("R5.cube", &fit, [0.0, 0.0, 1.0]);
        table("R5.agx.cube", "# made by hand\n", [0.5, 0.5, 0.5]);
        table("Film.cube", "", [0.25, 0.25, 0.25]);
        table(
            "Ghost.agx.cube",
            &format!("{fit}# display_curve: channels\n"),
            [0.1; 3],
        );
        let under = |name: &str, curve| {
            let look = LookLut {
                lut: LutChoice::Named(name.into()),
                strength: 1.0,
            };
            look_under_in(&dir, &look, curve).map(|l| l.lut.data[0])
        };
        assert_eq!(under("R6", Channels), Some([1.0, 0.0, 0.0]));
        assert_eq!(under("R6", Agx), Some([0.0, 1.0, 0.0]));
        assert_eq!(under("R5", Channels), Some([0.0, 0.0, 1.0]));
        assert_eq!(under("R5", Agx), None);
        assert_eq!(under("Film", Agx), Some([0.25; 3]));
        assert_eq!(under("Film", Channels), Some([0.25; 3]));
        assert_eq!(under("Gone", Agx), None);
        // Whether a look is there at all agrees with the listing: a
        // variant counts only when it declares its curve.
        let named = |name: &str| LookLut {
            lut: LutChoice::Named(name.into()),
            strength: 1.0,
        };
        assert!(is_there_in(&dir, &named("R6")));
        assert!(is_there_in(&dir, &named("Film")));
        assert!(!is_there_in(&dir, &named("Ghost")));
        assert!(!is_there_in(&dir, &named("Gone")));
        assert_eq!(under("Ghost", Agx), None);
        assert_eq!(under("Ghost", Channels), None);
        // And the listing reads the same directory the same way.
        let entries: Vec<Entry> = lut::list_dir(&dir)
            .into_iter()
            .map(|(stem, path, info)| entry_of(&stem, path, info.unwrap()))
            .collect();
        assert_eq!(table_for(&entries, "R6", Agx).unwrap().variant, Some(Agx));
        assert!(table_for(&entries, "R5", Agx).is_none());
        assert!(entries.iter().any(|e| e.name == "R5.agx"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The directory the panel lists, and what a header read gives it.
    #[test]
    fn the_directory_lists_what_it_holds() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("list");
        std::fs::write(dir.join("Flat.cube"), flat_cube([1.0, 0.0, 0.0])).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a look").unwrap();
        let listed = lut::list_dir(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "Flat");
        assert_eq!(listed[0].2.as_ref().unwrap().size, 2);
        // A header read keeps nothing: the table is only built for the
        // one that is chosen.
        assert!(!CACHE.lock().unwrap().contains_key(&listed[0].1));
        std::fs::remove_dir_all(&dir).ok();
    }
}
