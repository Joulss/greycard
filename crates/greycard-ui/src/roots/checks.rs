//! The roots' looks: whether each root answers, one look a root at
//! most.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

#[cfg(test)]
use super::tests;
use super::{Checks, ROOT_WAIT, online};

/// The editor's looks at the roots.
static CHECKS: LazyLock<Checks> = LazyLock::new(|| Checks::new(online));

/// Which roots are offline, by [`Checks::offline`].
pub(super) fn offline_of(roots: &[PathBuf], wait: Duration) -> HashSet<PathBuf> {
    #[cfg(test)]
    let (roots, mut hung): (Vec<PathBuf>, HashSet<PathBuf>) = {
        let (hung, rest): (Vec<PathBuf>, Vec<PathBuf>) =
            roots.iter().cloned().partition(|r| tests::not_answering(r));
        (rest, hung.into_iter().collect())
    };
    #[cfg(not(test))]
    let (roots, mut hung) = (roots.to_vec(), HashSet::new());
    // Every root looked at together, each on a thread of its own, the
    // wait shared.
    hung.extend(CHECKS.offline(roots.as_slice(), wait));
    hung
}

/// Whether `path` is there, by the editor's looks ([`Checks::answers`]):
/// `None` when it has not answered within `wait`, or its last look is
/// still out. In a test a path the test names does not answer, without
/// a look.
pub(crate) fn answer_of(path: &Path, wait: Duration) -> Option<bool> {
    // In a test, in place: the clock's wait under a loaded machine is
    // no test's business, and a path that does not answer is said so.
    #[cfg(test)]
    return {
        let _ = wait;
        tests::LOOKED.with(|l| l.borrow_mut().push(path.to_path_buf()));
        (!tests::not_answering(path)).then(|| online(path))
    };
    #[cfg_attr(test, allow(unreachable_code))]
    CHECKS
        .answers(&[path.to_path_buf()], wait)
        .remove(path)
        .flatten()
}

/// Each of `paths` looked at as [`answer_of`] looks: whether it is
/// there, or `None` when it has not answered within `wait` (or its last
/// look is still out). At most [`LOOKS_AT_ONCE`] at a time, each batch
/// with the wait shared; once one in a batch has not answered, the rest
/// are not looked at and are taken for no answer, so the whole holds its
/// caller for one wait at most and never asks for more threads than that.
pub(crate) fn answers_of(paths: &[PathBuf], wait: Duration) -> HashMap<PathBuf, Option<bool>> {
    let mut answers = HashMap::new();
    let mut quiet = false;
    for batch in paths.chunks(LOOKS_AT_ONCE) {
        if quiet {
            answers.extend(batch.iter().map(|p| (p.clone(), None)));
            continue;
        }
        #[cfg(test)]
        let got: HashMap<PathBuf, Option<bool>> = batch
            .iter()
            .map(|p| (p.clone(), answer_of(p, wait)))
            .collect();
        #[cfg(not(test))]
        let got = CHECKS.answers(batch, wait);
        quiet = got.values().any(Option::is_none);
        answers.extend(got);
    }
    answers
}

/// How many paths [`answers_of`] looks at together.
const LOOKS_AT_ONCE: usize = 16;

/// Whether `root` answers the roots' look: up to 3 s, then a second
/// try on the same look up to [`PASS_PATIENCE`] from when it was asked,
/// and at once no while a look older than that is still out. The
/// indexer asks it before each pass over a root, on its own thread. A
/// root slow to answer on a loaded machine is not a share gone away,
/// and a pass skipped waits for the next launch or change; a share that
/// is gone costs the second wait once, since its look is out after.
pub(crate) fn answers(root: &Path) -> bool {
    answer_of(root, ROOT_WAIT)
        .or_else(|| answer_of(root, PASS_PATIENCE))
        .unwrap_or(false)
}

/// How long the indexer's look at a root waits in all before its pass
/// is skipped.
pub(crate) const PASS_PATIENCE: Duration = Duration::from_secs(6);
