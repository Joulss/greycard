//! The session's rating, flag, label, keyword and title changes, and
//! its capture time shifts, for culling's undo.
//!
//! A develop edit has its history in the sidecar; the culling keys'
//! changes have none there, since a rating is a fact about the frame
//! and not a step in developing it (§117). So culling's Ctrl+Z steps
//! back through these instead: every change the keys, the frame menu
//! and the panel's Title and Keywords fields made this session,
//! newest first, and nothing else. Out of culling, Ctrl+Z steps
//! these too when one was done after the frame's last edit
//! ([`marks_first`]).
//!
//! Each step names its frames by path, not by row or index: the list
//! of files changes under it (a rejects move, a filter, a root added),
//! and a step whose frame has gone is simply passed over. A frame is
//! put back only while it still carries what the step left on it, so
//! a rating that arrived since from another tool's XMP, or a paste,
//! is not undone by a step that never made it.

use std::path::{Path, PathBuf};

use greycard_edit::Order;
use greycard_edit::meta::{Change, Flag, Label, Meta};

use crate::tr;

/// The fields the culling keys and the panel's words write, and the
/// capture time's shift, which the shift sheet writes down the same
/// path and takes back with the same undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    pub rating: u8,
    pub flag: Flag,
    pub label: Label,
    pub keywords: Vec<String>,
    pub title: String,
    pub time_shift: i64,
}

impl Tags {
    pub fn of(meta: &Meta) -> Self {
        Self {
            rating: meta.rating,
            flag: meta.flag,
            label: meta.label,
            keywords: meta.keywords.clone(),
            title: meta.title.clone(),
            time_shift: meta.time_shift,
        }
    }

    pub fn put(self, meta: &mut Meta) {
        meta.rating = self.rating;
        meta.flag = self.flag;
        meta.label = self.label;
        meta.keywords = self.keywords;
        meta.title = self.title;
        meta.set_time_shift(self.time_shift);
    }
}

/// The word over the picture for a key's change, once settled
/// against the frames it went on: what the frame carries now, not
/// the key that was pressed, so a label key that cleared the label
/// says so. The flag's past tense is the one a culler says aloud.
pub fn notice(change: &Change) -> String {
    match change {
        // The undo list's name for a change: 'Undo: No stars'
        Change::Rating(0) => tr!("No stars"),
        Change::Rating(n) => tr!("{n} star" | "{n} stars" % *n),
        Change::Flag(Flag::Pick) => tr!("Pick"),
        Change::Flag(Flag::Reject) => tr!("Rejected"),
        Change::Flag(Flag::None) => tr!("Unflagged"),
        Change::Label(Label::None) => tr!("No label"),
        // The label's name is marked for the catalog in `filter`.
        Change::Label(label) => crate::i18n::text(label.name()),
        Change::AddKeywords(words) => tr!(
            "Keyword {words}" | "Keywords {words}" % words.len(),
            words = words.join(", ")
        ),
        Change::RemoveKeyword(word) => tr!("No keyword {word}", word = word),
        Change::Title(text) if text.is_empty() => tr!("No title"),
        Change::Title(_) => tr!("Titled"),
        Change::TimeShift(by) => tr!("Capture time {}", signed(*by)),
        Change::ClearTimeShift => tr!("Capture time as the camera set it"),
    }
}

/// A shift as the info line and the notices write it: a sign, then
/// minutes and seconds, the hours in front when there are any: `+5:00`,
/// `-1:02:03`, `+0:00`.
pub fn signed(secs: i64) -> String {
    let sign = if secs < 0 { '-' } else { '+' };
    let a = secs.unsigned_abs();
    let (h, m, s) = (a / 3600, a / 60 % 60, a % 60);
    if h > 0 {
        format!("{sign}{h}:{m:02}:{s:02}")
    } else {
        format!("{sign}{m}:{s:02}")
    }
}

/// The status line's words for a key that took frames out of the
/// filtered list: what the key did, that the filter (`filter`, in its
/// own words) hid them, and the way back. `who` is the frame's name,
/// or how many there were. Without it a frame rated under a filter
/// that no longer wants it is gone from the strip and the grid with
/// no word, and reads as lost.
pub fn hidden_by_filter(change: &Change, who: &str, many: bool, filter: &str) -> String {
    let done = match change {
        // In 'N frames rated 3 and hidden by the filter'
        Change::Rating(n) => tr!("rated {n}", n = n),
        Change::Flag(Flag::Pick) => tr!("picked"),
        Change::Flag(Flag::Reject) => tr!("rejected"),
        Change::Flag(Flag::None) => tr!("unflagged"),
        Change::Label(Label::None) => tr!("unlabeled"),
        Change::Label(label) => tr!(
            "labeled {}",
            crate::i18n::text(&label.name().to_lowercase())
        ),
        Change::AddKeywords(words) => tr!("tagged {}", words.join(", ")),
        Change::RemoveKeyword(word) => tr!("untagged {word}", word = word),
        Change::Title(text) if text.is_empty() => tr!("untitled"),
        Change::Title(_) => tr!("titled"),
        Change::TimeShift(_) | Change::ClearTimeShift => tr!("moved in time"),
    };
    if many {
        tr!(
            "{who} {done} and hidden by the filter ({filter}); Clear the filter to see them",
            who = who,
            done = done,
            filter = filter
        )
    } else {
        tr!(
            "{who} {done} and hidden by the filter ({filter}); Clear the filter to see it",
            who = who,
            done = done,
            filter = filter
        )
    }
}

/// The one field that differs between `from` and `to`, as the change
/// that would take a frame from the one to the other: what an undo
/// or a redo did to it, in the key's own terms, for the word over
/// the picture. None when nothing differs; a step moves one field,
/// so a difference in more than one names the first found.
pub fn between(from: &Tags, to: &Tags) -> Option<Change> {
    let has = |words: &[String], w: &str| words.iter().any(|k| k.eq_ignore_ascii_case(w));
    if from.rating != to.rating {
        Some(Change::Rating(to.rating))
    } else if from.flag != to.flag {
        Some(Change::Flag(to.flag))
    } else if from.label != to.label {
        Some(Change::Label(to.label))
    } else if from.keywords != to.keywords {
        let added: Vec<String> = to
            .keywords
            .iter()
            .filter(|w| !has(&from.keywords, w))
            .cloned()
            .collect();
        if !added.is_empty() {
            return Some(Change::AddKeywords(added));
        }
        from.keywords
            .iter()
            .find(|w| !has(&to.keywords, w))
            .map(|w| Change::RemoveKeyword(w.clone()))
    } else if from.title != to.title {
        Some(Change::Title(to.title.clone()))
    } else if from.time_shift != to.time_shift {
        Some(match to.time_shift {
            0 => Change::ClearTimeShift,
            _ => Change::TimeShift(to.time_shift - from.time_shift),
        })
    } else {
        None
    }
}

/// The word for an undo (`back`) or a redo that put `landed` frames
/// back: "Undo: No stars" for one frame, what it carries now after
/// the key's own word, and a bare "Undone" for a step over several.
pub fn step_notice(back: bool, landed: &[Option<Change>]) -> String {
    match landed {
        [Some(change)] if back => tr!("Undo: {}", notice(change)),
        [Some(change)] => tr!("Redo: {}", notice(change)),
        _ if back => tr!("Undone"),
        _ => tr!("Redone"),
    }
}

/// One frame's part in a step.
#[derive(Debug, Clone, PartialEq)]
pub struct Moved {
    pub path: PathBuf,
    pub before: Tags,
    pub after: Tags,
}

/// One key's change, over every frame it moved.
pub type Step = Vec<Moved>;

/// The session's steps, each with when it was done on the clock the
/// edit's lines keep too ([`Order`]), so that undo outside culling can
/// take back whichever was done last, a mark or the frame's edit
/// ([`marks_first`]).
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<(Order, Step)>,
    redo: Vec<(Order, Step)>,
}

/// As many steps as a long culling pass takes and then some; the
/// oldest go past it.
const KEPT: usize = 1000;

impl History {
    /// A change just made. What was undone is gone, as in any undo.
    pub fn record(&mut self, step: Step) {
        if step.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push((Order::next(), step));
        if self.undo.len() > KEPT {
            self.undo.remove(0);
        }
    }

    /// The step to take back, with each frame going to its `before`;
    /// it moves to the redo side.
    pub fn undo(&mut self) -> Option<Step> {
        let (_, step) = self.undo.pop()?;
        self.redo.push((Order::next(), step.clone()));
        Some(step)
    }

    /// The step to make again, each frame going back to its `after`,
    /// given as an undo is: `before` the state it leaves, `after` the
    /// state it lands on.
    pub fn redo(&mut self) -> Option<Step> {
        let (_, step) = self.redo.pop()?;
        self.undo.push((Order::next(), step.clone()));
        Some(step.into_iter().map(Moved::reversed).collect())
    }

    /// Whether there is any step to undo or redo.
    pub fn any(&self) -> bool {
        !self.undo.is_empty() || !self.redo.is_empty()
    }

    /// When the step an undo would take back was last done (made, or
    /// redone); None when there is none.
    pub fn undo_order(&self) -> Option<Order> {
        self.undo.last().map(|(o, _)| *o)
    }

    /// When the step a redo would make again was undone.
    pub fn redo_order(&self) -> Option<Order> {
        self.redo.last().map(|(o, _)| *o)
    }
}

/// Whether an undo or a redo outside culling steps the session's
/// marks rather than the frame's edit. For an undo, `mark` and `edit`
/// are when the step each would take back was last done (made, or
/// redone); for a redo, when the step each would make again was
/// undone (see [`Order`]); each None when there is nothing of the kind
/// to step. The later goes, either way: an undo takes back what was
/// done last, a redo makes again what was undone last, so a Ctrl+Z
/// always takes back the Ctrl+Shift+Z before it and the other way
/// round. An edit from before the session is older than any mark.
pub fn marks_first(mark: Option<Order>, edit: Option<Order>) -> bool {
    match (mark, edit) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(m), Some(e)) => m.tick() > e.tick(),
    }
}

impl Moved {
    /// The same change the other way: undone becomes redone.
    fn reversed(self) -> Self {
        Self {
            path: self.path,
            before: self.after,
            after: self.before,
        }
    }
}

/// Where `step` would put each frame it still finds as it left it,
/// as (file index, tags to put); `find` looks a path up among the
/// files now listed. For an undo, `step` is as [`History::undo`] gave
/// it; a redo's is already turned round.
pub fn landing(
    step: &[Moved],
    find: impl Fn(&Path) -> Option<usize>,
    now: impl Fn(usize) -> Tags,
) -> Vec<(usize, Tags)> {
    step.iter()
        .filter_map(|m| {
            let i = find(&m.path)?;
            (now(i) == m.after).then(|| (i, m.before.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(rating: u8, flag: Flag) -> Tags {
        Tags {
            rating,
            flag,
            label: Label::None,
            keywords: Vec::new(),
            title: String::new(),
            time_shift: 0,
        }
    }

    fn moved(path: &str, before: &Tags, after: &Tags) -> Moved {
        Moved {
            path: path.into(),
            before: before.clone(),
            after: after.clone(),
        }
    }

    #[test]
    fn undo_and_redo_walk_the_steps_and_a_new_change_drops_the_redo() {
        let mut h = History::default();
        let (none, three, pick) = (
            tags(0, Flag::None),
            tags(3, Flag::None),
            tags(3, Flag::Pick),
        );
        h.record(vec![moved("a", &none, &three)]);
        h.record(vec![moved("a", &three, &pick)]);

        // The last first, each frame going to what it was.
        let back = h.undo().unwrap();
        assert_eq!((&back[0].before, &back[0].after), (&three, &pick));
        let back = h.undo().unwrap();
        assert_eq!((&back[0].before, &back[0].after), (&none, &three));
        assert!(h.undo().is_none());

        // A redo is given the way round it will be laid.
        let again = h.redo().unwrap();
        assert_eq!((&again[0].before, &again[0].after), (&three, &none));

        // A new change: the other undone step is gone for good.
        h.record(vec![moved("b", &none, &pick)]);
        assert!(h.redo().is_none());
        assert_eq!(h.undo().unwrap()[0].path, PathBuf::from("b"));
        assert_eq!(h.undo().unwrap()[0].path, PathBuf::from("a"));
        assert!(h.undo().is_none());
    }

    #[test]
    fn a_frame_changed_since_or_gone_is_left_alone() {
        let (none, three, five) = (
            tags(0, Flag::None),
            tags(3, Flag::None),
            tags(5, Flag::None),
        );
        let step = vec![
            moved("kept", &none, &three),
            moved("rated-since", &none, &three),
            moved("gone", &none, &three),
        ];
        let files = ["kept", "rated-since"];
        let find = |p: &Path| files.iter().position(|f| Path::new(f) == p);
        // "rated-since" has five stars now, from somewhere else.
        let now = |i: usize| if i == 0 { three.clone() } else { five.clone() };
        assert_eq!(landing(&step, find, now), vec![(0, none)]);
    }

    #[test]
    fn the_notice_says_what_the_frame_carries_now() {
        assert_eq!(notice(&Change::Rating(0)), "No stars");
        assert_eq!(notice(&Change::Rating(1)), "1 star");
        assert_eq!(notice(&Change::Rating(3)), "3 stars");
        assert_eq!(notice(&Change::Flag(Flag::Pick)), "Pick");
        assert_eq!(notice(&Change::Flag(Flag::Reject)), "Rejected");
        assert_eq!(notice(&Change::Flag(Flag::None)), "Unflagged");
        assert_eq!(notice(&Change::Label(Label::Red)), "Red");
        assert_eq!(notice(&Change::Label(Label::Blue)), "Blue");
        assert_eq!(notice(&Change::Label(Label::None)), "No label");
        assert_eq!(
            notice(&Change::AddKeywords(vec!["sea".into()])),
            "Keyword sea"
        );
        assert_eq!(
            notice(&Change::AddKeywords(vec!["sea".into(), "dusk".into()])),
            "Keywords sea, dusk"
        );
        assert_eq!(
            notice(&Change::RemoveKeyword("sea".into())),
            "No keyword sea"
        );
        assert_eq!(notice(&Change::Title("Dusk".into())), "Titled");
        assert_eq!(notice(&Change::Title(String::new())), "No title");
    }

    #[test]
    fn the_word_for_a_step_names_what_one_frame_carries_now() {
        let (none, three, pick) = (
            tags(0, Flag::None),
            tags(3, Flag::None),
            tags(3, Flag::Pick),
        );
        assert_eq!(between(&three, &none), Some(Change::Rating(0)));
        assert_eq!(between(&three, &pick), Some(Change::Flag(Flag::Pick)));
        assert_eq!(between(&pick, &pick), None);
        assert_eq!(
            step_notice(true, &[between(&three, &none)]),
            "Undo: No stars"
        );
        assert_eq!(
            step_notice(false, &[between(&none, &three)]),
            "Redo: 3 stars"
        );
        // A keyword put on or taken off, and a title, in their words.
        let sea = Tags {
            keywords: vec!["sea".into()],
            ..none.clone()
        };
        let titled = Tags {
            title: "Dusk".into(),
            ..none.clone()
        };
        assert_eq!(
            between(&none, &sea),
            Some(Change::AddKeywords(vec!["sea".into()]))
        );
        assert_eq!(
            step_notice(true, &[between(&sea, &none)]),
            "Undo: No keyword sea"
        );
        assert_eq!(
            step_notice(true, &[between(&titled, &none)]),
            "Undo: No title"
        );
        assert_eq!(
            step_notice(true, &[Some(Change::Rating(0)), Some(Change::Rating(0))]),
            "Undone"
        );
        assert_eq!(step_notice(false, &[None, None]), "Redone");
    }

    #[test]
    fn a_time_shift_is_a_step_with_words_of_its_own() {
        let none = tags(3, Flag::None);
        let shifted = Tags {
            time_shift: 300,
            ..none.clone()
        };
        let a_minute = Tags {
            time_shift: 60,
            ..none.clone()
        };
        assert_eq!(notice(&Change::TimeShift(300)), "Capture time +5:00");
        assert_eq!(
            notice(&Change::ClearTimeShift),
            "Capture time as the camera set it"
        );
        assert_eq!(between(&none, &shifted), Some(Change::TimeShift(300)));
        assert_eq!(between(&shifted, &none), Some(Change::ClearTimeShift));
        assert_eq!(
            step_notice(true, &[between(&shifted, &a_minute)]),
            "Undo: Capture time -4:00"
        );
        // The shift is put back with the rest of the tags.
        let mut meta = Meta::default();
        shifted.clone().put(&mut meta);
        assert_eq!((meta.rating, meta.time_shift), (3, 300));
        assert_eq!(Tags::of(&meta), shifted);
        let said = hidden_by_filter(&Change::TimeShift(60), "f.CR3", false, "date:2026");
        assert!(said.contains("moved in time"), "{said}");
    }

    #[test]
    fn the_later_of_a_mark_and_an_edit_steps_first() {
        let (before, edit, mark) = (Order::BEFORE, Order::next(), Order::next());
        // The later; the mark when there is no edit to step, and
        // never a mark that is not there.
        assert!(marks_first(Some(mark), Some(edit)));
        assert!(!marks_first(Some(edit), Some(mark)));
        assert!(marks_first(Some(mark), None));
        assert!(!marks_first(None, Some(edit)));
        assert!(!marks_first(None, None));
        // An edit from before the session is older than any mark.
        assert!(marks_first(Some(mark), Some(before)));
    }

    /// A step is stamped each time it is acted on: undone, then
    /// redone, each later than the last.
    #[test]
    fn a_step_is_stamped_when_undone_and_again_when_redone() {
        let mut h = History::default();
        let (none, three) = (tags(0, Flag::None), tags(3, Flag::None));
        assert!(h.undo_order().is_none());
        h.record(vec![moved("a", &none, &three)]);
        let done = h.undo_order().unwrap().tick();
        assert!(done > 0);
        h.undo();
        assert!(h.undo_order().is_none());
        let undone = h.redo_order().unwrap().tick();
        assert!(undone > done);
        h.redo();
        assert!(h.undo_order().unwrap().tick() > undone);
        assert!(h.redo_order().is_none());
    }

    #[test]
    fn an_empty_change_is_not_a_step() {
        let mut h = History::default();
        h.record(Vec::new());
        assert!(h.undo().is_none());
    }
}
