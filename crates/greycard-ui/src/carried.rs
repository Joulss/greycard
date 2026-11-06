//! The collections, smart collections, and stacks carried beside their
//! frames (`greycard_library::carry`): the editor's side.
//!
//! Every save of one, here, is sent to the carrier, which writes its
//! copy into the folders it touches on a thread of its own, each
//! folder's reads and writes under a time limit, so a share that does
//! not answer holds nothing of the window's and only its own folder of
//! the writer's. It keeps, beside the collections (`carried/
//! state.json`, a folder an older build passes over), the revision and
//! the folders each was last written to, so a copy is taken out of a
//! folder its frames left and written again where a write failed (a
//! minute later, and at the next launch), a folder whose write did not
//! answer kept among them, since that write may finish after; and the
//! deletions made here, written by the window's own thread before it
//! goes on, so a copy met later that a deletion saw does not come back,
//! a quit while the writer is held up included.
//!
//! The index's passes bring the copies they find ([`Report::sets`]),
//! and the window joins them into its own (`take`): a set new here is
//! added, one behind is left (its folder is written again), one ahead
//! is taken, two that went their own ways keep both. A copy a deletion
//! here has seen is buried where it was found, and the leftovers a pass
//! names (part files a write left, deletions past their keep) are
//! cleared here, on the writer's thread.
//!
//! [`Report::sets`]: greycard_library::Report::sets

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use greycard_library::Roots;
use greycard_library::carry::{self, Gone, Item, Wrote};
use serde::{Deserialize, Serialize};

/// The folder beside the collections that holds what the carrier keeps.
pub const DIR: &str = "carried";

const STATE: &str = "state.json";

/// The shape of what the carrier keeps: a later build's is left alone,
/// and nothing is carried by this one while it stands.
const STATE_VERSION: u32 = 1;

/// The longest one folder's reads and writes are waited on: a share
/// that does not answer by then is passed over, and tried again.
const FOLDER_LIMIT: Duration = Duration::from_secs(10);

/// How long a write that failed waits before it is tried again.
const RETRY: Duration = Duration::from_secs(60);

/// The longest a quit waits for the writes asked for before it.
pub const QUIT_WAIT: Duration = Duration::from_secs(3);

/// Deletions kept, oldest dropped first.
const GONE_KEPT: usize = 512;

/// This machine's name, read once.
pub fn host() -> &'static str {
    static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOST.get_or_init(greycard_edit::sync::host)
}

/// Seconds since the epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Where one set's copies were last written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Written {
    /// The hash of its lineage then.
    rev: String,
    folders: Vec<PathBuf>,
    /// Folders whose write did not answer in time: it may finish after,
    /// so a copy may be there. The next write goes there again, a set
    /// that leaves one is taken out of it, and a deletion is carried to
    /// it, as to the folders written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    late: Vec<PathBuf>,
}

impl Written {
    /// Every folder a copy was, or may have been, written to.
    fn all(self) -> Vec<PathBuf> {
        let mut all = self.folders;
        for f in self.late {
            if !all.contains(&f) {
                all.push(f);
            }
        }
        all
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    version: u32,
    written: BTreeMap<String, Written>,
    gone: Vec<Gone>,
}

/// What the carrier keeps, shared by the window and the writer: neither
/// holds it across a folder's read or write, only across the small
/// local file it is saved to.
struct Kept {
    file: PathBuf,
    state: State,
}

impl Kept {
    /// Read from `file`. One that will not read is kept beside itself
    /// as `.json.bad` and started afresh; a later build's is none.
    fn load(file: PathBuf) -> Option<Kept> {
        let state = match std::fs::read(&file) {
            Ok(b) => match serde_json::from_slice::<State>(&b) {
                Ok(s) if s.version > STATE_VERSION => {
                    tracing::warn!(
                        "carried sets {}: a later build's; nothing carried this session",
                        file.display()
                    );
                    return None;
                }
                Ok(s) => s,
                Err(e) => {
                    let aside = file.with_extension("json.bad");
                    let _ = std::fs::copy(&file, &aside);
                    tracing::warn!(
                        "carried sets {}: {e}; kept as {}, started afresh",
                        file.display(),
                        aside.display()
                    );
                    State::default()
                }
            },
            Err(_) => State::default(),
        };
        Some(Kept { file, state })
    }

    fn save(&mut self) {
        self.state.version = STATE_VERSION;
        let text = match serde_json::to_string_pretty(&self.state) {
            Ok(t) => t,
            Err(e) => return tracing::warn!("carried sets: {e}"),
        };
        let file = &self.file;
        let write = || -> std::io::Result<()> {
            if let Some(d) = file.parent() {
                std::fs::create_dir_all(d)?;
            }
            static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let part = file.with_extension(format!("json.{}-{n}.part", std::process::id()));
            greycard_edit::write_synced(&part, text.as_bytes())?;
            std::fs::rename(&part, file)
        };
        if let Err(e) = write() {
            tracing::warn!("carried sets: {} not written: {e}", file.display());
        }
    }

    /// The deletion of a set kept, its record of where it was written
    /// taken: the folders the deletion is to be carried to.
    fn bury(&mut self, gone: &Gone) -> Vec<PathBuf> {
        let folders = self
            .state
            .written
            .remove(&gone.id)
            .map(Written::all)
            .unwrap_or_default();
        self.state.gone.retain(|g| g.id != gone.id);
        self.state.gone.push(gone.clone());
        let n = self.state.gone.len();
        if n > GONE_KEPT {
            self.state.gone.drain(..n - GONE_KEPT);
        }
        self.save();
        folders
    }
}

enum Job {
    /// A set's copies written; `force` when one of its folders is known
    /// to hold a copy behind it, though it was written there.
    Put { item: Box<Item>, force: bool },
    /// A deletion carried to the folders it was written to.
    Bury { gone: Gone, folders: Vec<PathBuf> },
    /// Leftovers a pass found in the sets' folders, cleared.
    Clear(Vec<PathBuf>),
    /// Answered once every job before it is done.
    Done(mpsc::Sender<()>),
}

/// The folders a write is out on and not yet back from.
type Out = Arc<Mutex<std::collections::HashSet<PathBuf>>>;

/// `work` on `folder`, waited on for `limit` at most: a share that does
/// not answer leaves its thread behind, as the index's reads do, and
/// the writer goes on. One thread a folder at most: while one is still
/// out on it, nothing more is asked of that folder. None when it did
/// not answer, or was not asked.
fn limited<T: Send + 'static>(
    out: &Out,
    folder: &Path,
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    if !out
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(folder.to_path_buf())
    {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    let (back, key) = (out.clone(), folder.to_path_buf());
    let spawned = std::thread::Builder::new()
        .name("greycard carry folder".into())
        .spawn(move || {
            let done = work();
            back.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
            let _ = tx.send(done);
        });
    if spawned.is_err() {
        out.lock().unwrap_or_else(|e| e.into_inner()).remove(folder);
        return None;
    }
    rx.recv_timeout(limit).ok()
}

/// The writer of the copies: the thread's half.
struct Worker {
    kept: Arc<Mutex<Kept>>,
    roots: Option<PathBuf>,
    /// The roots as last read, and the file's time then: read again
    /// only when the file changed, not for every set at a launch.
    read: Option<(std::time::SystemTime, Roots)>,
    /// Sets whose copies did not all go, to be tried again.
    failed: BTreeMap<String, Item>,
    /// Leftovers whose folder did not answer, to be cleared again.
    clearing: Vec<PathBuf>,
    /// Folders that refused a clear, said once.
    refused: std::collections::HashSet<PathBuf>,
    limit: Duration,
    out: Out,
}

impl Worker {
    fn run(&mut self, job: Job) {
        match job {
            Job::Put { item, force } => self.put(&item, force),
            Job::Bury { gone, folders } => self.bury(&gone, &folders),
            Job::Clear(paths) => self.clear(paths),
            Job::Done(tx) => {
                let _ = tx.send(());
            }
        }
    }

    /// The sets whose writes failed, tried again, and the leftovers
    /// whose folder did not answer.
    fn retry(&mut self) {
        for (_, item) in std::mem::take(&mut self.failed) {
            self.put(&item, true);
        }
        let clearing = std::mem::take(&mut self.clearing);
        if !clearing.is_empty() {
            self.clear(clearing);
        }
    }

    /// Whether anything waits to be tried again.
    fn waiting(&self) -> bool {
        !self.failed.is_empty() || !self.clearing.is_empty()
    }

    /// The leftovers at `paths` cleared, folder by folder, each under
    /// the folder's limit. One that did not answer is tried again on
    /// the minute; a folder that refused (a read-only share) is said
    /// once a session, though the next pass names its leftovers again.
    fn clear(&mut self, paths: Vec<PathBuf>) {
        let mut by: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for p in paths {
            if let Some(folder) = p.parent().and_then(Path::parent).and_then(Path::parent) {
                let at = by.entry(folder.to_path_buf()).or_default();
                if !at.contains(&p) {
                    at.push(p);
                }
            }
        }
        for (folder, paths) in by {
            let asked = paths.clone();
            match limited(&self.out, &folder, self.limit, move || carry::clear(&asked)) {
                Some(done) => {
                    if done.removed > 0 {
                        tracing::info!(
                            "carried sets {}: {} leftovers cleared",
                            folder.display(),
                            done.removed
                        );
                    }
                    if let Some(e) = done.refused
                        && self.refused.insert(folder.clone())
                    {
                        tracing::info!(
                            "carried sets {}: leftovers not cleared: {e}",
                            folder.display()
                        );
                    }
                }
                None => {
                    tracing::info!(
                        "carried sets {}: did not answer; leftovers cleared later",
                        folder.display()
                    );
                    self.clearing.extend(paths);
                }
            }
        }
    }

    fn roots(&mut self) -> Option<Roots> {
        let file = self.roots.as_ref()?;
        let Ok(time) = std::fs::metadata(file).and_then(|m| m.modified()) else {
            // A library whose roots file is not there holds no folder.
            return Some(Roots::default());
        };
        match &self.read {
            Some((t, roots)) if *t == time => Some(roots.clone()),
            _ => {
                let roots = Roots::load(file).unwrap_or_default();
                self.read = Some((time, roots.clone()));
                Some(roots)
            }
        }
    }

    /// `item`'s copies written where its frames are, and taken out of
    /// the folders they left; nothing done when it was written to the
    /// same folders at the same revision.
    fn put(&mut self, item: &Item, force: bool) {
        let id = item.id().to_string();
        let roots = self.roots();
        let targets = carry::targets_of(item, roots.as_ref());
        let folders: Vec<PathBuf> = targets.iter().map(|t| t.folder.clone()).collect();
        // The whole lineage, not the current revision alone: a join
        // that took in the other side's revisions is written, so the
        // copy there reads as behind it.
        let rev = blake3::hash(item.lineage().revs.join("\n").as_bytes())
            .to_hex()
            .to_string();
        let before = {
            let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
            if kept
                .state
                .gone
                .iter()
                .any(|g| g.id == id && g.covers(item.lineage()))
            {
                // Deleted here since it was asked for.
                return;
            }
            kept.state.gone.retain(|g| g.id != id);
            kept.state.written.get(&id).cloned().unwrap_or_default()
        };
        self.failed.remove(&id);
        if !force && before.rev == rev && before.folders == folders && before.late.is_empty() {
            return;
        }
        let mut done = Vec::new();
        let mut late = Vec::new();
        let mut failed = false;
        for t in targets {
            let (folder, home, it) = (t.folder.clone(), t.home, item.clone());
            let wrote = limited(&self.out, &t.folder, self.limit, move || {
                carry::put_as(&folder, &home, &it, greycard_library::collections::VERSION)
            });
            match wrote {
                Some(Ok(Wrote::Written | Wrote::Same)) => done.push(t.folder),
                Some(Ok(other)) => {
                    tracing::info!(
                        "carried set {id} in {}: {other:?}, left",
                        t.folder.display()
                    );
                    done.push(t.folder);
                }
                // A folder that is not there is a ref's stale hint,
                // a frame moved on since: the set's next revision
                // names where it went. Written nowhere, and not asked
                // again every minute.
                Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    tracing::info!(
                        "carried set {id} not written to {}: {e}; not tried again",
                        t.folder.display()
                    );
                    done.push(t.folder);
                }
                Some(Err(e)) => {
                    failed = true;
                    tracing::info!(
                        "carried set {id} not written to {}: {e}",
                        t.folder.display()
                    )
                }
                None => {
                    failed = true;
                    tracing::info!(
                        "carried set {id}: {} did not answer; tried again later",
                        t.folder.display()
                    );
                    late.push(t.folder);
                }
            }
        }
        // Out of the folders its frames left: those it was written to,
        // and those whose write did not answer and may have finished
        // since, where only a copy this one has seen is taken.
        let left = |f: &PathBuf| !folders.contains(f);
        let was_late: Vec<PathBuf> = before.late.iter().filter(|f| left(f)).cloned().collect();
        for f in before.folders.iter().filter(|f| left(f)).chain(&was_late) {
            let (folder, it) = (f.clone(), item.clone());
            match limited(&self.out, f, self.limit, move || {
                carry::take_out(&folder, &it)
            }) {
                Some(Ok(_)) => {}
                _ => {
                    tracing::info!("carried set {id} not taken out of {}", f.display());
                    if was_late.contains(f) {
                        failed = true;
                        late.push(f.clone());
                    } else {
                        done.push(f.clone());
                    }
                }
            }
        }
        // Deleted while the writes were out: the folders just written
        // take the deletion, rather than be kept as the set's.
        let deleted = {
            let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
            let gone = kept
                .state
                .gone
                .iter()
                .find(|g| g.id == id && g.covers(item.lineage()))
                .cloned();
            if gone.is_none() {
                kept.state.written.insert(
                    id.clone(),
                    Written {
                        rev,
                        folders: done.clone(),
                        late: late.clone(),
                    },
                );
                kept.save();
            }
            gone
        };
        match deleted {
            Some(gone) => {
                let all = Written {
                    folders: done,
                    late,
                    ..Written::default()
                }
                .all();
                self.bury(&gone, &all)
            }
            None if failed => {
                self.failed.insert(id, item.clone());
            }
            None => {}
        }
    }

    /// A deletion carried to the folders the set was last written to:
    /// those the window knew of when it was made, and any a write still
    /// out then has recorded since.
    fn bury(&mut self, gone: &Gone, folders: &[PathBuf]) {
        self.failed.remove(&gone.id);
        let mut folders = folders.to_vec();
        {
            let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(w) = kept.state.written.remove(&gone.id) {
                for f in w.all() {
                    if !folders.contains(&f) {
                        folders.push(f);
                    }
                }
                kept.save();
            }
        }
        for f in &folders {
            let (folder, g) = (f.clone(), gone.clone());
            match limited(&self.out, f, self.limit, move || carry::bury(&folder, &g)) {
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    tracing::info!(
                        "carried deletion {} not written to {}: {e}",
                        gone.id,
                        f.display()
                    )
                }
                None => tracing::info!(
                    "carried deletion {}: {} did not answer",
                    gone.id,
                    f.display()
                ),
            }
        }
    }
}

/// The window's handle on the writer of the copies, shared by the
/// collections and the stacks.
pub struct Carrier {
    tx: Option<mpsc::Sender<Job>>,
    /// The writer run in place, for the tests: every write done before
    /// the call that asked for it returns.
    inline: Option<RefCell<Worker>>,
    /// What is kept beside the collections, the deletions among it;
    /// none while a later build's is there.
    kept: Option<Arc<Mutex<Kept>>>,
    /// The folders the writer has a read or write out on.
    out: Out,
}

impl Carrier {
    /// The carrier for the collections in `collections`, the copies'
    /// folders paired by the roots in `roots` (the library's roots
    /// file, read again when it changes, so a pairing made since
    /// counts; none in the tests, for no roots to judge folders by).
    pub fn start(collections: &Path, roots: Option<PathBuf>) -> Carrier {
        Carrier::make(collections, roots, false, FOLDER_LIMIT)
    }

    /// [`Carrier::start`] with the writes done in place.
    pub fn in_place(collections: &Path, roots: Option<PathBuf>) -> Carrier {
        Carrier::make(collections, roots, true, FOLDER_LIMIT)
    }

    fn make(collections: &Path, roots: Option<PathBuf>, inline: bool, limit: Duration) -> Carrier {
        let none = || Carrier {
            tx: None,
            inline: None,
            kept: None,
            out: Out::default(),
        };
        let Some(kept) = Kept::load(collections.join(DIR).join(STATE)) else {
            return none();
        };
        let kept = Arc::new(Mutex::new(kept));
        let out = Out::default();
        let mut worker = Worker {
            kept: kept.clone(),
            roots,
            read: None,
            failed: BTreeMap::new(),
            clearing: Vec::new(),
            refused: Default::default(),
            limit,
            out: out.clone(),
        };
        if inline {
            return Carrier {
                tx: None,
                inline: Some(RefCell::new(worker)),
                kept: Some(kept),
                out,
            };
        }
        let (tx, rx) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("greycard carry".into())
            .spawn(move || {
                loop {
                    let wait = if worker.waiting() {
                        RETRY
                    } else {
                        Duration::MAX
                    };
                    match rx.recv_timeout(wait) {
                        Ok(job) => worker.run(job),
                        Err(mpsc::RecvTimeoutError::Timeout) => worker.retry(),
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("carried sets: no writer ({e}); nothing is carried this session");
            return none();
        }
        Carrier {
            tx: Some(tx),
            inline: None,
            kept: Some(kept),
            out,
        }
    }

    fn send(&self, job: Job) {
        if let Some(w) = &self.inline {
            w.borrow_mut().run(job);
        } else if let Some(tx) = &self.tx {
            let _ = tx.send(job);
        }
    }

    /// `item`'s copies written as it stands.
    pub fn put(&self, item: Item) {
        self.send(Job::Put {
            item: Box::new(item),
            force: false,
        });
    }

    /// `item`'s copies written again where they are behind it: a pass
    /// found one there that is.
    pub fn refresh(&self, item: Item) {
        self.send(Job::Put {
            item: Box::new(item),
            force: true,
        });
    }

    /// A set deleted here, or a deletion carried here: kept on the disk
    /// before this returns, then carried to the folders it was written
    /// to.
    pub fn bury(&self, gone: Gone) {
        let Some(kept) = &self.kept else {
            return;
        };
        let folders = kept.lock().unwrap_or_else(|e| e.into_inner()).bury(&gone);
        self.send(Job::Bury { gone, folders });
    }

    fn gone(&self, f: impl Fn(&Gone) -> bool) -> bool {
        self.kept.as_ref().is_some_and(|k| {
            k.lock()
                .unwrap_or_else(|e| e.into_inner())
                .state
                .gone
                .iter()
                .any(f)
        })
    }

    /// Whether a deletion made or taken here has seen `item`.
    pub fn buried(&self, item: &Item) -> bool {
        self.gone(|g| g.id == item.id() && g.covers(item.lineage()))
    }

    /// A copy of `item` a pass found in `folder`, which a deletion made
    /// or taken here has seen: a write that finished after its time
    /// left it, or a machine that has not seen the deletion wrote it.
    /// The deletion is carried there, so no other machine takes it.
    /// True when one had seen it.
    pub fn bury_found(&self, item: &Item, folder: &Path) -> bool {
        let Some(kept) = &self.kept else {
            return false;
        };
        let gone = kept
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state
            .gone
            .iter()
            .find(|g| g.id == item.id() && g.covers(item.lineage()))
            .cloned();
        match gone {
            Some(gone) => {
                self.send(Job::Bury {
                    gone,
                    folders: vec![folder.to_path_buf()],
                });
                true
            }
            None => false,
        }
    }

    /// The leftovers a pass found in the sets' folders
    /// (`Report::sets_stale`) cleared.
    pub fn clear(&self, paths: Vec<PathBuf>) {
        if !paths.is_empty() && self.kept.is_some() {
            self.send(Job::Clear(paths));
        }
    }

    /// Whether a deletion of `id` is kept here.
    pub fn knows_gone(&self, id: &str) -> bool {
        self.gone(|g| g.id == id)
    }

    /// Back once every write asked for so far is done, or `limit` has
    /// passed.
    pub fn wait(&self, limit: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        self.send(Job::Done(tx));
        rx.recv_timeout(limit).is_ok()
    }
}

impl Drop for Carrier {
    /// A quit waits a moment for the writes asked for before it; one
    /// held up by a share is tried again at the next launch.
    fn drop(&mut self) {
        if self.tx.is_some() && !self.wait(QUIT_WAIT) {
            tracing::info!("carried sets: writes still waiting at quit; tried again next launch");
        }
    }
}

impl Carrier {
    /// [`Carrier::start`] with a folder's time limit of `limit`, for
    /// the tests (the binary's among them, which build this library
    /// without cfg(test)).
    pub fn start_limited(collections: &Path, limit: Duration) -> Carrier {
        Carrier::make(collections, None, false, limit)
    }

    /// Whether `folder` has a read or write of the writer still out on
    /// it, one past its limit among them: for the tests.
    pub fn out_on(&self, folder: &Path) -> bool {
        self.out
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(folder)
    }
}
