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
//! ([`marks_first`]); in Develop only those on the open frame
//! ([`History::reach`]).
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
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// The id the next step recorded takes.
    next_id: u64,
    /// The step last recorded, undone, or redone.
    last: Option<u64>,
    /// That step took the window off its frame (the filter hid it):
    /// the next open lands where it carried the window
    /// ([`Self::opened`]).
    carrying: Option<u64>,
    /// The step that carried the window off its frame, and the file it
    /// landed on: while the window stays there, Develop's undo and redo
    /// reach that step as the open frame's own ([`Self::reach`]).
    carried: Option<(u64, PathBuf)>,
}

/// A step in the history: when it was last acted on, and which it is.
#[derive(Debug)]
struct Entry {
    order: Order,
    id: u64,
    step: Step,
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
        let id = self.next_id;
        self.next_id += 1;
        self.undo.push(Entry {
            order: Order::next(),
            id,
            step,
        });
        self.acted(id);
        if self.undo.len() > KEPT {
            self.undo.remove(0);
        }
    }

    /// A step just acted on: the one a carry would be by.
    fn acted(&mut self, id: u64) {
        self.last = Some(id);
        self.carrying = None;
    }

    /// The step to take back, with each frame going to its `before`;
    /// it moves to the redo side.
    pub fn undo(&mut self) -> Option<Step> {
        self.undo_at(self.undo.len().checked_sub(1)?)
    }

    /// The undo side's step `at` (0 the oldest) taken back, the steps
    /// after it left where they are: Develop's undo of the open frame's
    /// mark past marks on other frames ([`Self::reach`]).
    pub fn undo_at(&mut self, at: usize) -> Option<Step> {
        if at >= self.undo.len() {
            return None;
        }
        let Entry { id, step, .. } = self.undo.remove(at);
        self.redo.push(Entry {
            order: Order::next(),
            id,
            step: step.clone(),
        });
        self.acted(id);
        Some(step)
    }

    /// The step to make again, each frame going back to its `after`,
    /// given as an undo is: `before` the state it leaves, `after` the
    /// state it lands on.
    pub fn redo(&mut self) -> Option<Step> {
        self.redo_at(self.redo.len().checked_sub(1)?)
    }

    /// The redo side's step `at` (0 the first undone of those left)
    /// made again, as [`Self::undo_at`].
    pub fn redo_at(&mut self, at: usize) -> Option<Step> {
        if at >= self.redo.len() {
            return None;
        }
        let Entry { id, step, .. } = self.redo.remove(at);
        self.undo.push(Entry {
            order: Order::next(),
            id,
            step: step.clone(),
        });
        self.acted(id);
        Some(step.into_iter().map(Moved::reversed).collect())
    }

    /// Whether there is any step to undo or redo.
    pub fn any(&self) -> bool {
        !self.undo.is_empty() || !self.redo.is_empty()
    }

    /// When the step an undo would take back was last done (made, or
    /// redone); None when there is none.
    pub fn undo_order(&self) -> Option<Order> {
        self.undo.last().map(|e| e.order)
    }

    /// When the step a redo would make again was undone.
    pub fn redo_order(&self) -> Option<Order> {
        self.redo.last().map(|e| e.order)
    }

    /// The step on the undo (`back`) or redo side at `at`, with when it
    /// was last acted on.
    pub fn at(&self, back: bool, at: usize) -> Option<(Order, &Step)> {
        self.side(back).get(at).map(|e| (e.order, &e.step))
    }

    /// The undo side (`back`) or the redo side.
    fn side(&self, back: bool) -> &Vec<Entry> {
        if back { &self.undo } else { &self.redo }
    }

    /// The step Develop may take on the undo (`back`) or redo side,
    /// with `file` open: the latest that moved `file`, or that carried
    /// the window off its frame onto `file` while the window has stayed
    /// there. Steps on other frames after it are passed over and stay
    /// as they are, in their order.
    ///
    /// Whole, unless a later step on the same side names one of its
    /// frames: then that frame's share must wait for the later step,
    /// and only `file`'s share is taken (for a carried step, the
    /// shares no later step names). Decided by the history, not by
    /// what the frames carry now: a frame put back to the same value by
    /// later steps is still theirs to undo first.
    pub fn reach(&self, back: bool, file: &Path) -> Option<Reach> {
        let side = self.side(back);
        let carried = self
            .carried
            .as_ref()
            .and_then(|(id, to)| (to == file).then_some(*id));
        for at in (0..side.len()).rev() {
            let entry = &side[at];
            let names = entry.step.iter().any(|m| m.path == file);
            if !names && Some(entry.id) != carried {
                continue;
            }
            let later = &side[at + 1..];
            let free: Vec<PathBuf> = entry
                .step
                .iter()
                .map(|m| &m.path)
                .filter(|p| !later.iter().any(|l| l.step.iter().any(|m| &m.path == *p)))
                .cloned()
                .collect();
            if free.len() == entry.step.len() {
                return Some(Reach { at, part: None });
            }
            if names {
                // No later step names `file`, or it would be found first.
                return Some(Reach {
                    at,
                    part: Some(vec![file.to_path_buf()]),
                });
            }
            if !free.is_empty() {
                return Some(Reach {
                    at,
                    part: Some(free),
                });
            }
        }
        None
    }

    /// Take the step [`Self::reach`] found: undone (`back`) or redone,
    /// whole, or its share for `part`'s frames alone. A share is a step
    /// of its own on the other side; the rest stays where the step was,
    /// for Cull to take back in its turn. So each frame's steps stay in
    /// the order they were done, on both sides.
    pub fn take(&mut self, back: bool, reach: &Reach) -> Option<Step> {
        let Some(part) = &reach.part else {
            return if back {
                self.undo_at(reach.at)
            } else {
                self.redo_at(reach.at)
            };
        };
        let side = if back { &mut self.undo } else { &mut self.redo };
        let entry = side.get_mut(reach.at)?;
        let (taken, rest): (Step, Step) = std::mem::take(&mut entry.step)
            .into_iter()
            .partition(|m| part.contains(&m.path));
        entry.step = rest;
        if entry.step.is_empty() {
            side.remove(reach.at);
        }
        if taken.is_empty() {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let other = if back { &mut self.redo } else { &mut self.undo };
        other.push(Entry {
            order: Order::next(),
            id,
            step: taken.clone(),
        });
        self.acted(id);
        Some(if back {
            taken
        } else {
            taken.into_iter().map(Moved::reversed).collect()
        })
    }

    /// The step just acted on took the window off its frame: the next
    /// frame the window shows is where it carried it. Relies on `last`
    /// being that step: called right after it is recorded or stepped.
    pub fn carry(&mut self) {
        self.carrying = self.last;
    }

    /// The window's frame changed, `file` its file now (None for no
    /// frame): where a carry lands, or a move away from where one
    /// landed, which ends it.
    pub fn opened(&mut self, file: Option<&Path>) {
        if let Some(id) = self.carrying.take()
            && let Some(file) = file
        {
            self.carried = Some((id, file.to_path_buf()));
        } else if self
            .carried
            .as_ref()
            .is_some_and(|(_, to)| Some(to.as_path()) != file)
        {
            self.carried = None;
        }
    }
}

/// Where Develop's undo or redo of a mark goes ([`History::reach`]):
/// the step at `at` on its side, whole, or with `part` only those
/// frames' share of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reach {
    pub at: usize,
    pub part: Option<Vec<PathBuf>>,
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

    /// Develop's reach: the latest step on the file, past steps on
    /// others, which stay in their order; and a carried step from the
    /// file it landed on until a move away.
    #[test]
    fn a_frame_reaches_its_own_steps_and_the_one_that_carried_it() {
        let mut h = History::default();
        let (none, three) = (tags(0, Flag::None), tags(3, Flag::None));
        h.record(vec![moved("a", &none, &three)]);
        h.record(vec![moved("b", &none, &three)]);
        h.record(vec![moved("c", &none, &three)]);
        let whole = |at| Some(Reach { at, part: None });
        assert_eq!(h.reach(true, Path::new("a")), whole(0));
        assert_eq!(h.reach(true, Path::new("d")), None);
        let a = h.undo_at(0).unwrap();
        assert_eq!(a[0].path, Path::new("a"));
        assert_eq!(h.reach(false, Path::new("a")), whole(0));
        // The others, as they were.
        assert_eq!(h.undo().unwrap()[0].path, Path::new("c"));
        assert_eq!(h.undo().unwrap()[0].path, Path::new("b"));
        assert_eq!(h.redo().unwrap()[0].path, Path::new("b"));

        // "c", redone, carried the window onto "d".
        assert_eq!(h.redo().unwrap()[0].path, Path::new("c"));
        h.carry();
        let d = Some(Path::new("d"));
        h.opened(d);
        assert_eq!(h.reach(true, Path::new("d")), h.reach(true, Path::new("c")));
        h.opened(d);
        assert!(h.reach(true, Path::new("d")).is_some(), "still there");
        h.opened(Some(Path::new("e")));
        h.opened(d);
        assert_eq!(h.reach(true, Path::new("d")), None, "left");
        // No frame at all ends it too.
        h.carry();
        h.opened(d);
        h.opened(None);
        h.opened(d);
        assert_eq!(h.reach(true, Path::new("d")), None, "no frame");
    }

    /// A step over two frames, one of them named by a later step even
    /// back at the same value: blocked, so only the open frame's share
    /// goes, and the rest stays where it was. Each frame's steps keep
    /// their order, so undoing and redoing everything after lands each
    /// frame where it should.
    #[test]
    fn a_blocked_step_gives_the_open_frame_s_share_alone() {
        let mut h = History::default();
        let (none, five) = (tags(0, Flag::None), tags(5, Flag::None));
        h.record(vec![moved("a", &none, &five), moved("b", &none, &five)]);
        h.record(vec![moved("b", &five, &none)]);
        h.record(vec![moved("b", &none, &five)]);
        let reach = h.reach(true, Path::new("a")).unwrap();
        assert_eq!(
            reach,
            Reach {
                at: 0,
                part: Some(vec!["a".into()])
            }
        );
        let taken = h.take(true, &reach).unwrap();
        assert_eq!(taken, vec![moved("a", &none, &five)]);
        assert_eq!(h.reach(true, Path::new("a")), None, "not stuck on it");
        // Everything undone: b's three in order, newest first.
        let order: Vec<Step> = std::iter::from_fn(|| h.undo()).collect();
        assert_eq!(
            order,
            vec![
                vec![moved("b", &none, &five)],
                vec![moved("b", &five, &none)],
                vec![moved("b", &none, &five)],
            ]
        );
        // And redone: b's, then a's share last, as it was undone first.
        let redone: Vec<PathBuf> = std::iter::from_fn(|| h.redo())
            .map(|s| s[0].path.clone())
            .collect();
        assert_eq!(redone, ["b", "b", "b", "a"].map(PathBuf::from));
    }

    /// The frames of the randomized walk.
    const WALKED: [&str; 3] = ["a", "b", "c"];

    fn walked(p: &Path) -> usize {
        WALKED.iter().position(|q| Path::new(q) == p).unwrap()
    }

    /// A step put on the ratings `now`, each frame checked to be as
    /// the step expects, as an exact landing.
    fn put(now: &mut [u8; 3], step: &Step, log: &str) {
        for m in step {
            let i = walked(&m.path);
            assert_eq!(now[i], m.after.rating, "landing: {log}");
            now[i] = m.before.rating;
        }
    }

    /// Each frame's steps, read up the undo side and down the redo
    /// side, form one chain from its start (0) through `now`, and the
    /// undo side's ticks rise.
    fn chained(h: &History, now: &[u8; 3], log: &str) {
        let ticks: Vec<u64> = h.undo.iter().map(|e| e.order.tick()).collect();
        assert!(ticks.windows(2).all(|w| w[0] < w[1]), "ticks: {log}");
        for (i, p) in WALKED.iter().enumerate() {
            let named = |e: &&Entry| e.step.iter().any(|m| m.path == Path::new(p));
            let mut v = 0;
            let chain = h.undo.iter().filter(named).map(|e| (e, false));
            let chain = chain.chain(h.redo.iter().rev().filter(named).map(|e| (e, true)));
            let mut at_now = false;
            for (e, redo) in chain {
                if redo && !at_now {
                    assert_eq!(v, now[i], "{p} now: {log}");
                    at_now = true;
                }
                let m = e.step.iter().find(|m| m.path == Path::new(p)).unwrap();
                assert_eq!(m.before.rating, v, "{p} chain: {log}");
                v = m.after.rating;
            }
            if !at_now {
                assert_eq!(v, now[i], "{p} now: {log}");
            }
        }
    }

    /// A run of `ops` random acts from `seed`: marks over random sets
    /// of three frames, Develop's undo and redo through `reach` and
    /// `take` from a random frame, Cull's, carries, and opens.
    fn random_walk(seed: u64, ops: usize) -> (History, [u8; 3], String) {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut n = |k: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % k
        };
        let (mut h, mut now, mut log) = (History::default(), [0u8; 3], String::new());
        for _ in 0..ops {
            match n(8) {
                0 | 1 => {
                    let (set, v) = (1 + n(7), n(6) as u8);
                    let step: Step = (0..3)
                        .filter(|&i| set & (1 << i) != 0 && now[i] != v)
                        .map(|i| moved(WALKED[i], &tags(now[i], Flag::None), &tags(v, Flag::None)))
                        .collect();
                    for m in &step {
                        now[walked(&m.path)] = v;
                    }
                    h.record(step);
                    log += &format!("mark {set}={v}; ");
                }
                2 | 3 => {
                    let (back, f) = (n(2) == 0, n(3) as usize);
                    let file = Path::new(WALKED[f]);
                    log += &format!("develop {} on {f}; ", if back { "undo" } else { "redo" });
                    if let Some(reach) = h.reach(back, file) {
                        let carried = h.carried.as_ref().is_some_and(|(_, to)| to == file);
                        let step = h.take(back, &reach).expect("a take where reach found");
                        assert!(carried || step.iter().any(|m| m.path == file), "{log}");
                        put(&mut now, &step, &log);
                    }
                }
                4 => {
                    log += "cull undo; ";
                    if let Some(step) = h.undo() {
                        put(&mut now, &step, &log);
                    }
                }
                5 => {
                    log += "cull redo; ";
                    if let Some(step) = h.redo() {
                        put(&mut now, &step, &log);
                    }
                }
                6 => {
                    let f = n(3) as usize;
                    log += &format!("carry to {f}; ");
                    h.carry();
                    h.opened(Some(Path::new(WALKED[f])));
                }
                _ => {
                    let f = n(4) as usize;
                    log += &format!("open {f}; ");
                    h.opened(WALKED.get(f).map(Path::new));
                }
            }
            chained(&h, &now, &log);
        }
        (h, now, log)
    }

    /// Develop's takes out of order and in shares keep the history
    /// sound under random walks: every frame's chain unbroken, every
    /// landing exact, undo-all back at the start and redo-all at what
    /// a plain redo-all of the same walk reaches, twice over.
    #[test]
    fn random_walks_keep_every_frame_s_chain() {
        for seed in 0..400 {
            let (mut first, mut top, log) = random_walk(seed, 60);
            while let Some(step) = first.redo() {
                put(&mut top, &step, &log);
            }
            let (mut h, mut now, _) = random_walk(seed, 60);
            for _ in 0..2 {
                while let Some(step) = h.undo() {
                    put(&mut now, &step, &log);
                }
                assert_eq!(now, [0, 0, 0], "undo-all: {log}");
                while let Some(step) = h.redo() {
                    put(&mut now, &step, &log);
                }
                assert_eq!(now, top, "redo-all: {log}");
            }
        }
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
