//! The camera profile directory: where it is, what it holds, and the
//! profiles read from it and kept. What a profile is, how one fits a
//! body, and what the panel lists and says are
//! `greycard_edit::camera`'s.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use greycard_core::Dcp;
use greycard_core::color::Profile;
use greycard_edit::camera::{EXTENSION, Entry};

/// Where the profiles live: `$XDG_DATA_HOME/greycard/profiles` when
/// that is set, else `greycard/profiles` under the platform's data
/// directory — `~/.local/share` on Linux, `~/Library/Application
/// Support` on macOS, `%APPDATA%` on Windows (notes §106).
pub fn store_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(dirs::data_dir)?;
    Some(base.join("greycard").join("profiles"))
}

/// Where a profile of this name is kept. A name that is a path, or
/// tries to climb out of the directory, is refused.
pub fn path_for(name: &str) -> Option<PathBuf> {
    path_in(store_dir().as_deref(), name)
}

/// [`path_for`] in the profile directory `dir`, `None` for none.
pub fn path_in(dir: Option<&Path>, name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name.contains("..")
        || Path::new(name).is_absolute()
    {
        log::warn!("camera profile {name:?} is not a name in the profile directory");
        return None;
    }
    Some(dir?.join(format!("{name}.{EXTENSION}")))
}

/// Every profile in the directory, by name.
///
/// Only each file's header is read — its name, its camera and its
/// copyright — not its tables: a directory of a dozen profiles with
/// look tables is megabytes of floats nobody has asked for yet, and
/// this runs on the thread that draws the panel. The profile that is
/// chosen is read in full by [`load`], once. A file that will not read
/// is passed over with a word in the log. Two that are one profile
/// under two file names are paired by [`pair_up`].
pub fn list() -> Vec<Entry> {
    // A listing is also the moment to let go of what was read before
    // it: a profile that has been replaced or is no longer wanted has
    // no claim on the memory its tables take.
    forget();
    match store_dir() {
        Some(dir) => list_in(&dir),
        None => Vec::new(),
    }
}

fn list_in(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Entry> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION))
        })
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let info = match Dcp::info(&path) {
                Ok(info) => info,
                Err(e) => {
                    log::warn!("camera profile {}: {e}", path.display());
                    return None;
                }
            };
            Some(Entry {
                name,
                path,
                title: info.name,
                unique_camera_model: info.unique_camera_model,
                copyright: info.copyright,
                same_as: None,
            })
        })
        .collect();
    // By file name first, so "the first by name" below is this order.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    pair_up(&mut out);
    out.sort_by_key(|e| e.label().to_lowercase());
    out
}

/// Two files that are one profile under two names, as a body's Adobe
/// Standard saved once by its US name and once by its own, list as
/// one. `entries` is in file-name order.
///
/// Only entries whose camera and name collide are looked at further,
/// and only those whose lengths agree are read: the listing otherwise
/// stays a header read.
fn pair_up(entries: &mut [Entry]) {
    let key = |e: &Entry| (e.folded_unique(), e.label().to_string());
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        groups.entry(key(e)).or_default().push(i);
    }
    for group in groups.values().filter(|g| g.len() > 1) {
        // The same contents: equal lengths, then equal bytes, each
        // file read at most once. A file that will not read is its own
        // profile.
        let lens: Vec<Option<u64>> = group
            .iter()
            .map(|&i| std::fs::metadata(&entries[i].path).ok().map(|m| m.len()))
            .collect();
        let mut contents: Vec<Option<Vec<u8>>> = vec![None; group.len()];
        let mut same: Vec<Vec<usize>> = Vec::new();
        for a in 0..group.len() {
            let found = same.iter().position(|class| {
                let b = class[0];
                if lens[a].is_none() || lens[a] != lens[b] {
                    return false;
                }
                for k in [a, b] {
                    if contents[k].is_none() {
                        contents[k] =
                            Some(std::fs::read(&entries[group[k]].path).unwrap_or_default());
                    }
                }
                contents[a].as_ref().is_some_and(|c| !c.is_empty()) && contents[a] == contents[b]
            });
            match found {
                Some(c) => same[c].push(a),
                None => same.push(vec![a]),
            }
        }
        let same: Vec<Vec<usize>> = same
            .into_iter()
            .map(|class| class.into_iter().map(|k| group[k]).collect())
            .collect();
        for class in same.iter().filter(|c| c.len() > 1) {
            // The file named for the profile stands for the pair, else
            // the first by name.
            let title = entries[class[0]].label().to_lowercase();
            let stands = class
                .iter()
                .copied()
                .find(|&i| entries[i].name.to_lowercase().contains(&title))
                .unwrap_or(class[0]);
            let name = entries[stands].name.clone();
            for &i in class.iter().filter(|&&i| i != stands) {
                entries[i].same_as = Some(name.clone());
            }
        }
    }
}

/// What a path read to last time.
enum Cached {
    /// There was no file to read, and it has been said once.
    Missing,
    /// A file of this age and size read to this, or to nothing.
    Read {
        stamp: Option<std::time::SystemTime>,
        len: u64,
        profile: Option<Arc<Profile>>,
    },
}

/// How many profiles are kept at once. One edit names one, and a
/// filmstrip of files naming different ones is the only way to reach
/// even a handful; past this the lot is dropped rather than grown.
const KEEP: usize = 8;

static CACHE: LazyLock<Mutex<HashMap<PathBuf, Cached>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The profile in a file, read once and kept until the file changes.
///
/// A file that is missing, or that will not read, is remembered as
/// such, so the warning is said once rather than on every develop.
pub fn load(path: &Path) -> Option<Arc<Profile>> {
    let stamped = match std::fs::metadata(path) {
        Ok(m) => Some((m.modified().ok(), m.len())),
        Err(_) => None,
    };
    let mut cache = CACHE.lock().ok()?;
    match (cache.get(path), &stamped) {
        // Missing, and already said so.
        (Some(Cached::Missing), None) => return None,
        // Read before, from a file that has not been touched since.
        (
            Some(Cached::Read {
                stamp,
                len,
                profile,
            }),
            Some((now, size)),
        ) if stamp == now && len == size => {
            return profile.clone();
        }
        _ => {}
    }
    if cache.len() >= KEEP {
        cache.clear();
    }
    let Some((stamp, len)) = stamped else {
        log::warn!("camera profile {}: it is not there", path.display());
        cache.insert(path.to_path_buf(), Cached::Missing);
        return None;
    };
    let profile = match Dcp::load(path).and_then(|dcp| dcp.profile()) {
        Ok(profile) => Some(Arc::new(profile)),
        Err(e) => {
            log::warn!("camera profile {}: {e}", path.display());
            None
        }
    };
    cache.insert(
        path.to_path_buf(),
        Cached::Read {
            stamp,
            len,
            profile: profile.clone(),
        },
    );
    profile
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
    use greycard_edit::camera::{Body, EMBEDDED, listed, warning};

    #[test]
    fn a_name_that_is_a_path_is_refused() {
        assert!(path_for("../../etc/passwd").is_none());
        assert!(path_for("sub/dir").is_none());
        assert!(path_for("/etc/passwd").is_none());
        assert!(path_for("").is_none());
    }

    /// A small but real DCP, written by the engine's own writer.
    /// `hues` changes both what is in it and how long it is, so a
    /// rewrite is a different file whatever the clock's resolution.
    fn a_profile_file(path: &Path, hues: usize) {
        a_profile(path, "Test Camera Neutral", "Test Cam", hues);
    }

    /// [`a_profile_file`] with its own name and camera.
    fn a_profile(path: &Path, name: &str, camera: &str, hues: usize) {
        use greycard_core::dcp::{DcpCalibration, EmbedPolicy, HueSatMap};
        let matrix = [0.8, -0.1, -0.1, -0.4, 1.3, 0.1, 0.0, -0.2, 0.9];
        let dcp = Dcp {
            name: Some(name.into()),
            unique_camera_model: Some(camera.into()),
            copyright: Some("public domain".into()),
            calibration_signature: None,
            embed_policy: EmbedPolicy::AllowCopying,
            primary: DcpCalibration {
                illuminant: 17,
                color_matrix: matrix,
                forward_matrix: None,
                hue_sat_map: Some(HueSatMap::identity(hues, 2, 1)),
            },
            secondary: None,
            baseline_exposure_offset: None,
            look: None,
            tone_curve: None,
        };
        std::fs::write(path, dcp.to_bytes()).unwrap();
    }

    /// The cache is one static for the whole process, and these tests
    /// say what is in it, so they take their turn rather than run
    /// alongside each other.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// A directory of this test's own, gone when it returns.
    fn scratch(what: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("greycard-dcp-{what}-{}", std::process::id()));
        greycard_core::testing::remove_dir_retry(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_profile_file_reads_to_a_profile_and_is_kept() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("ok");
        let path = dir.join("Test Cam Neutral.dcp");
        a_profile_file(&path, 4);

        let profile = load(&path).expect("the profile reads");
        assert_eq!(profile.name.as_deref(), Some("Test Camera Neutral"));
        assert!(profile.maps.is_some(), "it carries the map");
        // The second read is the same profile, not another copy of it.
        assert!(Arc::ptr_eq(&profile, &load(&path).unwrap()));

        // Rewritten with a map of another size, it is read afresh: the
        // length alone says so, without leaning on the file clock.
        a_profile_file(&path, 8);
        let again = load(&path).expect("the new one reads");
        assert!(!Arc::ptr_eq(&profile, &again));
        assert_eq!(again.maps.as_ref().unwrap().primary.hue_divisions, 8);

        // And forgetting makes the next read a fresh one too.
        forget();
        assert!(!Arc::ptr_eq(&again, &load(&path).unwrap()));
        greycard_core::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn a_file_that_is_not_a_profile_resolves_to_nothing() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("junk");
        let path = dir.join("not-a-profile.dcp");
        std::fs::write(&path, b"this is not a camera profile").unwrap();
        assert!(load(&path).is_none());
        // Twice: the second answer comes from the cache, and is the
        // same nothing.
        assert!(load(&path).is_none());
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// A profile the edit names and the directory has not got is
    /// remembered as missing, so the develop that follows every slider
    /// does not say so again; and it is read the moment it appears.
    #[test]
    fn a_missing_profile_is_remembered_as_missing() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("missing");
        let path = dir.join("Not Here Yet.dcp");
        assert!(load(&path).is_none());
        assert!(matches!(
            CACHE.lock().unwrap().get(&path),
            Some(Cached::Missing)
        ));
        assert!(load(&path).is_none());

        // Put it there and it is read, missing marker and all.
        a_profile_file(&path, 4);
        assert!(load(&path).is_some());
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// The listing reads headers, not tables: a profile in the
    /// directory is named without its map being built.
    #[test]
    fn listing_does_not_read_the_tables() {
        let _turn = ONE_AT_A_TIME.lock();
        let dir = scratch("list");
        let path = dir.join("Test Cam Neutral.dcp");
        a_profile_file(&path, 4);
        forget();
        let info = Dcp::info(&path).unwrap();
        assert_eq!(info.name.as_deref(), Some("Test Camera Neutral"));
        assert_eq!(info.unique_camera_model.as_deref(), Some("Test Cam"));
        assert!(
            !CACHE.lock().unwrap().contains_key(&path),
            "a header read keeps nothing"
        );
        greycard_core::testing::remove_dir_retry(&dir);
    }

    fn labels<'a>(entries: &'a [Entry], chosen: &str) -> Vec<&'a str> {
        listed(entries, chosen).map(Entry::label).collect()
    }

    /// The tester's case: a body's Adobe Standard saved under its own
    /// name and under its US one is one profile, listed once, by the
    /// file named for the profile even when it is not the first by
    /// name; an edit naming the other file still finds it, and it fits
    /// the body by its other name.
    #[test]
    fn one_profile_under_two_file_names_lists_once() {
        let dir = scratch("pair");
        let rebel = "Canon EOS Rebel T8i";
        let (first, named) = ("Canon EOS 850D", "Canon EOS Rebel T8i Adobe Standard");
        for name in [first, named] {
            a_profile(&dir.join(format!("{name}.dcp")), "Adobe Standard", rebel, 4);
        }
        let entries = list_in(&dir);
        assert_eq!(entries.len(), 2, "both files are entries");
        let shown: Vec<&str> = listed(&entries, EMBEDDED)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(shown, [named]);
        assert_eq!(labels(&entries, EMBEDDED), ["Adobe Standard"]);
        // A sidecar naming the other file shows that file in its place.
        let shown: Vec<&str> = listed(&entries, first).map(|e| e.name.as_str()).collect();
        assert_eq!(shown, [first]);
        assert_eq!(labels(&entries, first), ["Adobe Standard"]);
        // And the warning finds it by its name, not its label.
        let aliases = ["EOS Rebel T8i".to_string()];
        let body = Body::new("Canon", "EOS 850D", &aliases);
        assert!(warning(&entries, body, first).is_empty());
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// Two profiles of one name for one camera, with different tables,
    /// are two profiles: both listed, for the panel to tell apart.
    #[test]
    fn two_profiles_of_one_name_both_list() {
        let dir = scratch("twins");
        let rebel = "Canon EOS Rebel T8i";
        a_profile(&dir.join("old.dcp"), "Adobe Standard", rebel, 4);
        a_profile(&dir.join("new.dcp"), "Adobe Standard", rebel, 8);
        // The same length, different bytes: read, and kept apart.
        a_profile(
            &dir.join("other.dcp"),
            "Adobe Standard",
            "CANON EOS REBEL T8I",
            4,
        );
        let tables = |n: &str| std::fs::read(dir.join(n)).unwrap();
        let mut other = tables("other.dcp");
        let last = other.len() - 1;
        other[last] ^= 1;
        std::fs::write(dir.join("other.dcp"), &other).unwrap();
        assert_eq!(other.len(), tables("old.dcp").len());
        let entries = list_in(&dir);
        let shown: Vec<&str> = listed(&entries, EMBEDDED)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(shown, ["new", "old", "other"]);
        assert!(entries.iter().all(|e| e.same_as.is_none()));
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// One name for two cameras: two profiles, listed plainly.
    #[test]
    fn one_name_for_two_cameras_lists_plainly() {
        let dir = scratch("cameras");
        a_profile(&dir.join("r5.dcp"), "Adobe Standard", "Canon EOS R5", 4);
        a_profile(&dir.join("z8.dcp"), "Adobe Standard", "NIKON Z 8", 4);
        let entries = list_in(&dir);
        assert_eq!(
            labels(&entries, EMBEDDED),
            ["Adobe Standard", "Adobe Standard"]
        );
        greycard_core::testing::remove_dir_retry(&dir);
    }
}
