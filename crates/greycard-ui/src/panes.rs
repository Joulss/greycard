//! Tab over the loupe: the two side panes and the strip put away and
//! brought back. Tab puts away what is showing and brings back all it
//! put away, so a pane the user put away by hand stays away.
//!
//! The memory lives on `App` (`tab-kept`). A pane shown or put away
//! by hand (F6, F7, F8, its button) leaves it, so a later Tab never
//! undoes the hand; a pane shown by hand and still showing at the next
//! Tab joins it again. Over the grid Tab is the left pane's alone and
//! counts as a hand. The lightbox keeps the memory with the panes it
//! puts back. It is never saved: a session opened with all three away
//! has none, and Tab brings all three back.

use crate::{Panes, TabPanes};

const NONE: Panes = Panes {
    left: false,
    right: false,
    strip: false,
};
const ALL: Panes = Panes {
    left: true,
    right: true,
    strip: true,
};

/// Tab from `hidden`, with `kept` what earlier Tabs put away. Any
/// showing: all away, the showing added to the kept. All away: the
/// kept back, or all three when none is kept; nothing kept after.
pub(crate) fn tab(hidden: Panes, kept: Panes) -> TabPanes {
    if !(hidden.left && hidden.right && hidden.strip) {
        return TabPanes {
            hidden: ALL,
            kept: Panes {
                left: kept.left || !hidden.left,
                right: kept.right || !hidden.right,
                strip: kept.strip || !hidden.strip,
            },
        };
    }
    let back = if kept == NONE { ALL } else { kept };
    TabPanes {
        hidden: Panes {
            left: !back.left,
            right: !back.right,
            strip: !back.strip,
        },
        kept: NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(left: bool, right: bool, strip: bool) -> Panes {
        Panes { left, right, strip }
    }

    #[test]
    fn tab_brings_back_what_it_put_away() {
        // Every mix of three: Tab twice is where it started, all
        // three away excepted, which Tab twice leaves all showing.
        for bits in 0..8u8 {
            let start = p(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let one = tab(start.clone(), NONE);
            if start == ALL {
                assert_eq!(one.hidden, NONE, "all away, nothing kept: all back");
                assert_eq!(one.kept, NONE);
                continue;
            }
            assert_eq!(one.hidden, ALL, "{start:?}: all away");
            let two = tab(one.hidden, one.kept);
            assert_eq!(two.hidden, start, "{start:?}: back as it was");
            assert_eq!(two.kept, NONE);
        }
    }

    #[test]
    fn a_hide_adds_to_the_kept_and_a_show_is_never_none() {
        for h in 0..8u8 {
            for k in 0..8u8 {
                let hidden = p(h & 1 != 0, h & 2 != 0, h & 4 != 0);
                let kept = p(k & 1 != 0, k & 2 != 0, k & 4 != 0);
                let step = tab(hidden.clone(), kept.clone());
                if hidden == ALL {
                    assert_ne!(step.hidden, ALL, "{kept:?}: never a dead key");
                    assert_eq!(step.kept, NONE);
                } else {
                    assert_eq!(step.hidden, ALL);
                    let want = p(
                        kept.left || !hidden.left,
                        kept.right || !hidden.right,
                        kept.strip || !hidden.strip,
                    );
                    assert_eq!(step.kept, want, "{hidden:?} {kept:?}");
                }
            }
        }
        // Two put away by Tab, the third showing by hand: all three.
        let step = tab(p(true, false, true), p(true, false, true));
        assert_eq!(step.kept, ALL);
    }

    #[test]
    fn a_kept_set_is_what_comes_back() {
        let back = tab(ALL, p(false, false, true));
        assert_eq!(back.hidden, p(true, true, false));
        assert_eq!(back.kept, NONE);
    }
}
