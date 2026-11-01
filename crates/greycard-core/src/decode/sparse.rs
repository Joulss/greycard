//! A raw read only where its metadata is.
//!
//! The library's index wants a raw's camera, lens, exposure, date, kind
//! of mosaic, and maker's style, which sit in a few hundred KB at most
//! of a file of tens of MB. rawler reads from one buffer the file's
//! length, so the index gives it one: zeros, with the pieces the
//! format's own structure says the metadata is in read into place. A
//! [`Sparse`] is that buffer and the file behind it; read through as a
//! stream, it fetches what is asked of it, so a reader that walks the
//! file itself (the maker's style, the CR3's boxes) is given the file's
//! own bytes and never a zero it did not ask for.
//!
//! What each format's pieces are:
//!
//! | Format | Pieces |
//! |---|---|
//! | All | the first 64 KB, the bytes the content hash reads |
//! | CR3 | the boxes' headers, the `moov` box, and the CTMD record its table points at |
//! | TIFF (NEF, ARW, DNG, ORF, RW2) | every IFD, every value stored outside its entry up to 256 KB, the maker note whole |
//! | RAF | its header, the JPEG's EXIF as a TIFF, the RAF's own directory, and the TIFF at its CFA pointer |
//!
//! Never read: image data (strips, tiles, the samples a CR3's `mdat`
//! holds), the previews pointed at, and values over 256 KB (a DNG's
//! copy of the original raw and its look table, an RW2's JPEG). Read
//! all the same: a preview inside a maker note, as an ORF's is, since
//! the maker note is read whole.

use std::io::{Read, Seek, SeekFrom};

use ::rawler::formats::bmff::FileBox;

/// The blocks a [`Sparse`] reads in, so neighboring small reads (an
/// IFD and the values just past it) cost the file one read.
const BLOCK: u64 = 4096;

/// How much of the file's head is read first: what the content hash
/// reads, so a pass that hashes the file pays for these bytes once.
pub(crate) const HEAD: u64 = 64 * 1024;

/// The most IFDs a TIFF walk visits, and the most entries one may
/// hold: a file whose directories point at each other, or a count of
/// garbage, ends the walk instead of the read.
const MAX_IFDS: usize = 512;
const MAX_ENTRIES: u64 = 4096;

/// The largest value a walk reads: a maker note with a preview inside
/// it (Olympus's) is about a MB.
const MAX_VALUE: u64 = 16 << 20;

/// A file's length in zeros, with the pieces read so far in place.
pub(crate) struct Sparse<R> {
    file: R,
    len: u64,
    buf: Vec<u8>,
    /// Which [`BLOCK`]s are in `buf`.
    have: Vec<bool>,
    pos: u64,
    /// Bytes read from the file.
    read: u64,
}

impl<R: Read + Seek> Sparse<R> {
    /// A sparse copy of `file`, `len` bytes long, nothing read yet.
    /// None for a file too long to hold as one buffer.
    pub(crate) fn new(file: R, len: u64) -> Option<Self> {
        let size = usize::try_from(len).ok()?;
        let blocks = usize::try_from(len.div_ceil(BLOCK)).ok()?;
        Some(Sparse {
            file,
            len,
            // Zeroed by the allocator: pages never read are never
            // touched.
            buf: vec![0; size],
            have: vec![false; blocks],
            pos: 0,
            read: 0,
        })
    }

    /// Read `n` bytes at `at` into place, as far as the file goes, in
    /// whole blocks, each run of blocks not yet read in one read.
    pub(crate) fn fetch(&mut self, at: u64, n: u64) -> std::io::Result<()> {
        let end = at.saturating_add(n).min(self.len);
        if at >= end {
            return Ok(());
        }
        let (first, last) = (at / BLOCK, (end - 1) / BLOCK);
        let mut block = first;
        while block <= last {
            if self.have[block as usize] {
                block += 1;
                continue;
            }
            let run = block;
            while block <= last && !self.have[block as usize] {
                block += 1;
            }
            let from = run * BLOCK;
            let to = (block * BLOCK).min(self.len);
            self.file.seek(SeekFrom::Start(from))?;
            self.file
                .read_exact(&mut self.buf[from as usize..to as usize])?;
            self.read += to - from;
            self.have[run as usize..block as usize].fill(true);
        }
        Ok(())
    }

    /// `n` bytes at `at`, read first if they are not in place; an error
    /// for bytes past the end.
    pub(crate) fn piece(&mut self, at: u64, n: u64) -> std::io::Result<&[u8]> {
        let end = at
            .checked_add(n)
            .filter(|&end| end <= self.len)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;
        self.fetch(at, n)?;
        Ok(&self.buf[at as usize..end as usize])
    }

    /// The buffer and the bytes read from the file into it.
    pub(crate) fn into_parts(self) -> (Vec<u8>, u64) {
        (self.buf, self.read)
    }

    /// The buffer with every block not read set to `byte`: a reader
    /// whose answer changes with it looked at bytes never read.
    #[cfg(test)]
    pub(crate) fn filled(&self, byte: u8) -> Vec<u8> {
        let mut buf = self.buf.clone();
        for (block, have) in self.have.iter().enumerate() {
            if !have {
                let from = block * BLOCK as usize;
                let to = (from + BLOCK as usize).min(buf.len());
                buf[from..to].fill(byte);
            }
        }
        buf
    }

    /// The buffer, to change what was read in a test.
    #[cfg(test)]
    pub(crate) fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl<R: Read + Seek> Read for Sparse<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = (out.len() as u64).min(self.len.saturating_sub(self.pos));
        if n == 0 {
            return Ok(0);
        }
        let at = self.pos;
        out[..n as usize].copy_from_slice(self.piece(at, n)?);
        self.pos += n;
        Ok(n as usize)
    }
}

impl<R: Read + Seek> Seek for Sparse<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        }
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        self.pos = pos;
        Ok(pos)
    }
}

/// Read into `sparse` the pieces of the file a probe and its image's
/// description are in, by the format its head says it is; false for a
/// format with no such pieces known, which is read whole.
pub(crate) fn fetch_metadata<R: Read + Seek>(sparse: &mut Sparse<R>) -> std::io::Result<bool> {
    sparse.fetch(0, HEAD)?;
    let head: [u8; 16] = match sparse.piece(0, 16) {
        Ok(b) => b.try_into().expect("16 bytes"),
        Err(_) => return Ok(false),
    };
    if &head[4..8] == b"ftyp" && &head[8..12] == b"crx " {
        return cr3(sparse);
    }
    if head.starts_with(super::rawler_backend::RAF_MAGIC) {
        return raf(sparse);
    }
    if head.starts_with(b"II") || head.starts_with(b"MM") {
        return Ok(tiff(sparse, 0, &[])?.is_some());
    }
    Ok(false)
}

/// A CR3's boxes, walked by rawler's own parser through the sparse
/// file so it reads the headers and the `moov` box it wants, and then
/// the CTMD record (the exposure and the focal length) where the CTMD
/// track's table says the first sample is.
fn cr3<R: Read + Seek>(sparse: &mut Sparse<R>) -> std::io::Result<bool> {
    let filebox = FileBox::parse(&mut *sparse).map_err(std::io::Error::other)?;
    let ctmd = filebox
        .moov
        .traks
        .iter()
        .find(|trak| trak.mdia.minf.stbl.stsd.ctmd.is_some())
        .and_then(|trak| trak.mdia.minf.stbl.get_sample_offset(1));
    if let Some((at, n)) = ctmd {
        sparse.fetch(at as u64, n as u64)?;
    }
    Ok(true)
}

/// A RAF: the header's pointers, the embedded JPEG's EXIF (a TIFF
/// twelve bytes into the JPEG), the RAF's directory, and the TIFF at
/// the CFA pointer with its Fujifilm sub-IFD; where that pointer holds
/// no TIFF it points at the samples, which are not read.
fn raf<R: Read + Seek>(sparse: &mut Sparse<R>) -> std::io::Result<bool> {
    let header = sparse.piece(0, 108)?.to_vec();
    let be = |at: usize| u64::from(u32::from_be_bytes(header[at..at + 4].try_into().unwrap()));
    let (jpeg, directory, directory_len, cfa) = (be(84), be(92), be(96), be(100));
    if tiff(sparse, jpeg + 12, &[])?.is_none() {
        return Ok(false);
    }
    sparse.fetch(directory, directory_len.min(MAX_VALUE))?;
    // The Fujifilm IFD rawler follows from the CFA's TIFF.
    const FUJI_IFD: u16 = 0xf000;
    tiff(sparse, cfa, &[FUJI_IFD])?;
    Ok(true)
}

/// Tags whose entries point at more IFDs.
const SUB_IFDS: [u16; 4] = [
    0x014a, // SubIFDs
    0x8769, // Exif
    0x8825, // GPS
    0xa005, // Interoperability
];

/// Sony's SR2Private: in an ARW, four bytes that point at an IFD; in
/// a DNG, Adobe's private data, a value longer than that.
const SR2_PRIVATE: u16 = 0xc634;

/// In SR2Private's IFD, where the encrypted block with the white
/// balance and the levels is, and how long it is.
const SONY_OFFSET: u16 = 0x7200;
const SONY_LENGTH: u16 = 0x7201;

/// Values longer than this are a file's or an image's, or tables a
/// probe has no use for, and are never read: a DNG's copy of the
/// original raw and its profile's look table, an RW2's JPEG and its
/// second image. The maker note and Adobe's private data (a DNG's
/// copy of the maker note) are read whatever their length.
const BULK: u64 = 256 * 1024;

/// The maker note's tag.
const MAKER_NOTE: u16 = 0x927c;

/// The most pointers to IFDs a walk holds at once, and the most it
/// takes from one entry.
const MAX_QUEUED: usize = MAX_IFDS;
const MAX_POINTERS: usize = 64;

/// How a TIFF walk went: the IFDs it read, and the most it held to
/// read at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Walked {
    pub(crate) ifds: usize,
    pub(crate) most_queued: usize,
}

/// The TIFF whose header is at `base`, its offsets counted from there:
/// every IFD (the chain, the sub-IFDs, `subs`), and every value stored
/// outside its entry but a [`BULK`] one, to [`MAX_IFDS`] IFDs. None
/// when `base` holds no TIFF header. A value is only fetched into
/// place, never copied out, but for the pointers the walk follows.
fn tiff<R: Read + Seek>(
    sparse: &mut Sparse<R>,
    base: u64,
    subs: &[u16],
) -> std::io::Result<Option<Walked>> {
    let Ok(header) = sparse.piece(base, 8) else {
        return Ok(None);
    };
    let little = match &header[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Ok(None),
    };
    let u16_of = |b: &[u8]| {
        let b = [b[0], b[1]];
        if little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    let u32_of = |b: &[u8]| {
        let b = [b[0], b[1], b[2], b[3]];
        u64::from(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let mut queue = vec![u32_of(&header[4..8])];
    let mut walked = Walked {
        ifds: 0,
        most_queued: 1,
    };
    let mut seen = std::collections::HashSet::new();
    while let Some(offset) = queue.pop() {
        if walked.ifds >= MAX_IFDS {
            break;
        }
        if offset == 0 || !seen.insert(offset) {
            continue;
        }
        let at = base + offset;
        let Ok(count) = sparse.piece(at, 2).map(u16_of) else {
            continue;
        };
        let count = u64::from(count);
        if count == 0 || count > MAX_ENTRIES {
            continue;
        }
        let Ok(entries) = sparse.piece(at + 2, count * 12 + 4).map(<[u8]>::to_vec) else {
            continue;
        };
        walked.ifds += 1;
        let mut pointers = Vec::new();
        let (mut sony_offset, mut sony_length) = (None, None);
        for entry in entries[..count as usize * 12].as_chunks::<12>().0 {
            let (tag, kind) = (u16_of(&entry[0..2]), u16_of(&entry[2..4]));
            let n = u32_of(&entry[4..8]);
            let Some(size) = type_size(kind).and_then(|s| s.checked_mul(n)) else {
                continue;
            };
            let pointer = matches!(kind, 4 | 13);
            let follows = pointer && (SUB_IFDS.contains(&tag) || subs.contains(&tag) || kind == 13);
            let wanted =
                follows || (size == 4 && matches!(tag, SR2_PRIVATE | SONY_OFFSET | SONY_LENGTH));
            let inline = size <= 4;
            let value_at = base + u32_of(&entry[8..12]);
            if !inline {
                let bulk = size > BULK && !matches!(tag, MAKER_NOTE | SR2_PRIVATE);
                if bulk || size > MAX_VALUE || sparse.fetch(value_at, size).is_err() {
                    continue;
                }
            }
            if !wanted {
                continue;
            }
            // Pointers only: a follow's offsets, at most so many.
            let take = size.min(4 * MAX_POINTERS as u64);
            let value = if inline {
                entry[8..12].to_vec()
            } else {
                match sparse.piece(value_at, take) {
                    Ok(v) => v.to_vec(),
                    Err(_) => continue,
                }
            };
            if follows {
                pointers.extend(value.as_chunks::<4>().0.iter().map(|v| u32_of(v)));
            }
            match tag {
                SR2_PRIVATE => pointers.push(u32_of(&value)),
                SONY_OFFSET => sony_offset = Some(u32_of(&value)),
                SONY_LENGTH => sony_length = Some(u32_of(&value)),
                _ => {}
            }
        }
        if let (Some(at), Some(n)) = (sony_offset, sony_length) {
            sparse.fetch(base + at, n.min(MAX_VALUE))?;
        }
        pointers.push(u32_of(&entries[count as usize * 12..]));
        for p in pointers {
            if queue.len() >= MAX_QUEUED {
                break;
            }
            queue.push(p);
        }
        walked.most_queued = walked.most_queued.max(queue.len());
    }
    Ok(Some(walked))
}

/// The bytes a TIFF type's value takes, by its number.
fn type_size(kind: u16) -> Option<u64> {
    Some(match kind {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        16..=18 => 8,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A file of `len` bytes, each its offset's low byte plus one, so
    /// a zero is a byte never read.
    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 255) as u8 + 1).collect()
    }

    #[test]
    fn a_sparse_file_reads_whole_blocks_once_and_streams_the_files_bytes() {
        let file = pattern(5 * BLOCK as usize + 100);
        let mut sparse = Sparse::new(Cursor::new(file.clone()), file.len() as u64).unwrap();
        sparse.fetch(10, 20).unwrap();
        sparse.fetch(BLOCK - 1, 2).unwrap();
        assert_eq!(sparse.read, 2 * BLOCK);
        // Past the end: as far as the file goes.
        sparse.fetch(5 * BLOCK, 1000).unwrap();
        assert_eq!(sparse.read, 2 * BLOCK + 100);
        assert!(sparse.piece(5 * BLOCK + 90, 20).is_err());
        sparse.seek(SeekFrom::Start(3 * BLOCK - 3)).unwrap();
        let mut six = [0u8; 6];
        sparse.read_exact(&mut six).unwrap();
        let at = 3 * BLOCK as usize - 3;
        assert_eq!(six, file[at..at + 6]);
        assert_eq!(
            sparse.seek(SeekFrom::End(-1)).unwrap(),
            file.len() as u64 - 1
        );
        let (buf, read) = sparse.into_parts();
        assert_eq!(read, 4 * BLOCK + 100);
        let block = BLOCK as usize;
        assert_eq!(buf[..4 * block], file[..4 * block]);
        assert!(buf[4 * block..5 * block].iter().all(|&b| b == 0));
        assert_eq!(buf[5 * block..], file[5 * block..]);
    }

    /// A little-endian TIFF: IFD0 with a strip and an Exif pointer, the
    /// Exif IFD with a maker note of 300 KB and 400 KB of undefined
    /// bytes. The walk reads the IFDs, the maker note, and the short
    /// value, and neither the strip nor the long value.
    #[test]
    fn a_tiff_walk_reads_its_directories_and_values_and_no_image() {
        let mut file = pattern(2 << 20);
        let put = |f: &mut Vec<u8>, at: usize, b: &[u8]| f[at..at + b.len()].copy_from_slice(b);
        let entry = |tag: u16, kind: u16, n: u32, value: u32| {
            let mut e = Vec::new();
            e.extend_from_slice(&tag.to_le_bytes());
            e.extend_from_slice(&kind.to_le_bytes());
            e.extend_from_slice(&n.to_le_bytes());
            e.extend_from_slice(&value.to_le_bytes());
            e
        };
        let ifd = |entries: &[Vec<u8>]| {
            let mut b = (entries.len() as u16).to_le_bytes().to_vec();
            entries.iter().for_each(|e| b.extend_from_slice(e));
            b.extend_from_slice(&0u32.to_le_bytes());
            b
        };
        put(&mut file, 0, b"II\x2a\x00\x00\x00\x01\x00");
        let (ifd0, exif, make, note, long, strip) =
            (0x1_0000, 0x2_0000, 0x3_0000, 0x4_0000, 0xa_0000, 0x18_0000);
        put(
            &mut file,
            ifd0,
            &ifd(&[
                entry(0x010f, 2, 12, make as u32),
                entry(0x0111, 4, 1, strip as u32),
                entry(0x0117, 4, 1, 0x4_0000),
                entry(0x8769, 4, 1, exif as u32),
            ]),
        );
        put(
            &mut file,
            exif,
            &ifd(&[
                entry(MAKER_NOTE, 7, 300_000, note as u32),
                entry(0x9999, 7, 400_000, long as u32),
            ]),
        );
        let mut sparse = Sparse::new(Cursor::new(file.clone()), file.len() as u64).unwrap();
        assert!(fetch_metadata(&mut sparse).unwrap());
        let (buf, read) = sparse.into_parts();
        let same = |at: usize, n: usize| buf[at..at + n] == file[at..at + n];
        assert!(same(0, HEAD as usize));
        assert!(same(ifd0, 2 + 4 * 12 + 4));
        assert!(same(exif, 2 + 2 * 12 + 4));
        assert!(same(make, 12));
        assert!(same(note, 300_000));
        let zeros = |at: usize, n: usize| buf[at..at + n].iter().all(|&b| b == 0);
        assert!(zeros(long + BLOCK as usize, 400_000 - 2 * BLOCK as usize));
        assert!(zeros(strip, 0x4_0000));
        assert!(read < 450_000, "{read}");
    }

    /// A little-endian TIFF header with IFD0 at 8 and `ifd0`'s entries,
    /// `(tag, kind, count, value)`, in a file `len` bytes long.
    fn a_tiff(len: usize, ifd0: &[(u16, u16, u32, u32)]) -> Vec<u8> {
        let mut file = pattern(len);
        file[..8].copy_from_slice(b"II\x2a\x00\x08\x00\x00\x00");
        file[8..10].copy_from_slice(&(ifd0.len() as u16).to_le_bytes());
        for (i, (tag, kind, n, value)) in ifd0.iter().enumerate() {
            let at = 10 + 12 * i;
            file[at..at + 2].copy_from_slice(&tag.to_le_bytes());
            file[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
            file[at + 4..at + 8].copy_from_slice(&n.to_le_bytes());
            file[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
        }
        let next = 10 + 12 * ifd0.len();
        file[next..next + 4].copy_from_slice(&0u32.to_le_bytes());
        file
    }

    /// IFD0's 4096 entries each point at 64 of 1984 small IFDs: the
    /// walk reads [`MAX_IFDS`] of them and holds no more than
    /// [`MAX_QUEUED`] to read at once.
    #[test]
    fn a_tiff_walk_stops_at_its_bounds() {
        let (ifds, pointers) = (0x10_0000u32, 0x20_0000u32);
        let entries: Vec<_> = (0..4096u32)
            .map(|i| (0x014a, 4, 64, pointers + 256 * (i % 31)))
            .collect();
        let mut file = a_tiff(0x21_0000, &entries);
        for k in 0..1984u32 {
            let at = (ifds + 18 * k) as usize;
            file[at..at + 18]
                .copy_from_slice(&[1, 0, 0, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
            let p = (pointers + 4 * k) as usize;
            file[p..p + 4].copy_from_slice(&(ifds + 18 * k).to_le_bytes());
        }
        let mut sparse = Sparse::new(Cursor::new(file.clone()), file.len() as u64).unwrap();
        let walked = tiff(&mut sparse, 0, &[]).unwrap().unwrap();
        assert_eq!(walked.ifds, MAX_IFDS);
        assert!(walked.most_queued <= MAX_QUEUED, "{walked:?}");
    }

    /// 64 IFDs of 4096 entries, each a value of 256 KB over the same
    /// bytes: fetched into place once, never copied out, so the walk is
    /// quick where a copy a value was 64 GB.
    #[test]
    fn a_tiff_walk_copies_no_values() {
        let n = BULK as u32;
        let entries: Vec<_> = (0..4096u32).map(|i| (0x9000 + i as u16, 1, n, 8)).collect();
        let ifd = 2 + 12 * entries.len() + 4;
        let mut file = a_tiff(64 * ifd + BULK as usize + 8, &entries);
        // Each IFD a copy of IFD0, chained to the next.
        let first = file[8..8 + ifd].to_vec();
        for k in 1..64 {
            file[8 + k * ifd..8 + (k + 1) * ifd].copy_from_slice(&first);
            let link = 8 + k * ifd - 4;
            file[link..link + 4].copy_from_slice(&((8 + k * ifd) as u32).to_le_bytes());
        }
        let started = std::time::Instant::now();
        let mut sparse = Sparse::new(Cursor::new(file.clone()), file.len() as u64).unwrap();
        let walked = tiff(&mut sparse, 0, &[]).unwrap().unwrap();
        assert_eq!(walked.ifds, 64);
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
    }

    /// A TIFF whose IFD points at itself ends the walk, not the read.
    #[test]
    fn a_tiff_walk_ends_on_a_loop() {
        let mut file = pattern(256 * 1024);
        file[..8].copy_from_slice(b"II\x2a\x00\x08\x00\x00\x00");
        file[8..10].copy_from_slice(&1u16.to_le_bytes());
        file[10..22].copy_from_slice(&[0x69, 0x87, 4, 0, 1, 0, 0, 0, 8, 0, 0, 0]);
        file[22..26].copy_from_slice(&8u32.to_le_bytes());
        let mut sparse = Sparse::new(Cursor::new(file.clone()), file.len() as u64).unwrap();
        assert!(fetch_metadata(&mut sparse).unwrap());
        assert_eq!(sparse.into_parts().1, HEAD);
    }
}
