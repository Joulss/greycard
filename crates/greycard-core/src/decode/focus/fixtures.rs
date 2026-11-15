//! Small raws built for the tests: a TIFF raw of each maker read with
//! nothing in it but the IFDs and the maker note the focus point needs,
//! and a RAF of its header, a JPEG of an EXIF segment, and its own
//! metadata. No image data. Also the fuzz target's seeds
//! (`write_fuzz_seeds`).

use std::io::Cursor;

use super::{focus_head, focus_in_head, focus_stream};
use crate::raw::Orientation;

/// One IFD entry: tag, type, count, and the value's little-endian
/// bytes.
type Entry = (u16, u16, u32, Vec<u8>);

fn short(tag: u16, v: &[u16]) -> Entry {
    (
        tag,
        3,
        v.len() as u32,
        v.iter().flat_map(|n| n.to_le_bytes()).collect(),
    )
}

fn long(tag: u16, v: &[u32]) -> Entry {
    (
        tag,
        4,
        v.len() as u32,
        v.iter().flat_map(|n| n.to_le_bytes()).collect(),
    )
}

fn undef(tag: u16, b: Vec<u8>) -> Entry {
    (tag, 7, b.len() as u32, b)
}

fn ascii(tag: u16, s: &str) -> Entry {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    (tag, 2, b.len() as u32, b)
}

fn rational(tag: u16, typ: u16, v: &[(u32, u32)]) -> Entry {
    let b = v
        .iter()
        .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
        .collect();
    (tag, typ, v.len() as u32, b)
}

fn ifd_len(entries: &[Entry]) -> usize {
    2 + 12 * entries.len()
        + 4
        + entries
            .iter()
            .map(|e| e.3.len())
            .filter(|&n| n > 4)
            .sum::<usize>()
}

/// An IFD placed at `at`, its values after it, offsets counted from
/// `base`.
fn ifd_at(entries: &[Entry], at: usize, base: usize) -> Vec<u8> {
    let mut data_at = at + 2 + 12 * entries.len() + 4;
    let mut out = (entries.len() as u16).to_le_bytes().to_vec();
    let mut data: Vec<u8> = Vec::new();
    for (tag, typ, count, bytes) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(typ.to_le_bytes());
        out.extend(count.to_le_bytes());
        if bytes.len() <= 4 {
            let mut inline = bytes.clone();
            inline.resize(4, 0);
            out.extend(inline);
        } else {
            out.extend(((data_at - base) as u32).to_le_bytes());
            data_at += bytes.len();
            data.extend(bytes);
        }
    }
    out.extend(0u32.to_le_bytes());
    out.extend(data);
    out
}

/// A little-endian TIFF raw: IFD0 with Make, Orientation, the Exif
/// pointer, and `ifd0` besides; an Exif IFD with `exif` (inline values
/// only) and the maker note `note(at)` built for its place `at` last.
fn tiff_raw(
    magic: u16,
    make: &str,
    orientation: u16,
    ifd0: Vec<Entry>,
    exif: Vec<Entry>,
    note: impl Fn(usize) -> Vec<u8>,
) -> Vec<u8> {
    assert!(exif.iter().all(|e| e.3.len() <= 4));
    let mut first = vec![
        ascii(0x010f, make),
        short(0x0112, &[orientation]),
        long(0x8769, &[0]),
    ];
    first.extend(ifd0);
    let exif_at = 8 + ifd_len(&first);
    first[2] = long(0x8769, &[exif_at as u32]);
    let note_at = exif_at + 2 + 12 * (exif.len() + 1) + 4;
    let mut exif = exif;
    exif.push(undef(0x927c, note(note_at)));
    let mut out = b"II".to_vec();
    out.extend(magic.to_le_bytes());
    out.extend(8u32.to_le_bytes());
    out.extend(ifd_at(&first, 8, 0));
    assert_eq!(out.len(), exif_at);
    out.extend(ifd_at(&exif, exif_at, 0));
    out
}

/// A JPEG of SOI, an EXIF segment holding `tiff`, and EOI.
fn jpeg_of(tiff: &[u8]) -> Vec<u8> {
    let mut out = vec![0xff, 0xd8, 0xff, 0xe1];
    out.extend(((tiff.len() + 8) as u16).to_be_bytes());
    out.extend(b"Exif\0\0");
    out.extend(tiff);
    out.extend([0xff, 0xd9]);
    out
}

/// A Z6III's NEF: "Nikon\0", version 2.10, a TIFF of its own with
/// FocusMode, CropHiSpeed, and AFInfo2 version 0401.
pub(super) fn nef(mode: &str, x: u16, y: u16) -> Vec<u8> {
    let mut af = vec![0u8; 80];
    af[..4].copy_from_slice(b"0401");
    af[4] = 2;
    af[5] = 207;
    af[7] = 1;
    for (k, v) in [6048u16, 4032, x, y, 270, 268].into_iter().enumerate() {
        af[0x3e + 2 * k..0x40 + 2 * k].copy_from_slice(&v.to_le_bytes());
    }
    af[0x4a] = 1;
    let entries = vec![
        ascii(0x0007, mode),
        short(0x001b, &[11, 6064, 4040, 6064, 4040, 0, 0]),
        undef(0x00b7, af),
    ];
    let mut note = b"Nikon\0\x02\x10\0\0II*\0".to_vec();
    note.extend(8u32.to_le_bytes());
    note.extend(ifd_at(&entries, 8, 0));
    tiff_raw(42, "NIKON CORPORATION", 1, vec![], vec![], move |_| {
        note.clone()
    })
}

/// An ARW: a bare note IFD, offsets from the file's start.
pub(super) fn arw(focus_mode: u8, x: u16, y: u16) -> Vec<u8> {
    tiff_raw(42, "SONY", 8, vec![], vec![], move |at| {
        let entries = vec![
            (0x201b, 1, 1, vec![focus_mode]),
            short(0x2027, &[7008, 4672, x, y]),
            (
                0x2037,
                7,
                6,
                [153u16, 154, 1]
                    .iter()
                    .flat_map(|n| n.to_le_bytes())
                    .collect(),
            ),
        ];
        ifd_at(&entries, at, 0)
    })
}

/// An RW2: IFD0 with the sensor's size and a JPEG whose EXIF holds the
/// note, "Panasonic" and three NULs, offsets from that EXIF's header.
pub(super) fn rw2(focus_mode: u16, x: (u32, u32), y: (u32, u32)) -> Vec<u8> {
    let inner = tiff_raw(42, "Panasonic", 1, vec![], vec![], move |at| {
        let entries = vec![
            short(0x0007, &[focus_mode]),
            long(0x004b, &[6000]),
            long(0x004c, &[4000]),
            rational(0x004d, 5, &[x, y]),
        ];
        let mut note = b"Panasonic\0\0\0".to_vec();
        note.extend(ifd_at(&entries, at + 12, 0));
        note
    });
    let jpeg = jpeg_of(&inner);
    // IFD0's JpgFromRaw sits after IFD0 and an empty Exif IFD; the
    // maker note there is the JPEG's.
    let ifd0 = vec![
        ascii(0x010f, "Panasonic"),
        short(0x0002, &[6008]),
        short(0x0003, &[4008]),
        short(0x0112, &[8]),
        undef(0x002e, jpeg),
    ];
    let mut out = b"IIU\0".to_vec();
    out.extend(8u32.to_le_bytes());
    out.extend(ifd_at(&ifd0, 8, 0));
    out
}

/// An OM System ORF: "OM SYSTEM", the byte order and a version, then
/// the main IFD with Camera Settings and Image Processing, offsets
/// from the note's start.
pub(super) fn orf(focus_mode: u16, selected: [(i32, i32); 4], aspect: [u16; 4]) -> Vec<u8> {
    let mut sel = vec![(0i32, 0i32)];
    sel.extend(selected);
    let sel: Vec<(u32, u32)> = sel.iter().map(|&(n, d)| (n as u32, d as u32)).collect();
    let cs = vec![
        short(0x0301, &[focus_mode, 0x41]),
        short(0x0302, &[1]),
        short(0x0303, &[1]),
        long(0x0304, &[1668773512, 0, 0, 0]),
        rational(0x0305, 10, &sel),
    ];
    let ip = vec![
        long(0x0614, &[5184]),
        long(0x0615, &[3888]),
        short(0x1113, &aspect),
    ];
    let main_len = 2 + 12 * 2 + 4;
    let cs_at = 16 + main_len;
    let ip_at = cs_at + ifd_len(&cs);
    let main = vec![
        (0x2020, 13, 1, (cs_at as u32).to_le_bytes().to_vec()),
        (0x2040, 13, 1, (ip_at as u32).to_le_bytes().to_vec()),
    ];
    let mut note = b"OM SYSTEM\0\0\0II\x03\0".to_vec();
    note.extend(ifd_at(&main, 16, 0));
    note.extend(ifd_at(&cs, cs_at, 0));
    note.extend(ifd_at(&ip, ip_at, 0));
    tiff_raw(
        0x4f52,
        "OM Digital Solutions",
        8,
        vec![],
        vec![],
        move |_| note.clone(),
    )
}

/// A RAF: the header, a JPEG whose EXIF holds the picture's size and the
/// "FUJIFILM" note, and the RAF's metadata with the cropped size.
pub(super) fn raf(focus_mode: u16, pixel: [u16; 2], cropped: [u16; 2]) -> Vec<u8> {
    let entries = vec![
        short(0x1021, &[focus_mode]),
        short(0x1023, &pixel),
        short(0x104d, &[0]),
        short(0x1301, &[0]),
        // A head, then a left eye.
        (0x4201, 1, 2, vec![8, 2]),
        short(0x4203, &[2287, 1324, 3104, 2003, 2474, 1299, 2554, 1379]),
    ];
    let mut note = b"FUJIFILM".to_vec();
    note.extend(12u32.to_le_bytes());
    note.extend(ifd_at(&entries, 12, 0));
    let exif = vec![long(0xa002, &[4000]), long(0xa003, &[3000])];
    let inner = tiff_raw(42, "FUJIFILM", 6, vec![], exif, move |_| note.clone());
    let jpeg = jpeg_of(&inner);
    let mut meta = 2u32.to_be_bytes().to_vec();
    meta.extend([0x01, 0x00, 0x00, 0x04, 0x22, 0x80, 0x2e, 0x80]);
    meta.extend([0x01, 0x11, 0x00, 0x04]);
    meta.extend(cropped[1].to_be_bytes());
    meta.extend(cropped[0].to_be_bytes());
    let mut out = b"FUJIFILMCCD-RAW 0201FF383501".to_vec();
    out.resize(0x64, 0);
    let jpeg_at = 0x64u32;
    let meta_at = jpeg_at + jpeg.len() as u32;
    for (at, v) in [
        (0x54, jpeg_at),
        (0x58, jpeg.len() as u32),
        (0x5c, meta_at),
        (0x60, meta.len() as u32),
    ] {
        out[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }
    out.extend(jpeg);
    out.extend(meta);
    out
}

fn center(file: &[u8]) -> Option<((f32, f32), Orientation)> {
    let (p, o) = focus_head(Cursor::new(file)).unwrap()?;
    let [x, y, w, h] = p.area;
    Some(((x + w / 2.0, y + h / 2.0), o))
}

fn near(a: (f32, f32), b: (f64, f64)) -> bool {
    (f64::from(a.0) - b.0).abs() < 1e-4 && (f64::from(a.1) - b.1).abs() < 1e-4
}

#[test]
fn a_nef_is_read_to_its_point() {
    let ((x, y), o) = center(&nef("AF-C  ", 4158, 1103)).unwrap();
    assert!(near((x, y), (4158.0 / 6048.0, 1103.0 / 4032.0)), "{x} {y}");
    assert_eq!(o, Orientation::Normal);
    assert_eq!(center(&nef("MF", 4158, 1103)), None);
    // The decode's crop of the AF image's shape takes it; another shape
    // does not.
    let file = nef("AF-S", 4158, 1103);
    assert!(
        focus_stream(Cursor::new(&file), (6048, 4032))
            .unwrap()
            .is_some()
    );
    assert!(
        focus_stream(Cursor::new(&file), (6000, 4000))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        focus_stream(Cursor::new(&file), (4032, 4032)).unwrap(),
        None
    );
}

#[test]
fn an_arw_is_read_to_its_point() {
    let ((x, y), o) = center(&arw(3, 4982, 2735)).unwrap();
    assert!(near((x, y), (4982.0 / 7008.0, 2735.0 / 4672.0)), "{x} {y}");
    assert_eq!(o, Orientation::Rotate270);
    assert_eq!(center(&arw(0, 4982, 2735)), None);
    assert_eq!(center(&arw(3, 3504, 2336)).map(|c| c.0), Some((0.5, 0.5)));
}

#[test]
fn an_rw2_is_read_through_its_jpeg() {
    let ((x, y), o) = center(&rw2(5, (774, 1024), (384, 1024))).unwrap();
    assert!(near((x, y), (774.0 / 1024.0, 0.375)), "{x} {y}");
    assert_eq!(o, Orientation::Rotate270);
    assert_eq!(center(&rw2(2, (774, 1024), (384, 1024))), None);
    assert_eq!(center(&rw2(5, (16777216, 1), (16777216, 1))), None);
}

#[test]
fn an_orf_is_read_to_its_area() {
    let at = [(273, 640), (240, 480), (273, 640), (240, 480)];
    let full = [0, 0, 5183, 3887];
    let ((x, y), _) = center(&orf(0, at, full)).unwrap();
    // The AF area (99, 119) to (118, 136) in 255ths holds the point.
    assert!(near((x, y), (108.5 / 255.0, 127.5 / 255.0)), "{x} {y}");
    assert_eq!(center(&orf(10, at, full)), None);
    assert_eq!(center(&orf(0, at, [0, 486, 5183, 3401])), None);
}

#[test]
fn a_raf_is_read_to_its_eye() {
    let file = raf(0, [2491, 1337], [11648, 8736]);
    let ((x, y), o) = center(&file).unwrap();
    // The eye box holding the pixel.
    assert!(near((x, y), (2514.0 / 4000.0, 1339.0 / 3000.0)), "{x} {y}");
    assert_eq!(o, Orientation::Rotate90);
    assert_eq!(center(&raf(1, [2491, 1337], [11648, 8736])), None);
    // A JPEG of another shape than the raw's cropped size.
    assert_eq!(center(&raf(0, [2491, 1337], [11648, 6552])), None);
    // The head read alone, the metadata past it: read the file.
    let head = file[..file.len() - 8].to_vec();
    assert!(focus_in_head(head, file.len() as u64).is_none());
}

/// Every built file cut short anywhere is an answer or an error, never
/// a panic; a point read from a piece lies on the picture.
#[test]
fn built_raws_cut_short_never_panic() {
    let files = [
        nef("AF-C", 4158, 1103),
        arw(3, 4982, 2735),
        rw2(5, (774, 1024), (384, 1024)),
        orf(
            0,
            [(273, 640), (240, 480), (273, 640), (240, 480)],
            [0, 0, 5183, 3887],
        ),
        raf(0, [2491, 1337], [11648, 8736]),
    ];
    for file in files {
        for n in 0..file.len() {
            if let Ok(Some((p, _))) = focus_head(Cursor::new(&file[..n])) {
                assert!(p.area.iter().all(|v| v.is_finite()));
            }
        }
    }
}

/// Writes the built raws as the fuzz target's seeds into the folder
/// `GREYCARD_FOCUS_SEEDS` names.
#[test]
#[ignore]
fn write_fuzz_seeds() {
    let Some(dir) = std::env::var_os("GREYCARD_FOCUS_SEEDS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for (name, file) in [
        ("seed-nef-z", nef("AF-C", 4158, 1103)),
        ("seed-arw", arw(3, 4982, 2735)),
        ("seed-rw2", rw2(5, (774, 1024), (384, 1024))),
        (
            "seed-orf",
            orf(
                0,
                [(273, 640), (240, 480), (273, 640), (240, 480)],
                [0, 0, 5183, 3887],
            ),
        ),
        ("seed-raf", raf(0, [2491, 1337], [11648, 8736])),
    ] {
        std::fs::write(dir.join(name), file).unwrap();
    }
}

/// A spot off the picture, larger than it, or of no size once single
/// precision is none; a crop of the maker's shape scales it.
#[test]
fn a_spot_is_held_to_the_picture() {
    use super::{Spot, spot_focus};
    let of = (6000.0, 4000.0);
    let on = Spot::at(3000.0, 2000.0, 100.0, 100.0, of).unwrap();
    assert!(spot_focus(&on, None).is_some());
    assert!(spot_focus(&on, Some((3000, 2000))).is_some());
    assert_eq!(spot_focus(&on, Some((4000, 4000))), None);
    let off = Spot::at(6100.0, 2000.0, 100.0, 100.0, of).unwrap();
    assert_eq!(spot_focus(&off, None), None);
    let big = Spot::at(3000.0, 2000.0, 7000.0, 100.0, of).unwrap();
    assert_eq!(spot_focus(&big, None), None);
    let tiny = Spot::at(3000.0, 2000.0, 1e-300, 1e-300, of).unwrap();
    assert_eq!(spot_focus(&tiny, None), None);
    assert_eq!(Spot::at(1.0, 1.0, 1.0, 1.0, (0.0, 4000.0)), None);
}

/// A RAF whose header gives its JPEG 0 or 1 bytes, and an RW2 whose
/// JpgFromRaw entry counts 0 or 1, have no point and do not panic: the
/// JPEG head reader once indexed two bytes it did not have.
#[test]
fn a_jpeg_of_under_two_bytes_is_no_point() {
    for len in [0u32, 1] {
        let mut file = raf(0, [2491, 1337], [11648, 8736]);
        file[0x58..0x5c].copy_from_slice(&len.to_be_bytes());
        assert_eq!(focus_head(Cursor::new(&file)).unwrap(), None, "raf {len}");
        let mut file = rw2(5, (774, 1024), (384, 1024));
        // IFD0 at 8; JpgFromRaw is its fifth entry, its count at 4.
        let entry = 8 + 2 + 12 * 4;
        assert_eq!(&file[entry..entry + 2], &0x002eu16.to_le_bytes());
        file[entry + 4..entry + 8].copy_from_slice(&len.to_le_bytes());
        assert!(
            focus_head(Cursor::new(&file)).is_ok_and(|p| p.is_none()),
            "rw2 {len}"
        );
    }
}

/// The fuzz target's check on stable: each seed mutated a few thousand
/// times (bits flipped, bytes and words overwritten with edge values,
/// cut short, grown) by a fixed generator. Never a panic, and a point
/// found lies on the picture.
#[test]
fn mutated_seeds_never_panic_or_leave_the_picture() {
    let cr3: &[u8] = include_bytes!("../../../../../fuzz/corpus/focus_point/seed-cr3-two-in-focus");
    let seeds = [
        cr3.to_vec(),
        nef("AF-C", 4158, 1103),
        arw(3, 4982, 2735),
        rw2(5, (774, 1024), (384, 1024)),
        orf(
            0,
            [(273, 640), (240, 480), (273, 640), (240, 480)],
            [0, 0, 5183, 3887],
        ),
        raf(0, [2491, 1337], [11648, 8736]),
    ];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    const EDGES: [u32; 9] = [
        0,
        1,
        2,
        8,
        0x10,
        0xfffe,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_ffff,
    ];
    for seed in &seeds {
        for _ in 0..3000 {
            let mut d = seed.clone();
            for _ in 0..1 + next() % 4 {
                let at = next() as usize % d.len();
                match next() % 6 {
                    0 => d[at] ^= 1 << (next() % 8),
                    1 => d[at] = next() as u8,
                    2 => {
                        let v = EDGES[(next() % 9) as usize];
                        let b = match next() % 2 {
                            0 => v.to_le_bytes(),
                            _ => v.to_be_bytes(),
                        };
                        for (k, byte) in b.into_iter().enumerate() {
                            if let Some(slot) = d.get_mut(at + k) {
                                *slot = byte;
                            }
                        }
                    }
                    3 => d.truncate((next() as usize % d.len()).max(1)),
                    4 => {
                        for _ in 0..next() % 64 {
                            d.push(next() as u8);
                        }
                    }
                    _ => {
                        let v = [0u16, 1, 2, 0xffff, 0x8000][(next() % 5) as usize];
                        if at + 1 < d.len() {
                            d[at..at + 2].copy_from_slice(&v.to_le_bytes());
                        }
                    }
                }
            }
            if let Ok(Some((p, _))) = focus_head(Cursor::new(&d)) {
                let [x, y, w, h] = p.area;
                assert!(p.area.iter().all(|v| v.is_finite()), "{:?}", p.area);
                assert!(w > 0.0 && h > 0.0 && w <= 1.0 && h <= 1.0, "{:?}", p.area);
                let on = -1e-4..=1.0 + 1e-4;
                assert!(on.contains(&(x + w / 2.0)) && on.contains(&(y + h / 2.0)));
            }
        }
    }
}
