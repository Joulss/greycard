//! The window's state behind its one borrow, with the place that took
//! it written down. Every Slint callback borrows the same `State`, and
//! a Slint call made while one holds it can fire another that borrows
//! again; a bare `RefCell` then panics with "already mutably borrowed"
//! and nothing says which two met. Here each borrow records its
//! caller's place (`#[track_caller]`), so the panic names both sides
//! and no call site carries a name by hand that could drift from it.

use std::cell::{BorrowError, BorrowMutError, Cell, Ref, RefCell, RefMut};
use std::ops::{Deref, DerefMut};
use std::panic::Location;

use crate::State;

/// Where a borrow was taken.
type Place = &'static Location<'static>;

/// A `RefCell` that knows who holds it. Generic only so its tests need
/// no window; the editor's is the default, `Shared<State>`.
pub(crate) struct Shared<T = State> {
    value: RefCell<T>,
    /// The writer while one holds the value; else the first reader
    /// still holding it, if any. Readers after the first are not
    /// recorded: a shared borrow under another is legal and common
    /// (a helper reading what its caller reads), and the first is the
    /// outermost, the one a clash report wants.
    holder: Cell<Option<Place>>,
}

impl<T> Shared<T> {
    pub(crate) fn new(value: T) -> Self {
        Shared {
            value: RefCell::new(value),
            holder: Cell::new(None),
        }
    }

    /// The value to change, or a panic that names the asker and the
    /// holder.
    #[track_caller]
    pub(crate) fn borrow_mut(&self) -> HeldMut<'_, T> {
        match self.try_borrow_mut() {
            Ok(held) => held,
            Err(_) => self.clash(),
        }
    }

    /// The value to read, or a panic that names the asker and the
    /// writer holding it.
    #[track_caller]
    pub(crate) fn borrow(&self) -> HeldRef<'_, T> {
        match self.try_borrow() {
            Ok(held) => held,
            Err(_) => self.clash(),
        }
    }

    /// A mutable borrow, or the error, never a panic: the polling
    /// sites ask and go on. On success the caller is on record.
    #[track_caller]
    pub(crate) fn try_borrow_mut(&self) -> Result<HeldMut<'_, T>, BorrowMutError> {
        let value = self.value.try_borrow_mut()?;
        self.holder.set(Some(Location::caller()));
        Ok(HeldMut {
            value,
            holder: &self.holder,
        })
    }

    /// A shared borrow, or the error, never a panic. The first reader
    /// is on record, as under [`Shared::borrow`].
    #[track_caller]
    pub(crate) fn try_borrow(&self) -> Result<HeldRef<'_, T>, BorrowError> {
        let value = self.value.try_borrow()?;
        let first = self.holder.get().is_none();
        if first {
            self.holder.set(Some(Location::caller()));
        }
        Ok(HeldRef {
            value,
            holder: first.then_some(&self.holder),
        })
    }

    /// Kept out of line so the borrows themselves stay small.
    #[cold]
    #[inline(never)]
    #[track_caller]
    fn clash(&self) -> ! {
        let asker = Location::caller();
        match self.holder.get() {
            Some(holder) => panic!(
                "{}:{} needs the state while {}:{} holds it",
                asker.file(),
                asker.line(),
                holder.file(),
                holder.line()
            ),
            // A later reader outlived the first, which cleared the
            // record when it went.
            None => panic!(
                "{}:{} needs the state while a reader holds it",
                asker.file(),
                asker.line()
            ),
        }
    }
}

/// A writer's hold on the value; clears the record when it goes.
pub(crate) struct HeldMut<'a, T = State> {
    value: RefMut<'a, T>,
    holder: &'a Cell<Option<Place>>,
}

impl<T> Drop for HeldMut<'_, T> {
    fn drop(&mut self) {
        self.holder.set(None);
    }
}

impl<T> Deref for HeldMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for HeldMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// A reader's hold on the value; the first reader clears the record
/// when it goes, the others never wrote one.
pub(crate) struct HeldRef<'a, T = State> {
    value: Ref<'a, T>,
    holder: Option<&'a Cell<Option<Place>>>,
}

impl<T> Drop for HeldRef<'_, T> {
    fn drop(&mut self) {
        if let Some(holder) = self.holder {
            holder.set(None);
        }
    }
}

impl<T> Deref for HeldRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// What a closure that should panic said. Each borrow under test
    /// shares its line with a `line!()` in the same tuple, so the
    /// lines asserted on cannot drift from the calls.
    fn said<R>(f: impl FnOnce() -> R) -> String {
        let payload = catch_unwind(AssertUnwindSafe(f))
            .err()
            .expect("the borrow panics");
        payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn a_second_writer_names_itself_and_the_holder() {
        let shared = Shared::new(0u32);
        let (mut held, outer) = (shared.borrow_mut(), line!());
        *held += 1;
        let inner = Cell::new(0);
        let message = said(|| (inner.set(line!()), shared.borrow_mut()));
        assert_eq!(
            message,
            format!(
                "{f}:{} needs the state while {f}:{outer} holds it",
                inner.get(),
                f = file!()
            )
        );
        drop(held);
        assert_eq!(*shared.borrow(), 1);
    }

    #[test]
    fn a_reader_under_a_writer_names_the_writer_and_a_writer_under_a_reader_the_reader() {
        let shared = Shared::new(0u32);
        let (held, writer) = (shared.borrow_mut(), line!());
        let reader = Cell::new(0);
        let message = said(|| (reader.set(line!()), shared.borrow()));
        let (f, r) = (file!(), reader.get());
        assert_eq!(
            message,
            format!("{f}:{r} needs the state while {f}:{writer} holds it")
        );
        drop(held);

        let (read, first) = (shared.borrow(), line!());
        // A second reader is fine and leaves the first on record.
        let again = shared.borrow();
        let message = said(|| shared.borrow_mut());
        assert!(
            message.ends_with(&format!("while {f}:{first} holds it")),
            "{message}"
        );
        drop((read, again));
    }

    #[test]
    fn the_record_clears_when_the_holder_goes() {
        let shared = Shared::new(0u32);
        assert!(shared.holder.get().is_none());
        {
            let (_held, at) = (shared.borrow_mut(), line!());
            assert_eq!(shared.holder.get().map(|l| l.line()), Some(at));
        }
        assert!(shared.holder.get().is_none());
        {
            let _read = shared.borrow();
            let _again = shared.try_borrow().unwrap();
        }
        assert!(shared.holder.get().is_none());
        // A clash unwinds through the holder, which clears it too.
        said(|| {
            let _held = shared.borrow_mut();
            shared.borrow_mut()
        });
        assert!(shared.holder.get().is_none());
        assert!(shared.try_borrow_mut().is_ok());
    }
}
