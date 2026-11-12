//! A DCP camera profile, which the user picks from disk: the reader,
//! the header a listing reads, the file written back, and the map and
//! look table run over a fixed set of pixels as the develop runs them.
//! Never a panic; tables within their caps, the offset sane, every
//! pixel finite. Each input is read twice: as a file, and laid over a
//! small synthetic profile (its first two bytes say where), so a run
//! explores a well-formed file with no profile in the corpus.
//! `greycard_core::testing::dcp` has the checks.
#![no_main]

use greycard_core::testing::dcp::{check, patched};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    check(data);
    check(&patched(data));
});
