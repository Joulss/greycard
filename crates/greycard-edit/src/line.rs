//! One line of development: the current edit, the history before it,
//! the snapshots, and the exports made along it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Edit, history, meta, retouch, sync};

/// One line of development: the current edit and the steps that led
/// to it, the states kept by name, and the exports made along it.
///
/// The sidecar's own line is version 0, the file's own version
/// (the sidecar's `line`); each further version holds a line of its own
/// ([`Version::line`]). Undo, redo, the snapshots, the export records
/// and the history's cap are all a line's, so a version walks its own
/// history and nothing it does touches another's.
///
/// [`Version::line`]: crate::Version::line
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Line {
    pub current: Edit,
    /// The words the step that made [`Self::current`] was recorded
    /// with, when it was one with a name of its own (a preset, a
    /// sync, a snapshot restored): see [`Step::label`]. Written as
    /// `step` just after `current`, left out when there is none, and
    /// read loosely.
    #[serde(
        rename = "step",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "meta::loose"
    )]
    pub current_label: Option<String>,
    /// The exports made from [`Self::current`]: see [`Step::exports`].
    /// Written as `exported` after `step`, left out when there are
    /// none, and read loosely, a record at a time.
    #[serde(
        rename = "exported",
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "loose_exports"
    )]
    pub current_exports: Vec<Exported>,
    /// The last export made of the frame, whatever state it was made
    /// from: a fact about what went out, not the state, so it outlives
    /// an undo past that state, a history that replaces it and the
    /// history's cap, none of which a record on a state does. What went
    /// out is a version's picture, so each line has its own. Written
    /// as `last_export` after `exported`, left out when the frame has
    /// never been exported (or its sidecar is from before the field,
    /// which gets it on its next export) and read loosely.
    #[serde(
        rename = "last_export",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "meta::loose"
    )]
    pub last_export: Option<Exported>,
    /// The id of the current state: see [`Step::id`]. Written as `id`
    /// after `exported`, left out when the state has none yet, and
    /// read loosely.
    #[serde(
        rename = "id",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "meta::loose"
    )]
    pub current_id: Option<String>,
    /// Earlier states, oldest first, each one a whole edit and the
    /// words it was recorded with, if any.
    pub history: Vec<Step>,
    /// States undone, newest first; kept for the session, not written.
    #[serde(skip)]
    pub redo: Vec<Step>,
    /// When this session made [`Self::current`] current, by a record
    /// or a redo: see [`Step::order`].
    #[serde(skip)]
    pub current_order: Order,
    /// States kept by name, in the order they were taken, outside
    /// the history's cap.
    pub snapshots: Vec<Snapshot>,
}

/// One state of the history: a whole edit, and the words the step
/// that made it was recorded with when it had a name of its own.
///
/// The label goes with the state it produced, so an undo carries it
/// to the redo stack and a redo brings it back, and a new step after
/// an undo drops it along with the rest of the redo stack. Most steps
/// have none, and the panel names those by what moved
/// ([`history::describe`]); a preset, a sync and a snapshot restored
/// are recorded with theirs ([`Sidecar::record_as`]), and the panel
/// reads it in place of the list of sections that moved
/// ([`history::describe_step`]).
///
/// Written as the edit's own object with a `step` key beside its
/// fields, and that key left out when there is no label: a sidecar
/// with no labels writes exactly as one did before labels existed,
/// and a build from before them reads each state as the edit it is
/// (an edit ignores a field it does not know), labels dropped. Read
/// loosely, as the meta is: a `step` that is not a string reads as
/// no label rather than costing the state.
///
/// The exports made from the state ride on it the same way, under an
/// `exported` key beside `step`: see [`Step::exports`].
///
/// [`Sidecar::record_as`]: crate::Sidecar::record_as
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Step {
    pub edit: Edit,
    pub label: Option<String>,
    /// The files written from this state, oldest first. A record of
    /// what went out, not a change: it adds no state to walk, so undo
    /// and redo step over it and carry it with its state, and a
    /// state that goes (undone and then replaced, or past the cap)
    /// takes its records with it.
    pub exports: Vec<Exported>,
    /// The state's id: blake3 of its parent state's id, the edit as
    /// compact JSON and the label ([`sync::state_id`]), computed once
    /// when the state is recorded and carried with it through undo
    /// and redo. The same state reached on two machines from the same
    /// parent has the same id, which is how two copies of one frame's
    /// sidecar find the states they share (§233). None only on a
    /// state from a sidecar written before ids, until its first read
    /// fills it ([`Sidecar::fill_ids`]). Written as `id` beside
    /// `step` and `exported`, and read loosely.
    ///
    /// [`Sidecar::fill_ids`]: crate::Sidecar::fill_ids
    pub id: Option<String>,
    /// When this session last acted on the state, never written: for
    /// one in the history, when it was made current (recorded, or
    /// redone); for one waiting to be redone, when it was undone. So
    /// an undo takes back the later of two kinds of step, and a redo
    /// makes again the one undone last. [`Order::BEFORE`] for one read
    /// from the sidecar or carried over by a join.
    pub order: Order,
}

/// Where a step stands in the session's order of things done: a tick
/// of one clock the process keeps for every state a line records and
/// every rating, flag, or label change the window records, so that
/// undo can take back whichever of the two was done last (§117's
/// marks have a history of their own, across frames, beside each
/// line's). Not part of what a state is: two states equal in all else
/// are equal whatever their order, so a state read back from disk
/// equals the one written.
#[derive(Debug, Clone, Copy, Default, Eq)]
pub struct Order(u64);

impl PartialEq for Order {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

static CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How far apart two acts' ticks are: room for the ticks of the work
/// one act's request does when it runs later ([`Order::during`]).
const STRIDE: u64 = 1 << 20;

thread_local! {
    /// The act whose work is running ([`Order::during`]): its tick,
    /// and how many ticks its work has taken so far.
    static AS_OF: std::cell::Cell<Option<(u64, u64)>> = const { std::cell::Cell::new(None) };
}

impl Order {
    /// A state from before the session: older than anything done in it.
    pub const BEFORE: Self = Self(0);

    /// The next tick: later than every one handed out before it. Inside
    /// [`Order::during`], the next of the act's own, which stay before
    /// every act after it however late its work runs.
    pub fn next() -> Self {
        if let Some((at, taken)) = AS_OF.get() {
            let taken = (taken + 1).min(STRIDE - 1);
            AS_OF.set(Some((at, taken)));
            return Self(at + taken);
        }
        Self(CLOCK.fetch_add(STRIDE, std::sync::atomic::Ordering::Relaxed) + STRIDE)
    }

    /// The tick of an act being made now, to carry to where its work
    /// is done: a new one, or inside [`Order::during`] the running
    /// act's, whose work this is part of.
    pub fn act() -> Self {
        match AS_OF.get() {
            Some((at, _)) => Self(at),
            None => Self::next(),
        }
    }

    /// `work` done in the name of the act ticked `at`: a request queued
    /// at a key's press and run when its read lands. Every tick it
    /// takes is the act's, so what it records stands where the press
    /// did, before anything done after the press and before the work.
    pub fn during<R>(at: Self, work: impl FnOnce() -> R) -> R {
        struct Restore(Option<(u64, u64)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                AS_OF.set(self.0);
            }
        }
        let _restore = Restore(AS_OF.replace(Some((at.0, 0))));
        work()
    }

    /// The tick, for comparing: 0 for a state from before the session.
    pub fn tick(self) -> u64 {
        self.0
    }
}

/// The later of two ticks.
fn later_of(a: Order, b: Order) -> Order {
    if a.tick() >= b.tick() { a } else { b }
}

impl Step {
    /// A state with no words of its own.
    pub fn plain(edit: Edit) -> Self {
        Self {
            edit,
            label: None,
            exports: Vec::new(),
            id: None,
            order: Order::BEFORE,
        }
    }

    fn parts(&self) -> (&Edit, Option<&str>) {
        (&self.edit, self.label.as_deref())
    }
}

/// An export made from a state of the history: the file written, the
/// export preset the sheet was when it was one, and when it finished.
///
/// Kept on the state it was exported from ([`Step::exports`],
/// [`Sidecar::current_exports`]) rather than as a state of its own:
/// an export changes nothing in the edit, so a step for it would be
/// an undo that does nothing, and an export that finished after the
/// panel moved on would have had to land between two states. On the
/// state, the history panel shows it as a row of its own above that
/// state's, and a click on it goes to that state
/// ([`Sidecar::rows`]).
///
/// A build from before it reads a sidecar with records as though
/// they were not there, and drops them at its next save.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Exported {
    /// The file written, as the path it was written to.
    pub file: String,
    /// The export preset's name, when the sheet was one as saved.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "meta::loose"
    )]
    pub preset: Option<String>,
    /// When it was written, seconds since the Unix epoch.
    #[serde(default, deserialize_with = "meta::loose")]
    pub at: u64,
}

impl Exported {
    /// The file's name, without its folder.
    pub fn file_name(&self) -> &str {
        Path::new(&self.file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.file)
    }

    /// What the history panel's row says: [`history::export_label`].
    pub fn label(&self) -> String {
        history::export_label(self.file_name(), self.preset.as_deref())
    }
}

/// The export records under a key, each read on its own: one that
/// will not read is left out, the rest kept, and a key that is not a
/// list reads as none.
fn loose_exports<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<Exported>, D::Error> {
    Ok(exports_from(serde_json::Value::deserialize(d)?))
}

fn exports_from(value: serde_json::Value) -> Vec<Exported> {
    match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|i| serde_json::from_value(i).ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// A row of the history panel, newest first: a state, or an export
/// made from one ([`Sidecar::rows`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The state at this index among [`Sidecar::states`].
    State(usize),
    /// Export `export` (oldest first) of the state at `state`.
    Exported { state: usize, export: usize },
}

impl Row {
    /// The state the row goes to when clicked: its own, or the one it
    /// was exported from.
    pub fn state(self) -> usize {
        match self {
            Self::State(i) | Self::Exported { state: i, .. } => i,
        }
    }
}

impl From<Edit> for Step {
    fn from(edit: Edit) -> Self {
        Self::plain(edit)
    }
}

impl PartialEq<Edit> for Step {
    fn eq(&self, other: &Edit) -> bool {
        self.edit == *other
    }
}

impl Serialize for Step {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Written<'a> {
            #[serde(flatten)]
            edit: &'a Edit,
            #[serde(skip_serializing_if = "Option::is_none")]
            step: &'a Option<String>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            exported: &'a Vec<Exported>,
            #[serde(skip_serializing_if = "Option::is_none")]
            id: &'a Option<String>,
        }
        Written {
            edit: &self.edit,
            step: &self.label,
            exported: &self.exports,
            id: &self.id,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let label = value
            .as_object_mut()
            .and_then(|o| o.remove("step"))
            .and_then(|s| match s {
                serde_json::Value::String(s) => Some(s),
                _ => None,
            });
        let exports = value
            .as_object_mut()
            .and_then(|o| o.remove("exported"))
            .map(exports_from)
            .unwrap_or_default();
        let id = value
            .as_object_mut()
            .and_then(|o| o.remove("id"))
            .and_then(|s| match s {
                serde_json::Value::String(s) => Some(s),
                _ => None,
            });
        let edit = Edit::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            edit,
            label,
            exports,
            id,
            order: Order::BEFORE,
        })
    }
}

/// A whole edit kept under a name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    /// When it was taken, seconds since the Unix epoch.
    pub taken: u64,
    pub edit: Edit,
}

/// How many earlier states a sidecar keeps. The earliest is kept
/// whatever the count, so the state the file was opened in stays.
pub const HISTORY: usize = 50;

impl Line {
    /// A line whose one state is `edit`, with nothing before it: what
    /// a frame nobody has developed starts from. Its id is made when
    /// first asked for ([`Self::fill_ids`]).
    pub fn of(edit: Edit) -> Self {
        Self {
            current: edit,
            ..Self::default()
        }
    }

    /// Record `edit` as the current state, keeping the one it replaces;
    /// false when it was the current state already. The step has no
    /// words of its own: the panel names it by what moved.
    pub fn record(&mut self, edit: Edit) -> bool {
        self.record_as(edit, None)
    }

    /// Record `edit` as the current state with `label` as the step's
    /// words ("Preset: Faded film"), which the history panel shows in
    /// place of the sections that moved; `None` is [`Self::record`].
    /// A blank label is none. False, and the label not kept, when
    /// `edit` was the current state already. What was undone goes,
    /// labels and all.
    pub fn record_as(&mut self, edit: Edit, label: Option<String>) -> bool {
        if edit == self.current {
            return false;
        }
        // Blank words are none, so nothing writes an empty `step`.
        let label = label.filter(|l| !l.trim().is_empty());
        // The new state's id hangs off the current state's, which a
        // sidecar from before ids gets here if nothing filled it yet.
        self.fill_ids();
        let parent = self.current_id.as_deref().unwrap_or(sync::ZERO_ID);
        let id = sync::state_id(parent, &edit, label.as_deref());
        let previous = Step {
            edit: std::mem::replace(&mut self.current, edit),
            label: std::mem::replace(&mut self.current_label, label),
            exports: std::mem::take(&mut self.current_exports),
            id: self.current_id.replace(id),
            order: std::mem::replace(&mut self.current_order, Order::next()),
        };
        self.history.push(previous);
        // The earliest stays; what follows it goes.
        self.cap_history();
        self.redo.clear();
        true
    }

    /// How many states there are to walk: the history, the current
    /// one, and what was undone.
    pub fn states(&self) -> usize {
        self.history.len() + 1 + self.redo.len()
    }

    /// Where the current state sits among [`Self::states`], oldest
    /// first.
    pub fn position(&self) -> usize {
        self.history.len()
    }

    /// The index among [`Self::states`] of the state the history
    /// panel's row `row` goes to ([`Self::rows`]): its own, or for an
    /// export's row the state it was exported from. None for a row
    /// past the end, or a negative one, which is what a list with
    /// nothing chosen hands back.
    pub fn state_at_row(&self, row: i32) -> Option<usize> {
        self.row(row).map(Row::state)
    }

    /// The history panel's row `row` of [`Self::rows`]; None past the
    /// end or for a negative one.
    pub fn row(&self, row: i32) -> Option<Row> {
        usize::try_from(row)
            .ok()
            .and_then(|r| self.rows().get(r).copied())
    }

    /// The history panel's rows, newest first: every state, each
    /// under the exports made from it, newest of those first. An
    /// export's row sits between its state and the next, since that
    /// is where the look it sent out stands in the history, whenever
    /// the file itself finished.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::with_capacity(self.states());
        for state in (0..self.states()).rev() {
            let n = self.exports(state).len();
            rows.extend((0..n).rev().map(|export| Row::Exported { state, export }));
            rows.push(Row::State(state));
        }
        rows
    }

    /// Where among [`Self::rows`] the state at `index` is.
    pub fn row_of_state(&self, index: usize) -> Option<usize> {
        self.rows().iter().position(|r| *r == Row::State(index))
    }

    /// The exports made from the state at `index` among
    /// [`Self::states`], oldest first.
    pub fn exports(&self, index: usize) -> &[Exported] {
        let position = self.position();
        if index < position {
            self.history.get(index).map_or(&[], |s| &s.exports)
        } else if index == position {
            &self.current_exports
        } else {
            let past = index - position - 1;
            self.redo
                .len()
                .checked_sub(past + 1)
                .map_or(&[], |i| &self.redo[i].exports)
        }
    }

    /// Note that `exported` was written from `edit`: on the state that
    /// is `edit`, the current one when it is, and otherwise the
    /// nearest to it, back through the history and then forward
    /// through what was undone. Nothing is recorded as a state, and
    /// undo and redo are as they were. False, and nothing noted, when
    /// no state here is `edit` any more: the history moved on past it
    /// while the file was written (undone and then replaced), and a
    /// record with no state to go back to would not do what its row
    /// promises.
    ///
    /// A record a state took is also kept as [`Self::last_export`]. One
    /// no state took (the command line's overrides, which are never the
    /// frame's) leaves the sidecar as it was.
    pub fn record_export(&mut self, edit: &Edit, exported: Exported) -> bool {
        let last = exported.clone();
        if self.current == *edit {
            self.current_exports.push(exported);
            self.keep_last_export(last);
            return true;
        }
        // The redo stack is newest first: its nearest is its last.
        let found = self
            .history
            .iter_mut()
            .rev()
            .chain(self.redo.iter_mut().rev())
            .find(|s| s.edit == *edit);
        match found {
            Some(step) => {
                step.exports.push(exported);
                self.keep_last_export(last);
                true
            }
            None => false,
        }
    }

    fn keep_last_export(&mut self, exported: Exported) {
        if self
            .last_export
            .as_ref()
            .is_none_or(|l| l.at <= exported.at)
        {
            self.last_export = Some(exported);
        }
    }

    /// The state at `index` among [`Self::states`], oldest first.
    pub fn state(&self, index: usize) -> Option<&Edit> {
        self.state_and_label(index).map(|(edit, _)| edit)
    }

    /// The words the step that made the state at `index` was recorded
    /// with, when it had any; see [`Step::label`].
    pub fn label(&self, index: usize) -> Option<&str> {
        self.state_and_label(index).and_then(|(_, label)| label)
    }

    /// The state at `index` among [`Self::states`] and its words.
    fn state_and_label(&self, index: usize) -> Option<(&Edit, Option<&str>)> {
        let position = self.position();
        if index < position {
            self.history.get(index).map(Step::parts)
        } else if index == position {
            Some((&self.current, self.current_label.as_deref()))
        } else {
            // The redo stack is newest first.
            let past = index - position - 1;
            self.redo
                .len()
                .checked_sub(past + 1)
                .map(|i| self.redo[i].parts())
        }
    }

    /// What the history panel calls the state at `index`: the words
    /// it was recorded with, or what moved from the state before it
    /// ([`history::describe_step`]). None for the earliest state,
    /// which has nothing before it to be named against, and for an
    /// index past the end.
    pub fn describe(&self, index: usize) -> Option<String> {
        let before = self.state(index.checked_sub(1)?)?;
        let (after, label) = self.state_and_label(index)?;
        Some(history::describe_step(
            before,
            after,
            label,
            &self.snapshots,
        ))
    }

    /// Make the state at `index` current by undoing or redoing up to
    /// it, so what lies past it is still there to redo; false when
    /// nothing moved.
    pub fn go_to(&mut self, index: usize) -> bool {
        let from = self.position();
        while self.position() > index && self.undo() {}
        while self.position() < index && self.redo() {}
        self.position() != from
    }

    /// Keep the current state under `name`; its index.
    pub fn take_snapshot(&mut self, name: impl Into<String>, taken: u64) -> usize {
        self.snapshots.push(Snapshot {
            name: name.into(),
            taken,
            edit: self.current.clone(),
        });
        self.snapshots.len() - 1
    }

    /// Make snapshot `index` the current state as a new step, so the
    /// history stays a line and undo goes back to before; false when
    /// it is the current state already, or there is no such snapshot.
    ///
    /// The step is recorded as "Snapshot: " and the name the snapshot
    /// had then, so the row keeps saying where it came from after the
    /// snapshot is renamed or removed.
    pub fn restore_snapshot(&mut self, index: usize) -> bool {
        match self.snapshots.get(index) {
            Some(s) => {
                let (edit, label) = (s.edit.clone(), history::snapshot_label(&s.name));
                self.record_as(edit, Some(label))
            }
            None => false,
        }
    }

    /// Back to the previous state; false at the beginning. The state
    /// undone keeps its words on the redo stack.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.history.pop() else {
            return false;
        };
        let undone = Step {
            edit: std::mem::replace(&mut self.current, previous.edit),
            label: std::mem::replace(&mut self.current_label, previous.label),
            exports: std::mem::replace(&mut self.current_exports, previous.exports),
            id: std::mem::replace(&mut self.current_id, previous.id),
            order: Order::next(),
        };
        self.current_order = previous.order;
        self.redo.push(undone);
        true
    }

    /// Give every state here the repair sources the engine chose,
    /// `done` being the patches it chose them for, where a state has
    /// the same patch still waiting on one (see
    /// [`retouch::Retouch::take_sources`]),
    /// in place: a chosen source completes the step that placed the
    /// patch and is not a step of its own, so nothing is recorded and
    /// nothing undone is dropped. A state that comes out the same as
    /// the one after it — a line a build that recorded the source as
    /// a step left behind — merges into it, keeping the earlier
    /// step's words. Any two equal neighbors merge, so a step that
    /// changed nothing, such as restoring a snapshot equal to the
    /// state before it, goes too; it had nothing to undo. True when
    /// anything changed.
    pub fn take_sources(&mut self, done: &[retouch::Patch]) -> bool {
        let mut took = false;
        for edit in self.edits_mut() {
            took |= edit.retouch.take_sources(done);
        }
        if !took {
            return false;
        }
        // Oldest first: the history, the current state, then the redo
        // stack from its end.
        // The exports made from either of two states that merge are
        // the merged state's, the earlier's first.
        self.history.dedup_by(|later, earlier| {
            let same = later.edit == earlier.edit;
            if same {
                earlier.exports.append(&mut later.exports);
                earlier.id = later.id.take();
                // Made current when the later was.
                earlier.order = later_of(earlier.order, later.order);
            }
            same
        });
        let current = &self.current;
        if let Some(mut earlier) = self.history.pop_if(|s| s.edit == *current) {
            self.current_label = earlier.label;
            earlier.exports.append(&mut self.current_exports);
            self.current_exports = earlier.exports;
        }
        // The redo stack is newest first, so its earlier state is the
        // later in the vector.
        self.redo.dedup_by(|earlier, later| {
            if later.edit == earlier.edit {
                std::mem::swap(&mut later.label, &mut earlier.label);
                // Undone when the last of the two was, so a redo still
                // makes again what was undone last.
                later.order = later_of(later.order, earlier.order);
                let mut merged = std::mem::take(&mut earlier.exports);
                merged.append(&mut later.exports);
                later.exports = merged;
                true
            } else {
                false
            }
        });
        while let Some(mut later) = self.redo.pop_if(|s| s.edit == self.current) {
            self.current_exports.append(&mut later.exports);
        }
        true
    }

    /// Forward again; false when nothing was undone.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let current = Step {
            edit: std::mem::replace(&mut self.current, next.edit),
            label: std::mem::replace(&mut self.current_label, next.label),
            exports: std::mem::replace(&mut self.current_exports, next.exports),
            id: std::mem::replace(&mut self.current_id, next.id),
            order: std::mem::replace(&mut self.current_order, Order::next()),
        };
        self.history.push(current);
        true
    }

    /// Every edit of this line: the current one, the history, what was
    /// undone, and the snapshots.
    pub fn edits(&self) -> impl Iterator<Item = &Edit> {
        std::iter::once(&self.current)
            .chain(self.history.iter().map(|s| &s.edit))
            .chain(self.redo.iter().map(|s| &s.edit))
            .chain(self.snapshots.iter().map(|s| &s.edit))
    }

    /// [`Self::edits`], to change in place.
    pub(crate) fn edits_mut(&mut self) -> impl Iterator<Item = &mut Edit> {
        std::iter::once(&mut self.current)
            .chain(self.history.iter_mut().map(|s| &mut s.edit))
            .chain(self.redo.iter_mut().map(|s| &mut s.edit))
            .chain(self.snapshots.iter_mut().map(|s| &mut s.edit))
    }

    /// Every state of this line that names the look `from` names `to`
    /// instead, in place: nothing is recorded, nothing undone is
    /// dropped, and undo never goes back to a name that is no longer
    /// there.
    ///
    /// A renamed state is a new state to the ids, though: from the
    /// first renamed state on, each state's id is made again from its
    /// new content and its parent's new id ([`sync::state_id`]), the
    /// redo stack's too. A join checks a chain by making ids again from
    /// content (§233's rule ii), so a renamed state under its old id
    /// would fail that check, and a copy elsewhere that still has the
    /// old name would take the renamed states for its own and lose
    /// them to its newer branch. Under new ids the renamed run is a
    /// branch of its own, and a join keeps both. The states before the
    /// first renamed one keep theirs and stay shared. True when any
    /// state changed.
    pub fn rename_look(&mut self, from: &str, to: &str) -> bool {
        let mut renamed = false;
        for snapshot in &mut self.snapshots {
            renamed |= snapshot.edit.look_lut.rename(from, to);
        }
        // The states oldest first: the history, the current one, then
        // the redo stack from its end, each after the one before.
        let mut parent = sync::ZERO_ID.to_string();
        let mut chained = false;
        let mut step = |edit: &mut Edit, label: Option<&str>, id: &mut Option<String>| {
            chained |= edit.look_lut.rename(from, to);
            // A state before the first renamed one keeps its id; one
            // with none yet is chained on as `sync::state_ids` reads it.
            let kept = id.clone().filter(|_| !chained);
            let this = kept.unwrap_or_else(|| sync::state_id(&parent, edit, label));
            if chained {
                *id = Some(this.clone());
            }
            parent = this;
        };
        for s in &mut self.history {
            step(&mut s.edit, s.label.as_deref(), &mut s.id);
        }
        step(
            &mut self.current,
            self.current_label.as_deref(),
            &mut self.current_id,
        );
        for s in self.redo.iter_mut().rev() {
            step(&mut s.edit, s.label.as_deref(), &mut s.id);
        }
        renamed | chained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{exposed, scratch};
    use crate::{
        Adjustment, Preset, Section, Sidecar, WhiteBalance, apply_preset_into, baseline, describe,
        geometry, sync_into,
    };

    /// The history list shows the newest step first, and undo leaves
    /// the undone ones below the current one rather than dropping
    /// them, so the mapping is over the whole line and not over what
    /// is still in force.
    #[test]
    fn a_row_of_the_newest_first_list_names_a_state() {
        let mut sidecar = Sidecar::default();
        for exposure in [1.0, 2.0, 3.0] {
            let mut edit = Edit::default();
            edit.light.exposure = exposure;
            assert!(sidecar.record(edit));
        }
        assert_eq!(sidecar.states(), 4);
        // Row 0 is the newest state, the last row the oldest.
        assert_eq!(sidecar.state_at_row(0), Some(3));
        assert_eq!(sidecar.state_at_row(3), Some(0));
        assert_eq!(sidecar.state_at_row(4), None);
        // A list with nothing chosen reports -1.
        assert_eq!(sidecar.state_at_row(-1), None);
        // Undone steps stay in the list, so the rows do not move.
        assert!(sidecar.undo());
        assert_eq!(sidecar.states(), 4);
        assert_eq!(sidecar.state_at_row(0), Some(3));
        assert_eq!(
            sidecar.state(sidecar.state_at_row(1).unwrap()).unwrap(),
            sidecar.state(sidecar.position()).unwrap()
        );
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut sidecar = Sidecar::default();
        for exposure in [1.0, 2.0, 3.0] {
            let mut e = Edit::default();
            e.light.exposure = exposure;
            sidecar.record(e);
        }
        assert!(sidecar.undo());
        assert_eq!(sidecar.current.light.exposure, 2.0);
        assert!(sidecar.undo());
        assert_eq!(sidecar.current.light.exposure, 1.0);
        assert!(sidecar.redo());
        assert_eq!(sidecar.current.light.exposure, 2.0);
        // A new state after an undo forgets what was undone.
        let mut e = Edit::default();
        e.light.exposure = 5.0;
        sidecar.record(e);
        assert!(!sidecar.redo());
        assert_eq!(sidecar.history.len(), 3);
        for _ in 0..3 {
            assert!(sidecar.undo());
        }
        assert!(!sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
    }

    #[test]
    fn go_to_walks_either_way_and_keeps_what_lies_past() {
        let mut sidecar = Sidecar::default();
        for exposure in [1.0, 2.0, 3.0] {
            let mut e = Edit::default();
            e.light.exposure = exposure;
            sidecar.record(e);
        }
        // Four states: the default, then 1, 2, 3; the current is last.
        assert_eq!(sidecar.states(), 4);
        assert_eq!(sidecar.position(), 3);
        assert!(sidecar.go_to(1));
        assert_eq!(sidecar.current.light.exposure, 1.0);
        assert_eq!(sidecar.position(), 1);
        assert_eq!(sidecar.states(), 4);
        // Every state is still there to read, in order.
        let exposures: Vec<f32> = (0..4)
            .map(|i| sidecar.state(i).unwrap().light.exposure)
            .collect();
        assert_eq!(exposures, [0.0, 1.0, 2.0, 3.0]);
        assert!(sidecar.state(4).is_none());
        assert!(sidecar.go_to(3));
        assert_eq!(sidecar.current.light.exposure, 3.0);
        assert!(!sidecar.go_to(3));
        // Past the end stops at the end.
        assert!(sidecar.go_to(0));
        assert!(sidecar.go_to(9));
        assert_eq!(sidecar.position(), 3);
    }

    #[test]
    fn the_cap_keeps_the_earliest_state() {
        let mut sidecar = Sidecar::default();
        let mut first = Edit::default();
        first.light.exposure = -9.0;
        sidecar.record(first.clone());
        for i in 0..(HISTORY + 20) {
            let mut e = Edit::default();
            e.light.exposure = i as f32;
            sidecar.record(e);
        }
        assert_eq!(sidecar.history.len(), HISTORY);
        assert_eq!(sidecar.history[0], Edit::default());
        assert_eq!(sidecar.history[1].edit.light.exposure, 20.0);
    }

    #[test]
    fn snapshots_are_kept_named_restored_as_a_step_and_written() {
        let mut sidecar = Sidecar::default();
        let mut e = Edit::default();
        e.light.exposure = 1.0;
        sidecar.record(e.clone());
        assert_eq!(sidecar.take_snapshot("Bright", 1_700_000_000), 0);
        let mut e2 = Edit::default();
        e2.light.exposure = -1.0;
        sidecar.record(e2);
        // Restoring is a step forward, not a step back.
        assert!(sidecar.restore_snapshot(0));
        assert_eq!(sidecar.current, e);
        assert_eq!(sidecar.history.len(), 3);
        assert!(!sidecar.restore_snapshot(0));
        assert!(!sidecar.restore_snapshot(7));
        assert!(sidecar.undo());
        assert_eq!(sidecar.current.light.exposure, -1.0);
        // The snapshot is written and read back; an old sidecar
        // without any reads as none.
        let dir = std::env::temp_dir().join(format!("greycard-snap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("IMG_0002.CR3");
        sidecar.save(&raw).unwrap();
        let back = Sidecar::load(&raw).unwrap().unwrap();
        assert_eq!(back.snapshots, sidecar.snapshots);
        assert_eq!(back.snapshots[0].name, "Bright");
        std::fs::write(
            Sidecar::path_for(&raw),
            r#"{"current":{"version":1},"history":[]}"#,
        )
        .unwrap();
        let old = Sidecar::load(&raw).unwrap().unwrap();
        assert!(old.snapshots.is_empty());
        // A snapshot's edit is migrated as the current one is.
        std::fs::write(
            Sidecar::path_for(&raw),
            r#"{"current":{"version":1},"snapshots":[{"name":"x","taken":0,"edit":{"version":1,"noise":{"enabled":true}}}]}"#,
        )
        .unwrap();
        let migrated = Sidecar::load(&raw).unwrap().unwrap();
        assert!(migrated.snapshots[0].edit.noise.profiled);
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// A labeled step is written with its words and read back with
    /// them, the current state's and a history state's alike; a step
    /// without words writes no `step` key, so a sidecar with no
    /// labels writes as one did before labels existed.
    #[test]
    fn a_labelled_step_round_trips_and_an_unlabelled_one_writes_as_before() {
        let mut sidecar = Sidecar::default();
        sidecar.record(exposed(0.5));
        assert!(sidecar.record_as(exposed(1.0), Some("Preset: Faded film".into())));
        sidecar.record(exposed(1.5));
        assert!(sidecar.record_as(exposed(2.0), Some("Sync from IMG_0001".into())));
        assert_eq!(sidecar.current_label.as_deref(), Some("Sync from IMG_0001"));
        assert_eq!(sidecar.label(2), Some("Preset: Faded film"));
        assert_eq!(sidecar.label(1), None);

        let dir = scratch("labels");
        let raw = dir.join("IMG_0003.CR3");
        sidecar.save(&raw).unwrap();
        let back = Sidecar::load(&raw).unwrap().unwrap();
        assert_eq!(back.current_label, sidecar.current_label);
        assert_eq!(back.history, sidecar.history);
        assert_eq!(back.label(2), Some("Preset: Faded film"));
        assert_eq!(back.describe(2).as_deref(), Some("Preset: Faded film"));
        assert_eq!(back.describe(4).as_deref(), Some("Sync from IMG_0001"));
        // The rows without words read as they always have.
        assert_eq!(back.describe(3).as_deref(), Some("Exposure +1.50"));

        // Where the words sit in the file: beside the edit's own
        // fields, and nowhere on a step that has none.
        let tree: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(Sidecar::path_for(&raw)).unwrap())
                .unwrap();
        assert_eq!(tree["step"], "Sync from IMG_0001");
        assert_eq!(tree["history"][2]["step"], "Preset: Faded film");
        assert_eq!(tree["history"][2]["light"]["exposure"], 1.0);
        assert!(tree["history"][1].get("step").is_none());

        let mut plain = Sidecar::default();
        plain.record(exposed(0.5));
        plain.record(exposed(1.0));
        let json = serde_json::to_string_pretty(&plain).unwrap();
        assert!(!json.contains("\"step\""), "{json}");
        // Byte for byte what a history of bare edits wrote, but for
        // each state's id beside it (§233).
        #[derive(Serialize)]
        struct State<'a> {
            #[serde(flatten)]
            edit: &'a Edit,
            id: &'a Option<String>,
        }
        #[derive(Serialize)]
        struct Before<'a> {
            current: &'a Edit,
            id: &'a Option<String>,
            history: Vec<State<'a>>,
            snapshots: &'a [Snapshot],
            saved: u64,
        }
        let before = Before {
            current: &plain.current,
            id: &plain.current_id,
            saved: plain.saved,
            history: plain
                .history
                .iter()
                .map(|s| State {
                    edit: &s.edit,
                    id: &s.id,
                })
                .collect(),
            snapshots: &plain.snapshots,
        };
        assert_eq!(json, serde_json::to_string_pretty(&before).unwrap());
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// A build from before labels reads each state of a labeled
    /// sidecar as the plain edit it is: what an edit does not know it
    /// ignores, so the history loads whole and the words are dropped.
    #[test]
    fn a_build_before_labels_reads_a_labelled_history_as_plain_edits() {
        let mut sidecar = Sidecar::default();
        sidecar.record_as(exposed(1.0), Some("Preset: Faded film".into()));
        sidecar.record_as(exposed(2.0), Some("Snapshot: Warm".into()));
        let json = serde_json::to_string(&sidecar).unwrap();
        #[derive(Deserialize)]
        struct Before {
            current: Edit,
            history: Vec<Edit>,
        }
        let old: Before = serde_json::from_str(&json).unwrap();
        assert_eq!(old.current, sidecar.current);
        let edits: Vec<Edit> = sidecar.history.iter().map(|s| s.edit.clone()).collect();
        assert_eq!(old.history, edits);
    }

    /// A sidecar written before labels, of every schema version,
    /// loads with none and its rows read as they did.
    #[test]
    fn an_old_sidecar_loads_with_no_labels_and_its_rows_read_as_before() {
        let dir = scratch("old-labels");
        let raw = dir.join("IMG_0004.CR3");
        std::fs::write(
            Sidecar::path_for(&raw),
            r#"{"current":{"version":4,"light":{"exposure":1.0,"tone":{"contrast":0.2}}},
               "history":[{"version":1},
                          {"version":2,"light":{"exposure":0.5}},
                          {"version":3,"light":{"exposure":1.0}}],
               "snapshots":[{"name":"a","taken":0,"edit":{"version":4}}]}"#,
        )
        .unwrap();
        let old = Sidecar::load(&raw).unwrap().unwrap();
        assert_eq!(old.current_label, None);
        assert!(old.history.iter().all(|s| s.label.is_none()));
        for i in 1..old.states() {
            let (before, after) = (old.state(i - 1).unwrap(), old.state(i).unwrap());
            assert_eq!(
                old.describe(i),
                Some(describe(before, after, &old.snapshots)),
                "row {i}"
            );
        }
        assert_eq!(old.describe(1).as_deref(), Some("Exposure +0.50"));
        assert_eq!(old.describe(0), None);
        // A `step` that is not words costs the label, not the state.
        std::fs::write(
            Sidecar::path_for(&raw),
            r#"{"current":{"version":4},"step":7,
               "history":[{"version":4,"light":{"exposure":1.0},"step":["x"]}]}"#,
        )
        .unwrap();
        let odd = Sidecar::load(&raw).unwrap().unwrap();
        assert_eq!(odd.current_label, None);
        // Written before the baselines per body, it keeps the zero it
        // was made against.
        let mut want = exposed(1.0);
        want.light.baseline = Some(baseline::BEFORE_BODIES);
        assert_eq!(odd.history[0], want);
        assert_eq!(odd.history[0].label, None);
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// The words go with the state they made: undo takes them to the
    /// redo stack and redo brings them back; a new step after an
    /// undo drops what was undone, words and all.
    #[test]
    fn undo_carries_a_label_and_a_new_step_drops_the_redos() {
        let mut sidecar = Sidecar::default();
        sidecar.record(exposed(1.0));
        sidecar.record_as(exposed(2.0), Some("Preset: Faded film".into()));
        assert!(sidecar.undo());
        assert_eq!(sidecar.current_label, None);
        assert_eq!(sidecar.redo[0].label.as_deref(), Some("Preset: Faded film"));
        // The undone row still reads by its words.
        assert_eq!(sidecar.describe(2).as_deref(), Some("Preset: Faded film"));
        assert!(sidecar.redo());
        assert_eq!(sidecar.current_label.as_deref(), Some("Preset: Faded film"));
        assert!(sidecar.undo());
        // A new step: the preset's state and its words are gone.
        sidecar.record(exposed(3.0));
        assert!(sidecar.redo.is_empty());
        assert_eq!(sidecar.current_label, None);
        assert!(
            (0..sidecar.states()).all(|i| sidecar.label(i).is_none()),
            "no state keeps the dropped words"
        );
        assert_eq!(sidecar.describe(2).as_deref(), Some("Exposure +3.00"));
        // A step that changes nothing records nothing, words included.
        assert!(!sidecar.record_as(exposed(3.0), Some("Preset: Faded film".into())));
        assert_eq!(sidecar.current_label, None);
        // go_to walks the words with the states.
        sidecar.record_as(exposed(4.0), Some("Sync from IMG_0001".into()));
        assert!(sidecar.go_to(0));
        assert_eq!(sidecar.label(3), Some("Sync from IMG_0001"));
        assert!(sidecar.go_to(3));
        assert_eq!(sidecar.current_label.as_deref(), Some("Sync from IMG_0001"));
    }

    /// A snapshot restored is a step named for the snapshot, and the
    /// row keeps its name after the snapshot is renamed, even when a
    /// single control is all that moved.
    #[test]
    fn a_snapshot_restored_is_named_for_it() {
        let mut sidecar = Sidecar::default();
        sidecar.record(exposed(1.0));
        sidecar.take_snapshot("Bright", 0);
        sidecar.record(exposed(-1.0));
        assert!(sidecar.restore_snapshot(0));
        assert_eq!(sidecar.current_label.as_deref(), Some("Snapshot: Bright"));
        sidecar.snapshots[0].name = "Renamed".into();
        assert_eq!(sidecar.describe(3).as_deref(), Some("Snapshot: Bright"));
    }

    /// A preset over a set and a sync record their words on each
    /// frame that took them, and none on a frame left alone.
    #[test]
    fn a_preset_and_a_sync_over_frames_record_their_words() {
        let mut sidecars = vec![Sidecar::default(), Sidecar::default(), Sidecar::default()];
        sidecars[2].record(exposed(1.0));
        let preset = Preset::from_edit("Bright", &exposed(1.0), &[Section::Light]);
        let moved = apply_preset_into(
            &mut sidecars,
            &preset,
            &[0, 1, 2],
            None,
            Some("Preset ×3: Bright"),
        );
        assert_eq!(moved, [0, 1]);
        for s in &sidecars[..2] {
            assert_eq!(s.current_label.as_deref(), Some("Preset ×3: Bright"));
            assert_eq!(s.describe(1).as_deref(), Some("Preset ×3: Bright"));
        }
        assert_eq!(sidecars[2].current_label, None);

        let moved = sync_into(
            &mut sidecars,
            &exposed(2.0),
            &[1, 2],
            &[Section::Light],
            Some("Sync from IMG_0001"),
        );
        assert_eq!(moved, [1, 2]);
        for s in &sidecars[1..] {
            assert_eq!(s.current_label.as_deref(), Some("Sync from IMG_0001"));
        }
        assert_eq!(sidecars[1].label(1), Some("Preset ×3: Bright"));
    }

    /// `Step` takes a `step` key off each state's object before the
    /// edit reads the rest, so no field of the edit may be called
    /// that: a fully populated edit, every section and list filled,
    /// writes no such key among its own.
    #[test]
    fn no_field_of_an_edit_is_called_step() {
        let mut edit = Edit::default();
        edit.light.exposure = 1.0;
        edit.white_balance = WhiteBalance::Custom {
            temperature: 5000.0,
            tint: 0.01,
        };
        edit.geometry.crop = Some(geometry::Crop {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 0.5,
        });
        edit.adjustments.push(Adjustment::default());
        let tree: serde_json::Value = serde_json::from_str(&edit.to_json()).unwrap();
        let keys: Vec<&String> = tree.as_object().unwrap().keys().collect();
        assert!(!keys.iter().any(|k| *k == "step"), "{keys:?}");
        // And a step with no words, blank or none, writes none.
        let mut sidecar = Sidecar::default();
        sidecar.record_as(exposed(1.0), Some("  ".into()));
        assert_eq!(sidecar.current_label, None);
        sidecar.record(exposed(2.0));
        let json = serde_json::to_string(&sidecar).unwrap();
        assert!(!json.contains("\"step\""), "{json}");
        // The key sits just after `current` when there is one.
        sidecar.record_as(exposed(3.0), Some("Preset: Faded film".into()));
        let json = serde_json::to_string(&sidecar).unwrap();
        let (current, step, history) = (
            json.find("\"current\"").unwrap(),
            json.find("\"step\"").unwrap(),
            json.find("\"history\"").unwrap(),
        );
        assert!(current < step && step < history, "{json}");
    }

    /// The export records are written beside the states they ride on,
    /// read back whole, read one at a time when some will not read,
    /// and are nothing at all to a build that does not know them.
    #[test]
    fn export_records_round_trip_and_an_older_build_reads_past_them() {
        let exposed = |ev: f32| {
            let mut e = Edit::default();
            e.light.exposure = ev;
            e
        };
        let web = Exported {
            file: "/out/IMG_0001.jpg".into(),
            preset: Some("Web".into()),
            at: 1_790_000_000,
        };
        let mut sidecar = Sidecar::default();
        sidecar.record(exposed(1.0));
        assert!(sidecar.record_export(&exposed(1.0), web.clone()));
        sidecar.record(exposed(2.0));
        assert!(sidecar.record_export(
            &exposed(2.0),
            Exported {
                preset: None,
                ..web.clone()
            }
        ));
        let json = serde_json::to_string(&sidecar).unwrap();
        let back: Sidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sidecar);
        assert_eq!(back.history[1].exports, vec![web.clone()]);
        // No preset, no key; and none at all where there were none.
        assert_eq!(json.matches("\"preset\"").count(), 1, "{json}");
        assert_eq!(json.matches("\"exported\"").count(), 2, "{json}");

        // A build from before: the sidecar and each state read as they
        // did, the records unknown fields.
        #[derive(Deserialize)]
        struct Before {
            current: Edit,
            history: Vec<Edit>,
        }
        let before: Before = serde_json::from_str(&json).unwrap();
        assert_eq!(before.current, exposed(2.0));
        assert_eq!(before.history, vec![Edit::default(), exposed(1.0)]);

        // A record that will not read costs itself, not the others,
        // and a key that is not a list costs the records only.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["history"][1]["exported"] =
            serde_json::json!([{"file": 3}, {"file": "a.jpg", "at": "noon"}]);
        value["exported"] = serde_json::json!("lots");
        let back: Sidecar = serde_json::from_value(value).unwrap();
        assert_eq!(back.current, exposed(2.0));
        assert!(back.current_exports.is_empty());
        assert_eq!(
            back.history[1].exports,
            vec![Exported {
                file: "a.jpg".into(),
                preset: None,
                at: 0,
            }]
        );
    }

    /// Two states that merge (a source completing the earlier into
    /// the later) keep both states' exports, the earlier's first.
    #[test]
    fn states_that_merge_keep_both_their_exports() {
        let (placed, done) = a_placed_spot();
        let record = |file: &str| Exported {
            file: file.into(),
            ..Exported::default()
        };
        let mut sidecar = Sidecar::default();
        assert!(sidecar.record(placed.clone()));
        assert!(sidecar.record_export(&placed, record("first.jpg")));
        assert!(sidecar.record(done.clone()));
        assert!(sidecar.record_export(&done, record("second.jpg")));
        assert!(sidecar.undo());
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 2);
        let files: Vec<&str> = sidecar
            .current_exports
            .iter()
            .map(|e| e.file.as_str())
            .collect();
        assert_eq!(files, vec!["first.jpg", "second.jpg"]);
    }

    /// Two undone states that merge keep the later of their undo
    /// ticks, so a redo still weighs as undone last.
    #[test]
    fn undone_states_that_merge_keep_the_later_tick() {
        let (placed, done) = a_placed_spot();
        let mut sidecar = Sidecar::default();
        assert!(sidecar.record(placed));
        assert!(sidecar.record(done.clone()));
        assert!(sidecar.undo());
        assert!(sidecar.undo());
        let last = sidecar.redo.last().unwrap().order.tick();
        assert!(last > sidecar.redo[0].order.tick());
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.redo.len(), 1);
        assert_eq!(sidecar.redo[0].order.tick(), last);
    }

    /// Two states in the history that merge keep the later of their
    /// ticks: undone back to, the merged state was made current when
    /// the later was, which is what the next undo weighs it by.
    #[test]
    fn states_in_the_history_that_merge_keep_the_later_tick() {
        let (placed, done) = a_placed_spot();
        let mut sidecar = Sidecar::default();
        assert!(sidecar.record(placed.clone()));
        assert!(sidecar.record(done.clone()));
        let later = sidecar.current_order.tick();
        let mut brighter = placed;
        brighter.light.exposure = 0.5;
        assert!(sidecar.record(brighter));
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 3);
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, done);
        assert_eq!(sidecar.current_order.tick(), later);
    }

    /// Work done in an act's name stands where the act did: before an
    /// act made after it, however late the work runs.
    #[test]
    fn work_in_an_act_s_name_takes_the_act_s_ticks() {
        let pressed = Order::act();
        let after = Order::next();
        let (first, second) = Order::during(pressed, || (Order::next(), Order::next()));
        assert!(pressed.tick() < first.tick());
        assert!(first.tick() < second.tick());
        assert!(second.tick() < after.tick());
        // Out of the act, the clock goes on past every tick before.
        assert!(Order::next().tick() > after.tick());
        // A request queued inside the act is the act's too.
        assert_eq!(Order::during(pressed, Order::act).tick(), pressed.tick());
    }

    /// A clone spot as placed, waiting on its source, and as the
    /// engine completed it.
    fn a_placed_spot() -> (Edit, Edit) {
        let mut placed = Edit::default();
        placed.retouch.patches.push(retouch::Patch {
            id: placed.retouch.next_id(),
            method: retouch::Method::Clone,
            points: vec![[0.4, 0.6]],
            radius: 0.03,
            ..Default::default()
        });
        let mut done = placed.clone();
        done.retouch.patches[0].source = Some([0.05, -0.02]);
        (placed, done)
    }

    /// Issue 7: when the develop is slow, the panel's save records
    /// the spot before the engine has chosen its source. The source
    /// completes that state in place; it is not a step, so one undo
    /// takes the spot away and a redo brings it back, source and all.
    #[test]
    fn a_chosen_source_completes_the_state_rather_than_adding_one() {
        let mut sidecar = Sidecar::default();
        let (placed, done) = a_placed_spot();
        // The save timer fires first.
        assert!(sidecar.record(placed));
        // Then the develop lands with the source.
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 2);
        assert_eq!(sidecar.current, done);
        // A state that merges into the one before it keeps the later
        // id: the state is the completed one.
        let mut stepped = Sidecar::default();
        stepped.record(a_placed_spot().0);
        stepped.record(done.clone());
        let later = stepped.current_id.clone();
        assert!(stepped.take_sources(&done.retouch.patches));
        assert_eq!(stepped.states(), 2);
        assert_eq!(stepped.current_id, later);
        // The save the develop's arrival would have made finds
        // nothing new.
        assert!(!sidecar.record(done.clone()));
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
        // The undone state re-develops; it has no spot, so nothing
        // is chosen, and the redo stays.
        assert!(!sidecar.take_sources(&sidecar.current.retouch.patches.clone()));
        assert!(sidecar.redo());
        assert_eq!(sidecar.current, done);
        // The redone state re-develops too, and has its source.
        assert!(!sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 2);
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
    }

    /// A develop that lands before the save: the state is recorded
    /// with its source, and there is nothing to complete.
    #[test]
    fn a_source_chosen_before_the_save_is_recorded_with_the_spot() {
        let mut sidecar = Sidecar::default();
        let (_, done) = a_placed_spot();
        assert!(sidecar.record(done.clone()));
        assert!(!sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 2);
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
    }

    /// Every state still waiting on the same patch's source takes it,
    /// the undone ones too, so walking the line never re-develops a
    /// state that would want one.
    #[test]
    fn every_state_waiting_on_the_patch_takes_its_source() {
        let mut sidecar = Sidecar::default();
        let (placed, done) = a_placed_spot();
        assert!(sidecar.record(placed.clone()));
        let mut brighter = placed.clone();
        brighter.light.exposure = 0.5;
        assert!(sidecar.record(brighter));
        assert!(sidecar.undo());
        // The develop of the spot alone lands after the undo.
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.current, done);
        assert_eq!(sidecar.states(), 3);
        assert!(sidecar.redo());
        assert_eq!(sidecar.current.retouch, done.retouch);
        assert_eq!(sidecar.current.light.exposure, 0.5);
    }

    /// A spot that took a removed spot's id, somewhere else, is not
    /// handed the removed one's source.
    #[test]
    fn a_reused_patch_id_elsewhere_takes_no_source() {
        let mut sidecar = Sidecar::default();
        let (placed, done) = a_placed_spot();
        let mut elsewhere = placed.clone();
        elsewhere.retouch.patches[0].points = vec![[0.1, 0.1]];
        assert!(sidecar.record(elsewhere.clone()));
        assert!(!sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.current, elsewhere);
    }

    /// A line a build before the fix left, the spot recorded without
    /// its source and then with it: the first re-develop of the
    /// sourceless state completes it, it merges with the state after
    /// it, and the next undo takes the spot away.
    #[test]
    fn a_line_with_the_source_as_its_own_step_is_healed() {
        let mut sidecar = Sidecar::default();
        let (placed, done) = a_placed_spot();
        assert!(sidecar.record_as(placed.clone(), Some("Clone".into())));
        assert!(sidecar.record(done.clone()));
        assert_eq!(sidecar.states(), 3);
        assert!(sidecar.undo());
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 2);
        assert_eq!(sidecar.current, done);
        assert_eq!(sidecar.current_label.as_deref(), Some("Clone"));
        assert!(sidecar.redo.is_empty());
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
        assert!(sidecar.redo());
        assert_eq!(sidecar.current, done);

        // The same line walked from the far end: a redo onto the
        // sourceless state develops it, and the undone copy after it
        // merges in.
        let mut sidecar = Sidecar::default();
        assert!(sidecar.record_as(placed, Some("Clone".into())));
        assert!(sidecar.record(done.clone()));
        let mut later = done.clone();
        later.light.exposure = 1.0;
        assert!(sidecar.record(later.clone()));
        assert!(sidecar.undo());
        assert!(sidecar.undo());
        assert!(sidecar.undo());
        assert_eq!(sidecar.current, Edit::default());
        assert!(sidecar.redo());
        assert!(sidecar.take_sources(&done.retouch.patches));
        assert_eq!(sidecar.states(), 3);
        assert_eq!(sidecar.current, done);
        assert_eq!(sidecar.current_label.as_deref(), Some("Clone"));
        assert!(sidecar.redo());
        assert_eq!(sidecar.current, later);
        assert!(!sidecar.redo());
    }

    /// A rename reaches every state that names the look, the undone
    /// and the snapshots too; a state naming another look, one whose
    /// name starts with this one's included, is left as it was. The
    /// states before the first renamed one keep their ids, and from it
    /// on every id is made again from the state's content, as a join
    /// checks a chain.
    #[test]
    fn a_look_renamed_reaches_every_state_and_chains_new_ids() {
        use crate::look::{LookLut, LutChoice};
        let named = |n: &str| LookLut {
            lut: LutChoice::Named(n.into()),
            strength: 0.6,
        };
        let mut sidecar = Sidecar::default();
        let mut e = Edit {
            look_lut: named("Neon"),
            ..Edit::default()
        };
        sidecar.record(e.clone());
        sidecar.take_snapshot("Glow", 1_700_000_000);
        e.look_lut = named("Neon Film");
        sidecar.record(e.clone());
        e.look_lut = named("Neon.agx");
        sidecar.record(e.clone());
        e.look_lut = named("Neon");
        e.light.exposure = 0.5;
        sidecar.record(e.clone());
        e.light.exposure = 1.0;
        sidecar.record(e);
        assert!(sidecar.undo());
        sidecar.fill_ids();
        let ids = sync::state_ids(&sidecar);
        assert!(sidecar.names_look("Neon"));
        assert!(!sidecar.names_look("neon"));

        assert!(sidecar.rename_look("Neon", "Glow"));
        assert!(!sidecar.names_look("Neon"));
        let names: Vec<&str> = sidecar
            .history
            .iter()
            .map(|s| s.edit.look_lut.lut.name())
            .chain(std::iter::once(sidecar.current.look_lut.lut.name()))
            .collect();
        assert_eq!(names, ["none", "Glow", "Neon Film", "Neon.agx", "Glow"]);
        assert_eq!(sidecar.redo[0].edit.look_lut.lut.name(), "Glow");
        assert_eq!(sidecar.snapshots[0].edit.look_lut.lut.name(), "Glow");
        assert_eq!(sidecar.current.look_lut.strength, 0.6);
        let renamed = sync::state_ids(&sidecar);
        assert_eq!(renamed[0], ids[0]);
        assert!(renamed[1..].iter().all(|id| !ids.contains(id)));
        // Every id is what its content and its parent make, the redo
        // stack's chained on from the current state.
        let mut fresh = sidecar.clone();
        for s in &mut fresh.history {
            s.id = None;
        }
        fresh.current_id = None;
        assert_eq!(sync::state_ids(&fresh), renamed);
        let redo = &sidecar.redo[0];
        assert_eq!(
            redo.id.as_deref(),
            Some(sync::state_id(&renamed[4], &redo.edit, redo.label.as_deref()).as_str())
        );
        // Nothing to rename is said so, and changes nothing.
        let before = sidecar.clone();
        assert!(!sidecar.rename_look("Neon", "Other"));
        assert_eq!(sidecar, before);
    }
}
