# §468. The borrow that names both sides

*2026-10-31*

Item 5 of the layering review: the window's state is one `RefCell`
behind about 1490 mutable borrows, and when a callback holding it
makes a Slint call that fires another callback, the second borrow
panics with "already mutably borrowed" and nothing says which two
met.

**The wrapper.** `Shared` in the window crate holds the `RefCell`
and a cell for the holder's source location. Its `borrow_mut`,
`borrow`, `try_borrow_mut`, and `try_borrow` carry the caller's
location (`#[track_caller]`), so no site types a name that can
drift. The first two try, and on failure panic with both places:
"panel/mask.rs:1561 needs the state while panel/viewport.rs:722
holds it". A borrow that succeeds goes on record and its guard
clears the record on drop; the guards deref as `RefMut` and `Ref`
do, so every call site compiles as it was. A shared borrow records
the first reader only, and only that reader clears it; a writer
blocked by a reader names the outermost one, and if readers drop
out of order the message says "while a reader holds it", with no
place rather than a wrong one. The try variants never panic, so the
polling sites behave as before. The panic hook already writes the
message to the log.

**The type.** `Rc<RefCell<State>>` became `Rc<Shared>` at 305
mentions in 61 files; no borrow site, Slint file, or name of the
state changed. The roadmap line asked for the callback's name; a
borrow inside a helper reports the helper's line, which is a place
rather than the callback, and far more than nothing. A helper
marked `#[track_caller]` would pass the caller's line through; none
is yet.

**The cost.** 200 million borrows through the wrapper against the
bare `RefCell`, out of line: within noise either way, a cell write a
borrow.
