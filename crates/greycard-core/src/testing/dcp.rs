//! What the `dcp` fuzz target asserts, here so the core's own tests can
//! run the same checks without cargo-fuzz.
//!
//! A DCP is a file the user picks from disk, so its reader takes bytes
//! nobody vetted. [`check`] holds the reader to more than "never
//! panics": a profile that reads has tables of the sizes it claims and
//! within the caps, a sane offset, writes back as a file that reads the
//! same, and its map and look table, run as the develop runs them,
//! leave every pixel of a fixed set finite.
//!
//! No profile is committed as a seed. [`synthetic`] builds a small one
//! that carries every tag the reader knows, and [`patched`] lays the
//! fuzzer's bytes over it, so a run explores a well-formed file without
//! one in the repository.

use rawcolor::{TempTint, Vec3, WhiteBalance};

use crate::color::{CAT, WORKING_SPACE};
use crate::dcp::{
    Dcp, DcpCalibration, EmbedPolicy, Encoding, HueSatMap, MAX_DIVISIONS, MAX_OFFSET_STOPS,
    MAX_TABLE_ENTRY,
};
use crate::develop::profile::MapStage;

/// Pixels the stages are run on: black, white, grey, the primaries, a
/// highlight, colors outside the space, all negative, very large, and
/// very small.
pub const PIXELS: [[f32; 3]; 14] = [
    [0.0, 0.0, 0.0],
    [1.0, 1.0, 1.0],
    [0.18, 0.18, 0.18],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [16.0, 8.0, 4.0],
    [-0.1, 0.3, 0.6],
    [0.4, -0.2, -0.1],
    [-1.0, -1.0, -1.0],
    [1.0e4, 1.0e4, 1.0e4],
    [1.0e4, 0.0, -1.0e3],
    [1.0e-30, 1.0e-30, 2.0e-30],
    [-1.0e3, 1.0e3, 0.5],
];

/// Scene temperatures the map is blended at, the illuminants' own among
/// them.
const CCTS: [f64; 6] = [1000.0, 2856.0, 4000.0, 5003.0, 6504.0, 50000.0];

/// Stops the look table is read at: none, Adobe's sort, past the cap
/// either way, and not a number.
const STOPS: [f32; 7] = [0.0, -0.64, 0.16, 3.0, -40.0, 40.0, f32::NAN];

/// A small profile with every tag the reader knows: two calibrations
/// with forward matrices, a map for each, an sRGB-encoded look table, a
/// tone curve, an offset, and its names.
pub fn synthetic() -> Vec<u8> {
    let matrix = [
        0.7, -0.2, -0.05, -0.4, 1.2, 0.2, -0.05, 0.15, 0.6, //
    ];
    let forward = [
        0.6, 0.25, 0.1, 0.25, 0.8, -0.05, 0.02, -0.15, 0.95, //
    ];
    let stepped = |hues: usize, sats: usize, vals: usize, k: f32| {
        let mut map = HueSatMap::identity(hues, sats, vals);
        for (i, e) in map.data.iter_mut().enumerate() {
            let t = i as f32 * k;
            *e = [t * 3.0 - 6.0, 1.0 + t * 0.1, 1.0 - t * 0.02];
        }
        map
    };
    let mut look = stepped(3, 2, 3, 0.25);
    look.encoding = Encoding::Srgb;
    Dcp {
        name: Some("Synthetic".into()),
        unique_camera_model: Some("No Camera".into()),
        copyright: Some("public domain".into()),
        calibration_signature: Some("none".into()),
        embed_policy: EmbedPolicy::NoRestrictions,
        primary: DcpCalibration {
            illuminant: 17,
            color_matrix: matrix,
            forward_matrix: Some(forward),
            hue_sat_map: Some(stepped(4, 2, 1, 0.5)),
        },
        secondary: Some(DcpCalibration {
            illuminant: 21,
            color_matrix: matrix,
            forward_matrix: Some(forward),
            hue_sat_map: Some(stepped(4, 2, 1, 0.3)),
        }),
        baseline_exposure_offset: Some(-0.5),
        look: Some(look),
        tone_curve: Some(vec![[0.0, 0.0], [0.25, 0.3], [1.0, 1.0]]),
    }
    .to_bytes()
}

/// `patch` laid over [`synthetic`]: its first two bytes, little endian,
/// say where (modulo the file's length), and the rest overwrite from
/// there, running past the end if they are longer. No bytes, no patch.
pub fn patched(patch: &[u8]) -> Vec<u8> {
    let mut bytes = synthetic();
    let Some((at, rest)) = patch.split_first_chunk::<2>() else {
        return bytes;
    };
    let at = usize::from(u16::from_le_bytes(*at)) % bytes.len();
    let end = at + rest.len();
    if end > bytes.len() {
        bytes.resize(end, 0);
    }
    bytes[at..end].copy_from_slice(rest);
    bytes
}

/// What the fuzz target asserts of `bytes`. Panics on a breach.
pub fn check(bytes: &[u8]) {
    let info = Dcp::info_from_bytes(bytes);
    let Ok(dcp) = Dcp::from_bytes(bytes) else {
        return;
    };
    // What reads in full reads its header, and says the same.
    let info = info.expect("a profile that reads in full reads its header");
    assert_eq!(info.name, dcp.name);
    assert_eq!(info.unique_camera_model, dcp.unique_camera_model);
    assert_eq!(info.copyright, dcp.copyright);
    assert_eq!(info.embed_policy, dcp.embed_policy);

    let maps = || {
        std::iter::once(&dcp.primary)
            .chain(dcp.secondary.as_ref())
            .filter_map(|c| c.hue_sat_map.as_ref())
    };
    for table in maps().chain(dcp.look.as_ref()) {
        table_holds(table);
    }
    if let Some(offset) = dcp.baseline_exposure_offset {
        assert!(
            offset.is_finite() && offset.abs() <= MAX_OFFSET_STOPS,
            "offset {offset}"
        );
    }
    if let Some(curve) = &dcp.tone_curve {
        assert!(curve.len() >= 2, "a tone curve of {} points", curve.len());
    }

    // What reads writes back as a file that reads the same.
    let back = Dcp::from_bytes(&dcp.to_bytes())
        .unwrap_or_else(|e| panic!("the profile does not read back: {e}"));
    assert_eq!(back.name, dcp.name);
    assert_eq!(back.unique_camera_model, dcp.unique_camera_model);
    assert_eq!(back.copyright, dcp.copyright);
    assert_eq!(back.calibration_signature, dcp.calibration_signature);
    assert_eq!(back.embed_policy, dcp.embed_policy);
    assert_eq!(back.look, dcp.look);
    let back_maps: Vec<_> = std::iter::once(&back.primary)
        .chain(back.secondary.as_ref())
        .filter_map(|c| c.hue_sat_map.as_ref())
        .collect();
    assert_eq!(back_maps, maps().collect::<Vec<_>>());
    match (back.baseline_exposure_offset, dcp.baseline_exposure_offset) {
        (Some(a), Some(b)) => assert!((a - b).abs() < 1e-3, "offset {b} read back as {a}"),
        (a, b) => assert_eq!(a, b, "the offset"),
    }

    // Then as the develop runs it.
    let Ok(profile) = dcp.profile() else {
        return;
    };
    if let Some(offset) = profile.baseline_exposure_offset {
        assert!(offset.is_finite() && offset.abs() <= MAX_OFFSET_STOPS);
    }
    // A white balance solved against the profile, as an as-shot one
    // is, and one asked for by temperature: the scene temperature the
    // map is blended at comes from one of the two.
    let mut ccts = CCTS.to_vec();
    if let Ok(wb) = WhiteBalance::from_coefficients(
        &profile.camera,
        Vec3::new(2.0, 1.0, 1.5),
        &WORKING_SPACE,
        CAT,
    ) {
        ccts.push(wb.temp_tint.cct);
    }
    if let Ok(wb) = WhiteBalance::from_temp_tint(
        &profile.camera,
        TempTint::new(5000.0, 0.0),
        &WORKING_SPACE,
        CAT,
    ) {
        ccts.push(wb.temp_tint.cct);
    }
    for cct in ccts {
        let map = profile
            .map_at_cct(cct)
            .unwrap_or_else(|e| panic!("the maps do not blend at {cct} K: {e}"));
        if let Some(map) = map {
            table_holds(&map);
            let stage = MapStage::new(map).expect("a map stage");
            stage_holds(&stage, &format!("the map at {cct} K"));
        }
    }
    if let Some(look) = &profile.look {
        for stops in STOPS {
            let stage = MapStage::look((**look).clone(), stops).expect("a look stage");
            stage_holds(&stage, &format!("the look table at {stops} stops"));
        }
    }
}

/// A table as [`Dcp::from_bytes`] promises one: the sizes it claims,
/// within the caps, every entry finite and within its bound.
fn table_holds(table: &HueSatMap) {
    let (h, s, v) = (
        table.hue_divisions,
        table.sat_divisions,
        table.val_divisions,
    );
    assert!(
        (1..=MAX_DIVISIONS).contains(&h)
            && (2..=MAX_DIVISIONS).contains(&s)
            && (1..=MAX_DIVISIONS).contains(&v),
        "a table of {h}x{s}x{v}"
    );
    assert_eq!(table.data.len(), h * s * v, "a table of {h}x{s}x{v}");
    assert!(
        table
            .data
            .iter()
            .flatten()
            .all(|e| e.is_finite() && e.abs() <= MAX_TABLE_ENTRY),
        "a table entry is past {MAX_TABLE_ENTRY}"
    );
}

fn stage_holds(stage: &MapStage, what: &str) {
    for px in PIXELS {
        let out = stage.pixel(px);
        assert!(
            out.iter().all(|c| c.is_finite()),
            "{what}: {px:?} became {out:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_synthetic_profile_reads_whole_and_holds() {
        let bytes = synthetic();
        let dcp = Dcp::from_bytes(&bytes).unwrap();
        assert!(dcp.secondary.is_some());
        assert!(dcp.primary.hue_sat_map.is_some());
        assert!(dcp.secondary.as_ref().unwrap().hue_sat_map.is_some());
        assert!(dcp.primary.forward_matrix.is_some());
        assert_eq!(dcp.look.as_ref().unwrap().encoding, Encoding::Srgb);
        assert_eq!(dcp.tone_curve.as_ref().unwrap().len(), 3);
        assert_eq!(dcp.calibration_signature.as_deref(), Some("none"));
        let profile = dcp.profile().unwrap();
        assert!(profile.camera.secondary.is_some());
        assert!(profile.maps.is_some() && profile.look.is_some());
        check(&bytes);
        // No patch, or one that changes nothing, is the profile itself.
        assert_eq!(patched(&[]), bytes);
        assert_eq!(patched(&[0, 0, b'I', b'I']), bytes);
    }

    /// The fuzz target's checks over mutated copies of the synthetic
    /// profile, for a machine without cargo-fuzz:
    /// `GREYCARD_DCP_FUZZ_SECS=600 cargo test -p greycard-core --lib
    /// mutated_profiles -- --ignored`. `GREYCARD_DCP_CORPUS` names a
    /// folder of more profiles to start from, and a failing input is
    /// written to `GREYCARD_DCP_ARTIFACTS` when that is set.
    #[test]
    #[ignore = "a fuzzing run, not a test"]
    fn mutated_profiles_hold() {
        let secs: u64 = std::env::var("GREYCARD_DCP_FUZZ_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let seed: u64 = std::env::var("GREYCARD_DCP_FUZZ_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut pool = vec![synthetic()];
        if let Ok(dir) = std::env::var("GREYCARD_DCP_CORPUS") {
            for entry in std::fs::read_dir(dir).unwrap() {
                pool.push(std::fs::read(entry.unwrap().path()).unwrap());
            }
        }
        let starts = pool.len();
        let mut rng = Rng(seed);
        let start = std::time::Instant::now();
        let (mut runs, mut reads, mut slowest) = (0u64, 0u64, std::time::Duration::ZERO);
        while start.elapsed().as_secs() < secs {
            let mut input = pool[rng.below(pool.len())].clone();
            for _ in 0..=rng.below(6) {
                mutate(&mut input, &mut rng);
            }
            // Every eighth run is a patch over the synthetic profile, as
            // the fuzz target reads half its inputs.
            if rng.below(8) == 0 {
                let len = rng.below(24);
                let patch: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
                input = patched(&patch);
            }
            let began = std::time::Instant::now();
            let held = std::panic::catch_unwind(|| check(&input));
            let took = began.elapsed();
            slowest = slowest.max(took);
            if held.is_err() || took.as_secs() >= 1 {
                if let Ok(dir) = std::env::var("GREYCARD_DCP_ARTIFACTS") {
                    std::fs::create_dir_all(&dir).unwrap();
                    let path = std::path::Path::new(&dir).join(format!("failed-{runs}"));
                    std::fs::write(&path, &input).unwrap();
                    eprintln!("wrote {}", path.display());
                }
                panic!(
                    "run {runs} failed ({took:?}) on {} bytes{}",
                    input.len(),
                    if input.len() <= 512 {
                        format!(": {input:?}")
                    } else {
                        String::new()
                    }
                );
            }
            runs += 1;
            // What still reads is kept to mutate further, in a pool of
            // bounded size: a cheap stand-in for coverage.
            if Dcp::from_bytes(&input).is_ok() {
                reads += 1;
                if pool.len() < starts + 512 {
                    pool.push(input);
                } else {
                    let at = starts + rng.below(512);
                    pool[at] = input;
                }
            }
        }
        println!(
            "{runs} runs in {:?}, {reads} read, slowest {slowest:?}, pool {}",
            start.elapsed(),
            pool.len()
        );
    }

    /// xorshift64*: enough for picking mutations.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
    }

    /// Four bytes worth trying: sizes, offsets, and floats at their
    /// edges.
    const INTERESTING: [u32; 24] = [
        0,
        1,
        2,
        3,
        0xFFFF,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFF,
        1024,
        1025,
        4_194_304,
        0x7F7F_FFFF, // f32::MAX
        0xFF7F_FFFF, // f32::MIN
        0x7F80_0000, // inf
        0x7FC0_0000, // NaN
        0x7149_F2CA, // 1e30
        0xF149_F2CA, // -1e30
        0x4974_2400, // 1e6
        0x4B18_9680, // 1e7
        0xC974_2400, // -1e6
        0x0000_0001, // the least subnormal
        0x501502F9,  // 1e10
        0x4120_0000, // 10
        0xC1A0_0000, // -20
    ];

    fn mutate(bytes: &mut Vec<u8>, rng: &mut Rng) {
        if bytes.is_empty() {
            bytes.extend((0..rng.below(64) + 1).map(|_| rng.next() as u8));
            return;
        }
        let len = bytes.len();
        // An entry's field more often than a byte anywhere: the
        // directory is where a profile's shape is.
        let pick = rng.next();
        let at_entry = (|| -> Option<usize> {
            if len < 10 || &bytes[0..2] != b"II" {
                return None;
            }
            let start = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
            let count =
                u16::from_le_bytes(bytes.get(start..start.checked_add(2)?)?.try_into().ok()?);
            let index = (pick % u64::from(count.max(1))) as usize;
            Some(start + 2 + 12 * index)
        })()
        .filter(|at| at + 12 <= len);
        let put = |bytes: &mut Vec<u8>, at: usize, value: &[u8]| {
            for (i, b) in value.iter().enumerate() {
                if let Some(slot) = bytes.get_mut(at + i) {
                    *slot = *b;
                }
            }
        };
        match rng.below(10) {
            0 => {
                let at = rng.below(len);
                bytes[at] ^= 1 << rng.below(8);
            }
            1 => {
                let at = rng.below(len);
                bytes[at] = rng.next() as u8;
            }
            2 | 3 => {
                let value = INTERESTING[rng.below(INTERESTING.len())].to_le_bytes();
                let at = rng.below(len) & !3;
                put(bytes, at, &value);
            }
            4..=6 => {
                let Some(at) = at_entry else {
                    return;
                };
                // The type, the count, or the offset.
                match rng.below(3) {
                    0 => put(bytes, at + 2, &(rng.below(14) as u16).to_le_bytes()),
                    1 => {
                        let value = if rng.below(2) == 0 {
                            INTERESTING[rng.below(INTERESTING.len())]
                        } else {
                            rng.below(len * 2) as u32
                        };
                        put(bytes, at + 4, &value.to_le_bytes());
                    }
                    _ => put(bytes, at + 8, &(rng.below(len + 16) as u32).to_le_bytes()),
                }
            }
            7 => bytes.truncate(rng.below(len)),
            8 => {
                let n = rng.below(64) + 1;
                bytes.extend((0..n).map(|_| rng.next() as u8));
            }
            _ => {
                let from = rng.below(len);
                let to = rng.below(len);
                let n = rng.below(64).min(len - from).min(len - to);
                bytes.copy_within(from..from + n, to);
            }
        }
    }
}
