//! Focus scores, filled in after the pass: a pass writes its rows with
//! the focus NULL and reads no preview, and [`score`] then reads the
//! previews of the rows still NULL on a few threads and writes their
//! scores in short batches, so a fresh card is listed at the pass's
//! speed and its scores land behind it.
//!
//! A frame on a network mount ([`crate::Library::set_network`]) is
//! scored only when its preview comes from the file's ends (a CR3, a
//! RAF): any other raw's preview is read from the whole file, tens of
//! megabytes a frame over the share, and a JPEG or a TIFF is decoded
//! whole. Those stay NULL. The ones scored are read one at a time,
//! each on a thread of its own and waited for up to [`FAR_LIMIT`],
//! outside the pass's reads and their limit, so a slow share never
//! marks its root as not answering. A read that does not answer is
//! counted against its share (the topmost folder above it on the
//! mount) until its thread comes back, and every sweep meanwhile asks
//! that share nothing, so a hung share strands one thread and not one
//! a sweep; a share whose pass has a read out is asked nothing either.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{TransactionBehavior, params};

use crate::{Library, Report, Result, path_from_bytes};

/// The threads a sweep reads local previews on: a preview is a JPEG
/// decode and a score, tens of milliseconds, and three leave the rest
/// of the machine to the window and its thumbnails.
pub const THREADS: usize = 3;

/// Rows a batch scores before its scores are written in one short
/// transaction.
const BATCH: usize = 32;

/// How long a network file's preview read may go unanswered before
/// the sweep reads no more network files.
pub const FAR_LIMIT: Duration = Duration::from_secs(60);

/// The scorer's network reads given up on and not back yet, by share.
static OUT: Mutex<Vec<(PathBuf, Arc<AtomicUsize>)>> = Mutex::new(Vec::new());

/// The count of `share`'s reads out, made when first asked for.
fn out_for(share: &Path) -> Arc<AtomicUsize> {
    let mut all = OUT.lock().unwrap_or_else(|e| e.into_inner());
    all.retain(|(_, n)| n.load(Ordering::SeqCst) > 0 || Arc::strong_count(n) > 1);
    match all.iter().find(|(f, _)| f == share) {
        Some((_, n)) => n.clone(),
        None => {
            let n = Arc::new(AtomicUsize::new(0));
            all.push((share.to_path_buf(), n.clone()));
            n
        }
    }
}

/// How a preview is scored: [`read_focus`], or a test's stand-in.
pub(crate) type Reader = Arc<dyn Fn(&Path) -> Option<f32> + Send + Sync>;

/// What one sweep knows of a folder, asked once a folder a sweep: on
/// a network mount or not, the share it is on, and whether that
/// share's own pass or the scorer has a read out on it.
#[derive(Clone)]
struct Place {
    far: bool,
    share: PathBuf,
    held: bool,
}

/// Whether a file's preview is read from its ends, not the whole
/// file: what is scored on a network mount.
pub fn preview_from_ends(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cr3") || e.eq_ignore_ascii_case("raf"))
}

/// A file's focus score from its preview
/// (`greycard_core::sharpness::focus_score`): a raw's embedded JPEG, a
/// picture's own pixels. `None`, said in the log, for a file with no
/// preview, one whose preview will not decode, and one that will not
/// read at all (a probe that failed among them): stored as no preview,
/// and not read again until the file changes.
pub fn read_focus(path: &Path) -> Option<f32> {
    let scored = std::panic::catch_unwind(|| {
        let rgb = crate::bursts::read_preview(path)?;
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        greycard_core::sharpness::focus_score(rgb.as_raw(), w, h)
    });
    match scored {
        Ok(Some(score)) => Some(score),
        Ok(None) => {
            log::debug!("{}: no preview to score", path.display());
            None
        }
        Err(_) => {
            log::warn!("{}: its preview: the decoder panicked", path.display());
            None
        }
    }
}

/// A row to score: its id, its path, and the mtime it was indexed at.
struct Unscored {
    id: i64,
    path: PathBuf,
    mtime: i64,
}

impl Library {
    /// The next rows not scored, past row `after`, by id; the missing
    /// left out.
    fn unscored(&self, after: i64, limit: usize) -> Result<Vec<Unscored>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path, mtime FROM files WHERE id > ? AND focus IS NULL \
             AND missing_since IS NULL ORDER BY id LIMIT ?",
        )?;
        let rows = stmt.query_map(params![after, limit as i64], |r| {
            Ok(Unscored {
                id: r.get(0)?,
                path: path_from_bytes(&r.get::<_, Vec<u8>>(1)?),
                mtime: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Keep these scores, each while its row is at the mtime it was
    /// read at and not scored since: a file changed meanwhile is NULL
    /// again and is scored by the next sweep.
    fn keep_scores(&mut self, scores: &[(i64, i64, Option<f32>)]) -> Result<usize> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut kept = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE files SET focus = ? WHERE id = ? AND mtime = ? AND focus IS NULL",
            )?;
            for (id, mtime, focus) in scores {
                kept += stmt.execute(params![crate::focus_column(*focus), id, mtime])?;
            }
        }
        tx.commit()?;
        Ok(kept)
    }

    /// What a sweep needs of `dir` (see [`Place`]), each folder asked of
    /// the mount table once a sweep through `seen`, its folders above
    /// it among them.
    fn place(&self, dir: &Path, seen: &mut HashMap<PathBuf, bool>) -> Place {
        let mut is = |d: &Path| match seen.get(d) {
            Some(&far) => far,
            None => {
                let far = self.network.as_ref().is_some_and(|n| (n.is)(d));
                seen.insert(d.to_path_buf(), far);
                far
            }
        };
        if !is(dir) {
            return Place {
                far: false,
                share: PathBuf::new(),
                held: false,
            };
        }
        let mut share = dir;
        while let Some(up) = share.parent().filter(|&u| is(u)) {
            share = up;
        }
        let held = out_for(share).load(Ordering::SeqCst) > 0
            || self.far_for(dir).is_some_and(|f| f.stalled());
        Place {
            far: true,
            share: share.to_path_buf(),
            held,
        }
    }
}

/// Score every row of the library not scored yet, the local ones on
/// `threads` threads and the network ones one at a time (see the
/// module), a batch at a time; `landed` is told each batch written,
/// with the folders its rows are in. `stop` is asked between files;
/// a sweep stopped leaves the rest NULL for the next. The report's
/// `scored` counts the rows written.
pub fn score(
    lib: &mut Library,
    threads: usize,
    stop: &(dyn Fn() -> bool + Sync),
    landed: &mut dyn FnMut(&[PathBuf]),
) -> Result<Report> {
    let reader: Reader = Arc::new(read_focus);
    score_with(lib, threads, stop, landed, &reader, FAR_LIMIT)
}

/// [`score`] with the reader and the network limit given.
pub(crate) fn score_with(
    lib: &mut Library,
    threads: usize,
    stop: &(dyn Fn() -> bool + Sync),
    landed: &mut dyn FnMut(&[PathBuf]),
    reader: &Reader,
    limit: Duration,
) -> Result<Report> {
    let mut report = Report::default();
    let mut after = 0;
    let mut seen: HashMap<PathBuf, bool> = HashMap::new();
    let mut places: HashMap<PathBuf, Place> = HashMap::new();
    loop {
        let rows = lib.unscored(after, BATCH)?;
        let Some(last) = rows.last() else {
            break;
        };
        after = last.id;
        let mut place = |r: &Unscored| -> Place {
            let dir = r.path.parent().unwrap_or(Path::new(""));
            if let Some(p) = places.get(dir) {
                return p.clone();
            }
            let p = lib.place(dir, &mut seen);
            places.insert(dir.to_path_buf(), p.clone());
            p
        };
        let (mut far, mut near) = (Vec::new(), Vec::new());
        for r in &rows {
            let p = place(r);
            if p.far {
                far.push((r, p));
            } else {
                near.push(r);
            }
        }
        let mut scores = read_near(&near, threads, stop, reader);
        for (r, p) in far {
            if stop() {
                break;
            }
            // Read again for each file: a read given up on in this
            // sweep holds its share for the rest of it too.
            let out = out_for(&p.share);
            if p.held || out.load(Ordering::SeqCst) > 0 || !preview_from_ends(&r.path) {
                continue;
            }
            match read_far(&r.path, reader, limit, &out) {
                Some(focus) => scores.push((r.id, r.mtime, focus)),
                None => log::warn!(
                    "{}: its preview did not answer in {} s; {} is asked nothing more until \
                     it does",
                    r.path.display(),
                    limit.as_secs_f64(),
                    p.share.display()
                ),
            }
        }
        if !scores.is_empty() {
            report.scored += lib.keep_scores(&scores)?;
            let mut folders: Vec<PathBuf> = Vec::new();
            for r in rows.iter().filter(|r| scores.iter().any(|s| s.0 == r.id)) {
                if let Some(d) = r.path.parent()
                    && !folders.iter().any(|f| f == d)
                {
                    folders.push(d.to_path_buf());
                }
            }
            landed(&folders);
        }
        if stop() {
            report.stopped = true;
            break;
        }
    }
    Ok(report)
}

/// The local rows' scores, on up to `threads` threads.
fn read_near(
    rows: &[&Unscored],
    threads: usize,
    stop: &(dyn Fn() -> bool + Sync),
    reader: &Reader,
) -> Vec<(i64, i64, Option<f32>)> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(rows.len()));
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, rows.len().max(1)) {
            s.spawn(|| {
                while !stop()
                    && let Some(r) = rows.get(next.fetch_add(1, Ordering::Relaxed))
                {
                    let focus = reader(&r.path);
                    out.lock().expect("scores").push((r.id, r.mtime, focus));
                }
            });
        }
    });
    out.into_inner().expect("scores")
}

/// A network file's score, read on a thread of its own and waited for
/// up to `limit`; `None` when it did not answer, the thread left to it
/// and counted in `out` until it comes back.
fn read_far(
    path: &Path,
    reader: &Reader,
    limit: Duration,
    out: &Arc<AtomicUsize>,
) -> Option<Option<f32>> {
    const RUNNING: usize = 0;
    const GIVEN_UP: usize = 1;
    let (tx, rx) = std::sync::mpsc::channel();
    let state = Arc::new(AtomicUsize::new(RUNNING));
    let (p, reader, mine, count) = (
        path.to_path_buf(),
        reader.clone(),
        state.clone(),
        out.clone(),
    );
    std::thread::Builder::new()
        .name("greycard focus score".into())
        .spawn(move || {
            let _ = tx.send(reader(&p));
            // Given up on and back after all: the share answers again.
            if mine.swap(GIVEN_UP + 1, Ordering::SeqCst) == GIVEN_UP {
                count.fetch_sub(1, Ordering::SeqCst);
            }
        })
        .ok()?;
    match rx.recv_timeout(limit) {
        Ok(focus) => Some(focus),
        Err(_) => {
            // Counted before it is given up on, so the thread's return
            // always finds the count to take back.
            out.fetch_add(1, Ordering::SeqCst);
            if state
                .compare_exchange(RUNNING, GIVEN_UP, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                // It came back between the wait and the mark.
                out.fetch_sub(1, Ordering::SeqCst);
                return rx.try_recv().ok();
            }
            None
        }
    }
}
