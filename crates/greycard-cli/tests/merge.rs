//! `greycard merge hdr`: a bracket merged exits 0 and writes
//! `<first>-HDR.dng` beside the first frame; one with a frame that
//! moved is written and exits 2, saying so; nothing written exits 1
//! with the reason on stderr. On made-up mosaic DNGs, so it needs no
//! sample raw.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use greycard_core::merge::Noise;
use greycard_core::merge::testing::{mosaic_shifted, write_dng};

/// A folder of the test's own, gone when the test is, however it ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        greycard_core::testing::remove_dir_retry(&self.0);
    }
}

const W: usize = 256;
const H: usize = 160;

/// Three frames at x0.26, x1, x3.8 of one scene, EXIF a stop pair
/// apart; `dx` moves each frame's scene.
fn bracket(dir: &Path, dx: [usize; 3]) -> Vec<PathBuf> {
    let gains = [1.0 / 3.8, 1.0, 3.8];
    let shutters = [(1, 400), (1, 100), (1, 25)];
    (0..3)
        .map(|k| {
            let path = dir.join(format!("IMG_{k}.dng"));
            let samples = mosaic_shifted(W, H, gains[k], Noise::ASSUMED, 5 + k as u64, dx[k]);
            write_dng(&path, samples, W, H, shutters[k], "Bracket");
            path
        })
        .collect()
}

fn merge<S: AsRef<std::ffi::OsStr>>(home: &Path, args: impl IntoIterator<Item = S>) -> Output {
    Command::new(env!("CARGO_BIN_EXE_greycard"))
        .args(["merge", "hdr"])
        .args(args)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_STATE_HOME", home.join("state"))
        .output()
        .expect("greycard runs")
}

#[test]
fn a_merges_exit_code_says_how_it_went() {
    let home = Scratch(
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("greycard-merge-hdr-{}", std::process::id())),
    );
    greycard_core::testing::remove_dir_retry(&home.0);
    let still = home.0.join("still");
    let moved = home.0.join("moved");
    std::fs::create_dir_all(&still).unwrap();
    std::fs::create_dir_all(&moved).unwrap();
    let frames = bracket(&still, [0; 3]);
    let ran = merge(&home.0, &frames);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(
        ran.status.code(),
        Some(0),
        "{said}{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(still.join("IMG_0-HDR.dng").is_file());
    assert!(said.contains("wrote"), "{said}");

    // A frame knocked between shots: written, and said, exit 2.
    let knocked = bracket(&moved, [0, 0, 2]);
    let ran = merge(&home.0, &knocked);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(ran.status.code(), Some(2), "{said}");
    assert!(said.contains("IMG_2.dng had moved"), "{said}");
    assert!(moved.join("IMG_0-HDR.dng").is_file());

    // Nothing written: a frame that is not there, and a file asked for
    // that is no DNG.
    let gone = still.join("gone.dng");
    let ran = merge(&home.0, [frames[0].as_os_str(), gone.as_os_str()]);
    assert_eq!(ran.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ran.stderr).contains("gone.dng"));
    let jpeg = still.join("out.jpg");
    let ran = merge(
        &home.0,
        [
            frames[0].as_os_str(),
            frames[1].as_os_str(),
            "--out".as_ref(),
            jpeg.as_os_str(),
        ],
    );
    assert_eq!(ran.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ran.stderr).contains("not a .dng"));
    assert!(!jpeg.exists());
}
