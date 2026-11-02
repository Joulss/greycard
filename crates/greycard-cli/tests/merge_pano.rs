//! `greycard merge pano`: frames of a camera turned about its lens, of
//! a body whose sensor the lens database knows, merge and exit 0,
//! writing `<first>-pano.dng` beside the first frame; frames it can
//! only lay on a plane are written and exit 2, saying so; frames that
//! share nothing exit 1 with the reason on stderr. On made-up mosaic
//! DNGs and a lens database of one body and its lens, so it needs no
//! sample raw.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use greycard_core::merge::Noise;
use greycard_core::merge::testing::{mosaic_of, turned, write_dng_shot};

/// A folder of the test's own, gone when the test is, however it ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        greycard_core::testing::remove_dir_retry(&self.0);
    }
}

const W: usize = 960;
const H: usize = 640;

/// Frames turned by `yaws` about the lens, 30 mm on a 36 mm sensor.
fn frames(dir: &Path, yaws: &[f64]) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).unwrap();
    yaws.iter()
        .enumerate()
        .map(|(k, &yaw)| {
            let path = dir.join(format!("IMG_{k}.dng"));
            let samples = mosaic_of(W, H, 1.0, Noise::ASSUMED, 20 + k as u64, |x, y, c| {
                turned(x, y, c, W, H, 800.0, yaw)
            });
            write_dng_shot(&path, samples, W, H, (1, 100), 100, "Pano", Some(30));
            path
        })
        .collect()
}

fn merge(home: &Path, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_greycard"))
        .args(["merge", "pano"])
        .args(args)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_STATE_HOME", home.join("state"))
        .output()
        .expect("greycard runs")
}

#[test]
fn a_panoramas_exit_code_says_how_it_went() {
    let home = Scratch(
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("greycard-merge-pano-{}", std::process::id())),
    );
    greycard_core::testing::remove_dir_retry(&home.0);
    let os = |p: &PathBuf| p.as_os_str().to_owned();

    // No lens database: the sensor's size is not known, the frames are
    // laid on a plane, written, and said.
    let planar = frames(&home.0.join("planar"), &[-0.2, 0.2]);
    let args: Vec<_> = planar.iter().map(os).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    let ran = merge(&home.0, &refs);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(
        ran.status.code(),
        Some(2),
        "{said}{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(said.contains("laid on a plane"), "{said}");
    assert!(home.0.join("planar").join("IMG_0-pano.dng").is_file());

    // A database that knows the body and its lens: a cylinder, the
    // frames corrected, exit 0.
    let lensfun = home.0.join("cache/greycard/lensfun/version_2");
    std::fs::create_dir_all(&lensfun).unwrap();
    std::fs::write(
        lensfun.join("synthetic.xml"),
        "<lensdatabase version=\"2\"><camera><maker>Synthetic</maker><model>Pano</model>\
         <mount>None</mount><cropfactor>1</cropfactor></camera>\
         <lens><maker>Synthetic</maker><model>Synthetic 30mm</model><mount>None</mount>\
         <cropfactor>1</cropfactor><calibration>\
         <distortion model=\"poly3\" focal=\"30\" k1=\"0.0001\"/>\
         </calibration></lens></lensdatabase>",
    )
    .unwrap();
    let wide = frames(&home.0.join("wide"), &[-0.45, 0.0, 0.45]);
    let args: Vec<_> = wide.iter().map(os).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    let ran = merge(&home.0, &refs);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(
        ran.status.code(),
        Some(0),
        "{said}{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(said.contains("a cylinder"), "{said}");
    assert!(home.0.join("wide").join("IMG_0-pano.dng").is_file());

    // Nothing written: frames that share nothing, and a file asked for
    // that is no DNG.
    let apart = frames(&home.0.join("apart"), &[0.0, 2.0]);
    let args: Vec<_> = apart.iter().map(os).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    let ran = merge(&home.0, &refs);
    assert_eq!(ran.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&ran.stderr).contains(
            "IMG_0.dng and IMG_1.dng share no matches: a panorama wants frames that overlap"
        ),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(!home.0.join("apart").join("IMG_0-pano.dng").exists());
    let jpeg = home.0.join("wide").join("out.jpg");
    let mut with_out = refs.clone();
    with_out.extend(["--out".as_ref(), jpeg.as_os_str()]);
    let ran = merge(&home.0, &with_out);
    assert_eq!(ran.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ran.stderr).contains("not a .dng"));
}
