//! Reads over a network mount: each under a limit, several out at once.
//!
//! A pass reads a file in a few round trips (a stat, the hash's head,
//! the probe's ends, and the sidecar), and over a share each is the
//! network's latency and the server's disk, not this machine's. Read
//! one after another, the pass waits on every one in turn; with a few
//! out at once it waits on about one in a few. A NAS whose disks have
//! gone to sleep, or a hard mount whose server has gone, does not
//! answer at all, and a read of it cannot be called off: the thread
//! that made it is stuck until the kernel gives up, which on a hard
//! mount is never. So each read is made on a thread of its own (a
//! file's reads together, or a folder's listing) and waited for up to
//! a limit. One that has not answered by then is an
//! error the pass records, the file's row is left as it was, and the
//! next pass asks again. The thread is left to it.
//!
//! Once one read has gone unanswered, the root is asked nothing new,
//! by this pass or any after it, until that read comes back: the reads
//! already out are waited for, each to its own limit, and the files
//! after them are recorded as not asked. A sleeping NAS costs one
//! limit and the threads that were out, rather than a limit a file,
//! and a share that answers the roots' look but hangs on its files
//! strands no more threads pass after pass. The count of reads given
//! up on is kept for the process, by the root a pass was over, since
//! the threads outlive the pass and its connection: a pass set aside
//! leaves its lane, and the next one opens a library of its own.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How long a read over a network mount may go unanswered before the
/// pass takes it for not answering. A NAS's disks spinning up take ten
/// seconds or more; a read that waits through that is better than one
/// recorded as an error and asked again ten minutes later.
pub const READ_LIMIT: Duration = Duration::from_secs(20);

/// How many of a folder's files are read at once over a network mount.
/// Measured on an SMB share of a NAS on the local network, raws of
/// 20 to 30 MB (`network_reads_measured`): a pass over rows already
/// current, a stat and a sidecar look a file and so round trips alone,
/// went from 2 to 3 ms a file one at a time to 1 ms at two or more. A
/// first pass, which reads about 2 MB a raw and waits on the server's
/// disks seeking, was 45 to 55 ms a file at one, two, and four, within
/// the noise of each other, and slower at eight and sixteen, the disks
/// seeking between files. Four, for a share further away than that,
/// where the round trip is the larger part.
pub const READS_AT_ONCE: usize = 4;

/// How a pass reads a folder on a network mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkReads {
    /// How long one read may go unanswered.
    pub limit: Duration,
    /// How many files are read at once.
    pub at_once: usize,
}

impl Default for NetworkReads {
    fn default() -> Self {
        NetworkReads {
            limit: READ_LIMIT,
            at_once: READS_AT_ONCE,
        }
    }
}

/// Which folders are on a network mount, as the caller tells them
/// (the crate has no mount table of its own), and how those are read.
pub(crate) struct Network {
    pub(crate) is: Box<dyn Fn(&Path) -> bool + Send>,
    pub(crate) reads: NetworkReads,
}

/// The reads given up on that have not come back yet, by the folder
/// the pass was over: a root, or a folder under one.
static STUCK: Mutex<Vec<(PathBuf, Arc<AtomicUsize>)>> = Mutex::new(Vec::new());

/// One pass's reads over a share. Cloned, it is the same pass: a read
/// unanswered under one clone holds the others' new reads too.
#[derive(Debug, Clone)]
pub(crate) struct Far {
    reads: NetworkReads,
    /// Reads given up on under the pass's own folder, not back yet.
    stuck: Arc<AtomicUsize>,
    /// The same for the folders above it and below it: a pass over a
    /// folder of a root whose last pass stranded a read asks nothing
    /// either, and the root's pass nothing while a folder's read is out.
    kin: Vec<Arc<AtomicUsize>>,
}

const RUNNING: u8 = 0;
const DONE: u8 = 1;
const GIVEN_UP: u8 = 2;

/// A read out on its thread.
struct Job<T> {
    answer: mpsc::Receiver<T>,
    state: Arc<AtomicU8>,
    started: Instant,
}

impl Far {
    /// The reads of a pass over `folder`.
    pub(crate) fn new(reads: NetworkReads, folder: &Path) -> Far {
        let mut all = STUCK.lock().unwrap_or_else(|e| e.into_inner());
        // A count at nothing, held by no pass and no read, is forgotten.
        all.retain(|(_, n)| n.load(Ordering::SeqCst) > 0 || Arc::strong_count(n) > 1);
        let stuck = match all.iter().find(|(f, _)| f == folder) {
            Some((_, n)) => n.clone(),
            None => {
                let n = Arc::new(AtomicUsize::new(0));
                all.push((folder.to_path_buf(), n.clone()));
                n
            }
        };
        let kin = all
            .iter()
            .filter(|(f, _)| f != folder && (folder.starts_with(f) || f.starts_with(folder)))
            .map(|(_, n)| n.clone())
            .collect();
        Far {
            reads: NetworkReads {
                limit: reads.limit,
                at_once: reads.at_once.max(1),
            },
            stuck,
            kin,
        }
    }

    /// Whether a read given up on is still out, so nothing new is asked.
    pub(crate) fn stalled(&self) -> bool {
        std::iter::once(&self.stuck)
            .chain(&self.kin)
            .any(|n| n.load(Ordering::SeqCst) > 0)
    }

    /// One read, waited for up to the limit.
    pub(crate) fn one<T: Send + 'static>(
        &self,
        read: impl FnOnce() -> T + Send + 'static,
    ) -> std::io::Result<T> {
        if self.stalled() {
            return Err(self.not_asked());
        }
        let job = self.start(read)?;
        self.finish(job)
    }

    /// `read` over each of `items`, at most `at_once` out at a time,
    /// the answers handed back in the order of `items`.
    pub(crate) fn many<I, T>(
        &self,
        items: Vec<I>,
        read: Arc<dyn Fn(I) -> T + Send + Sync>,
    ) -> Many<I, T>
    where
        I: Send + 'static,
        T: Send + 'static,
    {
        Many {
            far: self.clone(),
            read,
            todo: items.into(),
            out: VecDeque::new(),
        }
    }

    fn start<T: Send + 'static>(
        &self,
        read: impl FnOnce() -> T + Send + 'static,
    ) -> std::io::Result<Job<T>> {
        let (tell, answer) = mpsc::sync_channel(1);
        let state = Arc::new(AtomicU8::new(RUNNING));
        let (mine, stuck) = (state.clone(), self.stuck.clone());
        std::thread::Builder::new()
            .name("greycard index read".into())
            .spawn(move || {
                let _ = tell.send(read());
                if mine
                    .compare_exchange(RUNNING, DONE, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    // Given up on, and back after all: the share answers
                    // again, and the pass may ask it more.
                    stuck.fetch_sub(1, Ordering::SeqCst);
                }
            })?;
        Ok(Job {
            answer,
            state,
            started: Instant::now(),
        })
    }

    fn finish<T>(&self, job: Job<T>) -> std::io::Result<T> {
        let left = self.reads.limit.saturating_sub(job.started.elapsed());
        match job.answer.recv_timeout(left) {
            Ok(t) => Ok(t),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Counted before it is marked, so the thread's count
                // down, which comes only after the mark, never runs
                // ahead of it.
                self.stuck.fetch_add(1, Ordering::SeqCst);
                if job
                    .state
                    .compare_exchange(RUNNING, GIVEN_UP, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!(
                            "no answer from the network mount in {} s",
                            self.reads.limit.as_secs_f64()
                        ),
                    ));
                }
                // It answered as the limit passed.
                self.stuck.fetch_sub(1, Ordering::SeqCst);
                job.answer.recv().map_err(|_| ended())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ended()),
        }
    }

    fn not_asked(&self) -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!(
                "not read: an earlier read on the network mount has had no answer in {} s",
                self.reads.limit.as_secs_f64()
            ),
        )
    }
}

/// A read whose thread ended without an answer: it panicked.
fn ended() -> std::io::Error {
    std::io::Error::other("the read ended without an answer")
}

/// The reads of [`Far::many`], answered in order.
pub(crate) struct Many<I, T> {
    far: Far,
    read: Arc<dyn Fn(I) -> T + Send + Sync>,
    todo: VecDeque<I>,
    out: VecDeque<std::io::Result<Job<T>>>,
}

impl<I: Send + 'static, T: Send + 'static> Iterator for Many<I, T> {
    type Item = std::io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.out.len() < self.far.reads.at_once && !self.far.stalled() {
            let Some(item) = self.todo.pop_front() else {
                break;
            };
            let read = self.read.clone();
            self.out.push_back(self.far.start(move || read(item)));
        }
        match self.out.pop_front() {
            Some(Ok(job)) => Some(self.far.finish(job)),
            Some(Err(e)) => Some(Err(e)),
            // Stalled: the rest are not asked.
            None => self.todo.pop_front().map(|_| Err(self.far.not_asked())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn far(limit_ms: u64, at_once: usize, folder: &str) -> Far {
        Far::new(
            NetworkReads {
                limit: Duration::from_millis(limit_ms),
                at_once,
            },
            Path::new(folder),
        )
    }

    /// A read past the limit is an error, the reads after it are not
    /// asked while it is out, and the share is asked again once it is
    /// back.
    #[test]
    fn a_read_past_the_limit_is_an_error_and_holds_new_reads() {
        let f = far(100, 1, "/far-tests/stall");
        let (let_go, held) = mpsc::channel::<()>();
        let started = Instant::now();
        let err = f
            .one(move || {
                let _ = held.recv();
            })
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(f.stalled());
        // The next pass over the folder, and one over a folder under
        // it, ask nothing either, so no more threads are stranded;
        // another root is asked.
        assert!(far(100, 1, "/far-tests/stall").stalled());
        assert!(far(100, 1, "/far-tests/stall/day").stalled());
        assert!(!far(100, 1, "/far-tests/other").stalled());
        let asked = Arc::new(AtomicUsize::new(0));
        let a = asked.clone();
        let err = f
            .one(move || {
                a.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap_err();
        assert!(err.to_string().starts_with("not read"), "{err}");
        assert_eq!(
            asked.load(Ordering::SeqCst),
            0,
            "not asked while one is out"
        );
        let_go.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while f.stalled() {
            assert!(Instant::now() < until, "the stuck read came back");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(f.one(|| 7).unwrap(), 7);
        assert!(!far(100, 1, "/far-tests/stall/day").stalled());
    }

    /// No more than `at_once` reads are out at a time, and the answers
    /// come back in order.
    #[test]
    fn reads_are_bounded_and_in_order() {
        let f = far(5_000, 3, "/far-tests/bounded");
        let out = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let (o, m) = (out.clone(), most.clone());
        let read: Arc<dyn Fn(usize) -> usize + Send + Sync> = Arc::new(move |i| {
            let now = o.fetch_add(1, Ordering::SeqCst) + 1;
            m.fetch_max(now, Ordering::SeqCst);
            // Later items answer sooner, so order is the iterator's.
            std::thread::sleep(Duration::from_millis(30 - (i as u64 % 3) * 10));
            o.fetch_sub(1, Ordering::SeqCst);
            i * 2
        });
        let got: Vec<usize> = f
            .many((0..20).collect(), read)
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(got, (0..20).map(|i| i * 2).collect::<Vec<_>>());
        assert_eq!(most.load(Ordering::SeqCst), 3, "three out at most");
    }
}
