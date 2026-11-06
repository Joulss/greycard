//! The UNet held open between removals, one a process: in the
//! [`super::Memory`] modes that keep it, every [`super::Remove`] takes
//! it from here and gives it back, so a UNet opened ahead of the first
//! removal ([`super::prepare`]) is the one that removal runs, and a
//! second is never opened beside it. Keyed by the graph's path, which
//! names the sampler and the size.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

/// Where the held one is.
enum State<T> {
    /// Being opened, on whichever thread asked first: the others wait
    /// for it rather than open a second.
    Opening,
    /// Open and free.
    Ready(T),
    /// Out on the lease with this ticket.
    Lent(u64),
}

struct Entry<T> {
    path: PathBuf,
    state: State<T>,
}

/// One held value and who has it.
pub(crate) struct Slot<T> {
    entry: Mutex<Option<Entry<T>>>,
    changed: Condvar,
    tickets: AtomicU64,
}

/// A value out of the [`Slot`], given back when dropped: kept as the
/// slot's when it is still the one lent, or the slot is empty, and
/// dropped otherwise, so the slot never holds two.
pub(crate) struct Lease<'a, T> {
    slot: &'a Slot<T>,
    path: PathBuf,
    value: Option<T>,
    /// `None` for one opened beside a value already lent.
    ticket: Option<u64>,
}

impl<T> Lease<'_, T> {
    pub(crate) fn value(&mut self) -> &mut T {
        self.value
            .as_mut()
            .expect("a lease holds its value until dropped")
    }
}

impl<T> Drop for Lease<'_, T> {
    fn drop(&mut self) {
        let Some(value) = self.value.take() else {
            return;
        };
        let mut entry = self.slot.lock();
        let kept = match entry.as_mut() {
            Some(e)
                if e.path == self.path
                    && self
                        .ticket
                        .is_some_and(|t| matches!(e.state, State::Lent(l) if l == t)) =>
            {
                e.state = State::Ready(value);
                None
            }
            None => {
                *entry = Some(Entry {
                    path: self.path.clone(),
                    state: State::Ready(value),
                });
                None
            }
            Some(_) => Some(value),
        };
        drop(entry);
        self.slot.changed.notify_all();
        drop(kept);
    }
}

impl<T> Slot<T> {
    pub(crate) const fn new() -> Self {
        Self {
            entry: Mutex::new(None),
            changed: Condvar::new(),
            tickets: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Entry<T>>> {
        self.entry.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The value at `path`: the one held, once any open in flight has
    /// finished, else one `open` makes. One held for another path is
    /// dropped first. While the held one is lent, `open` makes another
    /// beside it, as two removals at once would have each made their
    /// own.
    pub(crate) fn acquire<E>(
        &self,
        path: &Path,
        open: impl FnOnce() -> std::result::Result<T, E>,
    ) -> std::result::Result<Lease<'_, T>, E> {
        let mut entry = self.lock();
        loop {
            match entry.as_mut() {
                None => break,
                Some(Entry {
                    state: State::Opening,
                    ..
                }) => {
                    entry = self.changed.wait(entry).unwrap_or_else(|e| e.into_inner());
                }
                Some(e) if e.path == path && matches!(e.state, State::Ready(_)) => {
                    let ticket = self.ticket();
                    let State::Ready(value) = std::mem::replace(&mut e.state, State::Lent(ticket))
                    else {
                        unreachable!("matched as ready")
                    };
                    return Ok(self.lease(path, value, Some(ticket)));
                }
                Some(Entry {
                    state: State::Lent(_),
                    ..
                }) => {
                    drop(entry);
                    return open().map(|value| self.lease(path, value, None));
                }
                Some(_) => {
                    // Ready for another path: let go before opening.
                    let old = entry.take();
                    drop(entry);
                    drop(old);
                    entry = self.lock();
                }
            }
        }
        let (mut entry, opened) = self.open_at(entry, path, open);
        let out = match opened {
            Ok(value) => {
                let ticket = self.ticket();
                *entry = Some(Entry {
                    path: path.to_path_buf(),
                    state: State::Lent(ticket),
                });
                Ok(self.lease(path, value, Some(ticket)))
            }
            Err(e) => {
                *entry = None;
                Err(e)
            }
        };
        drop(entry);
        self.changed.notify_all();
        out
    }

    /// `open` made and held at `path`, for a later [`Slot::acquire`]:
    /// false, and nothing opened, when it is held or being opened
    /// already, or another is lent. One held for another path is
    /// dropped first; one being opened is waited for.
    pub(crate) fn prepare<E>(
        &self,
        path: &Path,
        open: impl FnOnce() -> std::result::Result<T, E>,
    ) -> std::result::Result<bool, E> {
        let mut entry = self.lock();
        loop {
            match entry.as_mut() {
                None => break,
                Some(e) if e.path == path => return Ok(false),
                Some(Entry {
                    state: State::Opening,
                    ..
                }) => {
                    entry = self.changed.wait(entry).unwrap_or_else(|e| e.into_inner());
                }
                Some(Entry {
                    state: State::Lent(_),
                    ..
                }) => return Ok(false),
                Some(_) => {
                    let old = entry.take();
                    drop(entry);
                    drop(old);
                    entry = self.lock();
                }
            }
        }
        let (mut entry, opened) = self.open_at(entry, path, open);
        let out = match opened {
            Ok(value) => {
                *entry = Some(Entry {
                    path: path.to_path_buf(),
                    state: State::Ready(value),
                });
                Ok(true)
            }
            Err(e) => {
                *entry = None;
                Err(e)
            }
        };
        drop(entry);
        self.changed.notify_all();
        out
    }

    /// `open` run with the entry marked as opening at `path`, and the
    /// lock taken again after it. Should `open` panic, the mark is
    /// taken down and the waiters woken, so the next asks afresh
    /// rather than wait on an open that never ends.
    fn open_at<'a, E>(
        &'a self,
        mut entry: MutexGuard<'a, Option<Entry<T>>>,
        path: &Path,
        open: impl FnOnce() -> std::result::Result<T, E>,
    ) -> (MutexGuard<'a, Option<Entry<T>>>, std::result::Result<T, E>) {
        /// Takes the mark down unless the open came back.
        struct Unwound<'a, T> {
            slot: &'a Slot<T>,
            finished: bool,
        }
        impl<T> Drop for Unwound<'_, T> {
            fn drop(&mut self) {
                if !self.finished {
                    *self.slot.lock() = None;
                    self.slot.changed.notify_all();
                }
            }
        }
        *entry = Some(Entry {
            path: path.to_path_buf(),
            state: State::Opening,
        });
        drop(entry);
        let mut unwound = Unwound {
            slot: self,
            finished: false,
        };
        let opened = open();
        unwound.finished = true;
        (self.lock(), opened)
    }

    /// Whether one is held or being opened at `path`.
    pub(crate) fn holds(&self, path: &Path) -> bool {
        self.lock().as_ref().is_some_and(|e| e.path == path)
    }

    /// Whether one is open at `path`, held or lent: not one still being
    /// opened, whose weights are only partly on the card.
    #[cfg(test)]
    pub(crate) fn holds_open(&self, path: &Path) -> bool {
        self.lock()
            .as_ref()
            .is_some_and(|e| e.path == path && !matches!(e.state, State::Opening))
    }

    /// [`Slot::holds_open`], once an open of `path` in flight has
    /// finished, failed, or panicked, or `bound` has passed, when one
    /// still opening counts as not open: a hung open never holds the
    /// caller for good.
    pub(crate) fn wait_open(&self, path: &Path, bound: std::time::Duration) -> bool {
        let until = std::time::Instant::now() + bound;
        let mut entry = self.lock();
        while entry
            .as_ref()
            .is_some_and(|e| e.path == path && matches!(e.state, State::Opening))
        {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return false;
            }
            entry = self
                .changed
                .wait_timeout(entry, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        entry
            .as_ref()
            .is_some_and(|e| e.path == path && !matches!(e.state, State::Opening))
    }

    /// The one held and free let go unless `keep` says its path is
    /// wanted: true when one went.
    pub(crate) fn let_go_unless(&self, keep: impl Fn(&Path) -> bool) -> bool {
        let mut entry = self.lock();
        let old = match entry.as_ref() {
            Some(e) if matches!(e.state, State::Ready(_)) && !keep(&e.path) => entry.take(),
            _ => None,
        };
        drop(entry);
        old.is_some()
    }

    fn ticket(&self) -> u64 {
        self.tickets.fetch_add(1, Ordering::Relaxed)
    }

    fn lease(&self, path: &Path, value: T, ticket: Option<u64>) -> Lease<'_, T> {
        Lease {
            slot: self,
            path: path.to_path_buf(),
            value: Some(value),
            ticket,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    /// A value that counts how many of its kind are alive.
    struct Counted(Arc<AtomicUsize>, u32);

    impl Counted {
        fn new(alive: &Arc<AtomicUsize>, n: u32) -> Self {
            alive.fetch_add(1, Ordering::SeqCst);
            Counted(alive.clone(), n)
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// A prepared value is the one the next acquire at its path gets,
    /// with no open; given back it stays; another path lets it go
    /// before opening, so two are never alive at once.
    #[test]
    fn a_prepared_value_is_taken_and_another_path_lets_it_go() {
        let slot = Slot::<Counted>::new();
        let alive = Arc::new(AtomicUsize::new(0));
        let (a, b) = (Path::new("unet-1024.onnx"), Path::new("unet-512.onnx"));
        assert_eq!(
            slot.prepare(a, || Ok::<_, ()>(Counted::new(&alive, 1))),
            Ok(true)
        );
        assert!(slot.holds(a));
        assert_eq!(
            slot.prepare(a, || -> Result<Counted, ()> { panic!("held already") }),
            Ok(false)
        );
        {
            let mut lease = slot
                .acquire(a, || -> Result<Counted, ()> {
                    panic!("prepared, not opened again")
                })
                .unwrap();
            assert_eq!(lease.value().1, 1);
        }
        assert_eq!(alive.load(Ordering::SeqCst), 1, "given back, kept");
        {
            let mut lease = slot
                .acquire(b, || {
                    assert_eq!(alive.load(Ordering::SeqCst), 0, "let go before the open");
                    Ok::<_, ()>(Counted::new(&alive, 2))
                })
                .unwrap();
            assert_eq!(lease.value().1, 2);
        }
        assert!(slot.holds(b) && !slot.holds(a));
        assert!(!slot.let_go_unless(|p| p == b));
        assert!(slot.let_go_unless(|p| p == a));
        assert_eq!(alive.load(Ordering::SeqCst), 0);
    }

    /// An acquire while the prepare is still opening waits for it and
    /// takes its value instead of opening a second.
    #[test]
    fn an_acquire_waits_for_the_open_in_flight() {
        let slot = Arc::new(Slot::<u32>::new());
        let path = Path::new("unet-lcm-1024.onnx");
        let (started, release) = (std::sync::mpsc::channel(), std::sync::mpsc::channel::<()>());
        let preparing = {
            let slot = slot.clone();
            let (started, release) = (started.0, release.1);
            std::thread::spawn(move || {
                slot.prepare(path, || {
                    started.send(()).unwrap();
                    release.recv().unwrap();
                    Ok::<_, ()>(7)
                })
            })
        };
        started.1.recv().unwrap();
        let acquiring = {
            let slot = slot.clone();
            std::thread::spawn(move || {
                *slot
                    .acquire(path, || -> Result<u32, ()> { panic!("a second open") })
                    .unwrap()
                    .value()
            })
        };
        release.0.send(()).unwrap();
        assert_eq!(preparing.join().unwrap(), Ok(true));
        assert_eq!(acquiring.join().unwrap(), 7);
    }

    /// A failed open leaves nothing held, and one lent is not opened
    /// over: a second acquire makes its own, dropped when given back.
    #[test]
    fn a_failure_holds_nothing_and_a_second_beside_a_lent_one_is_dropped() {
        let slot = Slot::<Counted>::new();
        let alive = Arc::new(AtomicUsize::new(0));
        let path = Path::new("unet-1024.onnx");
        assert_eq!(
            slot.prepare(path, || Err::<Counted, _>("no adapter")),
            Err("no adapter")
        );
        assert!(!slot.holds(path));
        let first = slot
            .acquire(path, || Ok::<_, ()>(Counted::new(&alive, 1)))
            .unwrap();
        assert_eq!(
            slot.prepare(path, || -> Result<Counted, ()> { panic!("lent") }),
            Ok(false)
        );
        let second = slot
            .acquire(path, || Ok::<_, ()>(Counted::new(&alive, 2)))
            .unwrap();
        assert_eq!(alive.load(Ordering::SeqCst), 2);
        drop(second);
        assert_eq!(
            alive.load(Ordering::SeqCst),
            1,
            "the one beside is not kept"
        );
        drop(first);
        assert_eq!(alive.load(Ordering::SeqCst), 1, "the slot's own is");
        assert!(slot.holds(path));
    }

    /// An open that panics leaves nothing marked as opening: the next
    /// acquire opens afresh instead of waiting for good, and a prepare
    /// after a panic does too.
    #[test]
    fn an_open_that_panics_leaves_the_next_to_open_afresh() {
        let slot = Slot::<u32>::new();
        let path = Path::new("unet-1024.onnx");
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = slot.acquire(path, || -> Result<u32, ()> {
                panic!("the provider fell over")
            });
        }));
        assert!(panicked.is_err());
        assert!(!slot.holds(path));
        assert_eq!(*slot.acquire(path, || Ok::<_, ()>(3)).unwrap().value(), 3);
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = slot.prepare(Path::new("unet-512.onnx"), || -> Result<u32, ()> {
                panic!("the provider fell over")
            });
        }));
        assert!(panicked.is_err());
        assert!(!slot.holds(Path::new("unet-512.onnx")));
        assert_eq!(
            slot.prepare(Path::new("unet-512.onnx"), || Ok::<_, ()>(5)),
            Ok(true)
        );
    }

    /// A wait for an open in flight wakes when it lands, when it fails,
    /// and when it panics, and gives up at its bound.
    #[test]
    fn a_wait_for_an_open_wakes_on_its_end_however_it_ends() {
        use std::time::Duration;
        let path = Path::new("unet-512.onnx");
        let run = |open: Box<dyn FnOnce() -> Result<u32, ()> + Send>| -> bool {
            let slot = Arc::new(Slot::<u32>::new());
            let (started, go) = (
                Arc::new(std::sync::Barrier::new(2)),
                Arc::new(std::sync::Barrier::new(2)),
            );
            let opener = {
                let (slot, started, go) = (slot.clone(), started.clone(), go.clone());
                std::thread::spawn(move || {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        slot.prepare(path, || {
                            started.wait();
                            go.wait();
                            open()
                        })
                    }));
                })
            };
            started.wait();
            assert!(!slot.holds_open(path), "not while opening");
            let waiter = {
                let slot = slot.clone();
                std::thread::spawn(move || slot.wait_open(path, Duration::from_secs(30)))
            };
            go.wait();
            let held = waiter.join().unwrap();
            opener.join().unwrap();
            held
        };
        assert!(run(Box::new(|| Ok(1))), "opened");
        assert!(!run(Box::new(|| Err(()))), "failed");
        assert!(
            !run(Box::new(|| panic!("the provider fell over"))),
            "panicked"
        );
        // Bounded: an open that never ends is not waited on for good.
        let slot = Arc::new(Slot::<u32>::new());
        let started = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let opener = {
            let (slot, started, release) = (slot.clone(), started.clone(), release.clone());
            std::thread::spawn(move || {
                slot.prepare(path, || {
                    started.wait();
                    release.wait();
                    Ok::<_, ()>(2)
                })
            })
        };
        started.wait();
        assert!(!slot.wait_open(path, Duration::from_millis(50)));
        release.wait();
        opener.join().unwrap().unwrap();
        assert!(slot.wait_open(path, Duration::from_millis(50)));
    }
}
