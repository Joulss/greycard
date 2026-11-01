//! The copies of the collections and stacks a pass found beside the
//! frames (`crate::carried`), joined into the window's own: a set new
//! here added, one ahead taken, two that went their own ways both kept,
//! a deletion that saw ours taking it. The files beside the library
//! stay this machine's source; what is taken is written there.

use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;

use greycard_library::carry::{self, Item, Parsed};

use crate::*;

/// What a join of carried copies changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Took {
    /// The collections and smart collections added, changed, or gone.
    pub(crate) collections: Vec<String>,
    /// The stacks the same.
    pub(crate) stacks: Vec<String>,
}

impl Took {
    pub(crate) fn is_empty(&self) -> bool {
        self.collections.is_empty() && self.stacks.is_empty()
    }
}

fn ours(
    collections: &crate::collections::Store,
    stacks: &crate::stacks::Store,
    id: &str,
) -> Option<Item> {
    collections
        .item(id)
        .or_else(|| stacks.stack(id).cloned().map(Item::Stack))
}

/// The copies in `files` joined into `collections` and `stacks`.
/// Deletions first: one that saw ours takes it; one of a set not here
/// is kept, so a copy it saw met later is not taken. Then each copy: a
/// set a deletion here saw is passed over, the deletion carried to the
/// folder the copy is in; one new here is added; one of ours is joined
/// with it (`carry::join`), ours ahead written again to the folders
/// that hold the copy behind.
pub(crate) fn take(
    collections: &mut crate::collections::Store,
    stacks: &mut crate::stacks::Store,
    files: &[carry::File],
) -> Took {
    let mut took = Took::default();
    let carrier = collections.carrier.clone();
    // Each copy with the folder it was found in; a conflict's copy,
    // made here, with none.
    let mut items: VecDeque<(Item, Option<&Path>)> = VecDeque::new();
    let mut gones = Vec::new();
    for f in files {
        match f.parse() {
            Ok(Parsed::Item(i)) => items.push_back((i, Some(f.folder.as_path()))),
            Ok(Parsed::Gone(g)) => gones.push(g),
            Ok(Parsed::Later) => tracing::info!(
                "carried set {} in {}: a later build's, left alone",
                f.name,
                f.folder.display()
            ),
            Err(e) => tracing::warn!("carried set {} in {}: {e}", f.name, f.folder.display()),
        }
    }
    let said = |item: &Item, took: &mut Took| {
        let id = item.id().to_string();
        let list = match item {
            Item::Stack(_) => &mut took.stacks,
            _ => &mut took.collections,
        };
        if !list.contains(&id) {
            list.push(id);
        }
    };
    for g in gones {
        match ours(collections, stacks, &g.id) {
            Some(o) if g.covers(o.lineage()) => {
                tracing::info!(
                    "carried set {}: deleted on another machine; deleted here",
                    g.id
                );
                match &o {
                    Item::Stack(_) => {
                        stacks.unstack(&g.id);
                    }
                    _ => {
                        collections.delete(&g.id);
                    }
                }
                said(&o, &mut took);
            }
            // Edited since the deletion saw it: the edit stands, and
            // goes back over the deletion's file.
            Some(o) => {
                if let Some(c) = &carrier {
                    c.refresh(o);
                }
            }
            None => {
                if let Some(c) = &carrier
                    && !c.knows_gone(&g.id)
                {
                    c.bury(g);
                }
            }
        }
    }
    // A conflict's copy is joined in its turn; each join makes one at
    // most, so this ends, and the cap only guards against a bug.
    let mut turns = items.len() * 4 + 16;
    while let Some((theirs, found_in)) = items.pop_front() {
        let theirs_id = theirs.id().to_string();
        turns -= 1;
        if turns == 0 {
            tracing::warn!("carried sets: joins did not settle; the rest left");
            break;
        }
        // A copy a deletion here has seen: passed over, and the deletion
        // carried to the folder it was found in.
        if let Some(c) = &carrier
            && c.buried(&theirs)
        {
            if let Some(folder) = found_in {
                c.bury_found(&theirs, folder);
            }
            continue;
        }
        if matches!(&theirs, Item::Stack(s) if s.refs.len() < 2) {
            continue;
        }
        let joined = match ours(collections, stacks, theirs.id()) {
            None => {
                tracing::info!(
                    "carried set {}: new from {}",
                    theirs.id(),
                    theirs.lineage().host
                );
                carry::Joined {
                    keep: Some(theirs),
                    copy: None,
                }
            }
            Some(o) => {
                let order = carry::compare(o.lineage(), theirs.lineage());
                if order == carry::Order::Ahead
                    && let Some(c) = &carrier
                {
                    c.refresh(o.clone());
                }
                let mut j = carry::join(&o, &theirs);
                // The same content with revisions to take in: written,
                // and nothing for the window to show.
                if order == carry::Order::Same
                    && let Some(keep) = j.keep.take()
                {
                    match keep {
                        Item::Stack(s) => stacks.take(s),
                        other => {
                            collections.take(other);
                        }
                    }
                }
                j
            }
        };
        if let Some(keep) = joined.keep {
            said(&keep, &mut took);
            match keep {
                Item::Stack(s) => stacks.take(s),
                other => {
                    collections.take(other);
                }
            }
        }
        if let Some(copy) = joined.copy {
            tracing::info!("carried set {theirs_id}: went its own way on two machines; both kept");
            items.push_back((copy, None));
        }
    }
    took
}

/// The carried sets a pass found, joined, and the window brought up to
/// what changed: the collections' pane and marks, the view of one
/// changed, the stacks over the list.
pub(crate) fn landed(state: &Rc<Shared>, app: &App, files: &[carry::File]) {
    if files.is_empty() {
        return;
    }
    let viewing = {
        let mut st = state.borrow_mut();
        let st = &mut *st;
        let took = take(&mut st.views.collections, &mut st.views.stacks, files);
        if took.is_empty() {
            return;
        }
        if !took.collections.is_empty() {
            crate::panel::collections::members_changed(st, app);
        }
        if !took.stacks.is_empty()
            && let Some(worker) = crate::WORKER.with(|w| w.borrow().clone())
        {
            crate::panel::stacks::settle(st, app, &worker);
        }
        st.views
            .view
            .collection()
            .is_some_and(|id| took.collections.iter().any(|c| c == id))
    };
    if viewing && let Some(worker) = crate::WORKER.with(|w| w.borrow().clone()) {
        crate::roots::refresh_view(state, app, &worker);
    }
}

/// The leftovers a pass found in the carried sets' folders
/// (`Report::sets_stale`), given to the carrier to clear.
pub(crate) fn leftovers(state: &Rc<Shared>, paths: &[std::path::PathBuf]) {
    if paths.is_empty() {
        return;
    }
    if let Some(c) = &state.borrow().views.collections.carrier {
        c.clear(paths.to_vec());
    }
}

/// The carrier for the library at `library`, given to its collections
/// and its stacks, and every set carried as it stands: a set made
/// before the carry, or whose write failed, goes now.
pub(crate) fn attach(st: &mut State, library: &Path) {
    let dir = crate::collections::Store::dir_beside(library);
    let roots = Some(greycard_library::Roots::path_beside(library));
    #[cfg(not(test))]
    let carrier = crate::carried::Carrier::start(&dir, roots);
    #[cfg(test)]
    let carrier = crate::carried::Carrier::in_place(&dir, roots);
    let carrier = Rc::new(carrier);
    st.views.collections.carrier = Some(carrier.clone());
    st.views.stacks.carrier = Some(carrier);
    st.views.collections.carry_all();
    st.views.stacks.carry_all();
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use greycard_library::collections::Reference;
    use greycard_library::stacks::Stack;

    use super::*;
    use crate::carried::Carrier;

    /// One machine: its data folder, and its collections and stacks
    /// there, carried in place.
    struct Machine {
        collections: crate::collections::Store,
        stacks: crate::stacks::Store,
    }

    fn machine(data: &Path) -> Machine {
        let dir = data.join(crate::collections::DIR);
        machine_with(data, Carrier::in_place(&dir, None))
    }

    fn machine_with(data: &Path, carrier: Carrier) -> Machine {
        let dir = data.join(crate::collections::DIR);
        let carrier = Rc::new(carrier);
        let mut collections = crate::collections::Store::load(dir.clone());
        collections.carrier = Some(carrier.clone());
        let mut stacks = crate::stacks::Store::load(greycard_library::stacks::dir_in(&dir));
        stacks.carrier = Some(carrier);
        Machine {
            collections,
            stacks,
        }
    }

    /// A shoot of `n` frames on disk, each a few bytes of its own.
    fn shoot(dir: &Path, n: usize) -> Vec<PathBuf> {
        std::fs::create_dir_all(dir).unwrap();
        (0..n)
            .map(|i| {
                let p = dir.join(format!("IMG_000{i}.CR3"));
                std::fs::write(&p, format!("frame {i}")).unwrap();
                p
            })
            .collect()
    }

    fn r(i: usize, path: &Path) -> Reference {
        Reference {
            hash: format!("hash{i}"),
            hint: crate::collections::hint_of(path),
            version: None,
        }
    }

    fn copy_folder(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                copy_folder(&e.path(), &to.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }

    /// What a pass of the desk's index over `folder` brings.
    fn pass(lib: &mut greycard_library::Library, folder: &Path) -> Vec<carry::File> {
        lib.index_tree(folder, &mut |_| {}).unwrap().sets
    }

    #[test]
    fn a_set_gathered_on_the_laptop_joins_the_desks_own_through_a_backup() {
        let dir = crate::testing::scratch_dir("carried-two");
        let shoot_dir = dir.join("laptop-photos").join("shoot");
        let frames = shoot(&shoot_dir, 4);
        let mut laptop = machine(&dir.join("laptop-data"));
        let mut desk = machine(&dir.join("desk-data"));

        // The write: on the laptop, a collection of two frames and a
        // stack of three, each written beside the frames.
        let mut picks = crate::collections::Collection::new("Clients/Picks");
        picks.refs = vec![r(0, &frames[0]), r(2, &frames[2])];
        let picks_id = picks.id.clone();
        laptop.collections.list.push(picks);
        laptop.collections.save(&picks_id);
        let stack_id =
            laptop
                .stacks
                .make(vec![r(3, &frames[3]), r(1, &frames[1]), r(2, &frames[2])]);
        laptop.stacks.set_open(&stack_id, true);
        let names: Vec<String> = carry::read_folder(&shoot_dir)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(
            names,
            {
                let mut n = vec![format!("{picks_id}.json"), format!("{stack_id}.stack.json")];
                n.sort();
                n
            },
            "written beside the frames"
        );

        // The desk has a set of its own, and the shoot through a backup.
        let mut own = crate::collections::Collection::new("Desk set");
        own.refs = vec![r(9, &dir.join("elsewhere.CR3"))];
        let own_id = own.id.clone();
        desk.collections.list.push(own);
        desk.collections.save(&own_id);
        let archive = dir.join("archive").join("shoot");
        copy_folder(&shoot_dir, &archive);
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        let found = pass(&mut lib, &archive);
        assert_eq!(found.len(), 2);
        let took = take(&mut desk.collections, &mut desk.stacks, &found);
        assert_eq!(took.collections, std::slice::from_ref(&picks_id));
        assert_eq!(took.stacks, std::slice::from_ref(&stack_id));

        // The join: the desk's own kept, the laptop's added, its frames
        // at the desk's copy of the shoot.
        let mut names: Vec<&str> = desk
            .collections
            .list
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        names.sort();
        assert_eq!(names, ["Desk set", "Picks"]);
        let carried = desk.collections.collection(&picks_id).unwrap();
        assert_eq!(carried.set, "Clients");
        assert_eq!(
            carried
                .refs
                .iter()
                .map(|r| r.hint.clone())
                .collect::<Vec<_>>(),
            [
                crate::collections::hint_of(&archive.join("IMG_0000.CR3")),
                crate::collections::hint_of(&archive.join("IMG_0002.CR3")),
            ]
        );
        // The stack, carried, its top first and closed on the desk.
        let s = desk.stacks.stack(&stack_id).unwrap();
        assert!(!s.open, "open is the laptop's own");
        assert_eq!(
            s.refs[0].hint,
            crate::collections::hint_of(&archive.join("IMG_0003.CR3"))
        );
        // Written into the desk's own files, and read back there.
        let back = machine(&dir.join("desk-data"));
        assert_eq!(back.collections.list.len(), 2);
        assert_eq!(back.stacks.list.len(), 1);

        // Read again, nothing doubles: the same copies are the same sets.
        let again = carry::read_folder(&archive);
        assert!(take(&mut desk.collections, &mut desk.stacks, &again).is_empty());
        assert_eq!(desk.collections.list.len(), 2);
        // And the desk's own went nowhere near the laptop's.
        assert!(laptop.collections.collection(&own_id).is_none());
        crate::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn a_collection_edited_on_both_machines_is_kept_as_both() {
        let dir = crate::testing::scratch_dir("carried-conflict");
        let shoot_dir = dir.join("shoot");
        let frames = shoot(&shoot_dir, 3);
        let mut laptop = machine(&dir.join("laptop-data"));
        let mut desk = machine(&dir.join("desk-data"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        laptop.collections.list.push(picks);
        laptop.collections.save(&id);
        take(
            &mut desk.collections,
            &mut desk.stacks,
            &carry::read_folder(&shoot_dir),
        );
        assert_eq!(desk.collections.list.len(), 1);
        // Each goes its own way: the laptop adds a frame, the desk
        // renames. The laptop's copy is in the folder; the desk's write
        // leaves it, being apart from it.
        laptop
            .collections
            .collection_mut(&id)
            .unwrap()
            .refs
            .push(r(2, &frames[2]));
        laptop.collections.save(&id);
        let laptop_copy = carry::read_folder(&shoot_dir);
        desk.collections.collection_mut(&id).unwrap().name = "Selects".into();
        desk.collections.save(&id);
        let took = take(&mut desk.collections, &mut desk.stacks, &laptop_copy);
        assert_eq!(took.collections.len(), 2, "{took:?}");
        let mut got: Vec<(String, usize)> = desk
            .collections
            .list
            .iter()
            .map(|c| (c.name.clone(), c.refs.len()))
            .collect();
        got.sort();
        // Which keeps the id is the later's; both machines, one host here.
        let host = crate::carried::host();
        let laptop_kept = [
            ("Picks".to_string(), 3),
            (format!("Selects (on {host})"), 2),
        ];
        let desk_kept = [
            (format!("Picks (on {host})"), 3),
            ("Selects".to_string(), 2),
        ];
        assert!(got == laptop_kept || got == desk_kept, "{got:?}");
        // The folder now holds the desk's, which has seen the laptop's:
        // the laptop takes it and makes the same copy, and the two agree.
        let now = carry::read_folder(&shoot_dir);
        take(&mut laptop.collections, &mut laptop.stacks, &now);
        let mut on_laptop: Vec<(String, String)> = laptop
            .collections
            .list
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
        let mut on_desk: Vec<(String, String)> = desk
            .collections
            .list
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
        on_laptop.sort();
        on_desk.sort();
        assert_eq!(on_laptop, on_desk);
        // And met again either way, nothing more is made.
        assert!(
            take(
                &mut desk.collections,
                &mut desk.stacks,
                &carry::read_folder(&shoot_dir)
            )
            .is_empty()
        );
        crate::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn a_deletion_is_carried_and_an_older_copy_does_not_bring_it_back() {
        let dir = crate::testing::scratch_dir("carried-gone");
        let shoot_dir = dir.join("shoot");
        let frames = shoot(&shoot_dir, 2);
        let mut laptop = machine(&dir.join("laptop-data"));
        let mut desk = machine(&dir.join("desk-data"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        laptop.collections.list.push(picks);
        laptop.collections.save(&id);
        let before = carry::read_folder(&shoot_dir);
        take(&mut desk.collections, &mut desk.stacks, &before);
        assert!(desk.collections.collection(&id).is_some());
        laptop.collections.delete(&id);
        let gone = carry::read_folder(&shoot_dir);
        assert_eq!(gone.len(), 1);
        assert!(gone[0].name.ends_with(".gone.json"));
        let took = take(&mut desk.collections, &mut desk.stacks, &gone);
        assert_eq!(took.collections, std::slice::from_ref(&id));
        assert!(desk.collections.collection(&id).is_none());
        // The copy from before, met again on either machine, stays gone.
        assert!(take(&mut laptop.collections, &mut laptop.stacks, &before).is_empty());
        assert!(take(&mut desk.collections, &mut desk.stacks, &before).is_empty());
        crate::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn a_stack_carried_shows_on_the_desk_and_its_changes_follow() {
        let dir = crate::testing::scratch_dir("carried-stack");
        let shoot_dir = dir.join("shoot");
        let frames = shoot(&shoot_dir, 3);
        let mut laptop = machine(&dir.join("laptop-data"));
        let mut desk = machine(&dir.join("desk-data"));
        let id = laptop
            .stacks
            .make(vec![r(0, &frames[0]), r(1, &frames[1]), r(2, &frames[2])]);
        take(
            &mut desk.collections,
            &mut desk.stacks,
            &carry::read_folder(&shoot_dir),
        );
        desk.stacks.set_open(&id, true);
        // A new top on the laptop reaches the desk, which stays open.
        laptop.stacks.set_top_at(&id, 2);
        let took = take(
            &mut desk.collections,
            &mut desk.stacks,
            &carry::read_folder(&shoot_dir),
        );
        assert_eq!(took.stacks, std::slice::from_ref(&id));
        let s: &Stack = desk.stacks.stack(&id).unwrap();
        assert!(s.open);
        assert_eq!(s.refs[0].hash, "hash2");
        // Unstacked on the desk: gone on the laptop too.
        desk.stacks.unstack(&id);
        take(
            &mut laptop.collections,
            &mut laptop.stacks,
            &carry::read_folder(&shoot_dir),
        );
        assert!(laptop.stacks.stack(&id).is_none());
        crate::testing::remove_dir_retry(&dir);
    }

    #[test]
    fn a_later_builds_copy_is_neither_taken_nor_written_over() {
        let dir = crate::testing::scratch_dir("carried-later");
        let shoot_dir = dir.join("shoot");
        let frames = shoot(&shoot_dir, 1);
        let mut desk = machine(&dir.join("desk-data"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0])];
        let id = picks.id.clone();
        desk.collections.list.push(picks);
        desk.collections.save(&id);
        // A later build bumps the shape of the copy in the folder.
        let file = carry::sets_dir(&shoot_dir).join(format!("{id}.json"));
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        v["version"] = 99.into();
        v["name"] = "From a later build".into();
        let later = serde_json::to_string_pretty(&v).unwrap();
        std::fs::write(&file, &later).unwrap();
        assert!(
            take(
                &mut desk.collections,
                &mut desk.stacks,
                &carry::read_folder(&shoot_dir)
            )
            .is_empty()
        );
        assert_eq!(desk.collections.collection(&id).unwrap().name, "Picks");
        // A change here is kept here, and the later build's file stands.
        desk.collections.collection_mut(&id).unwrap().name = "Renamed".into();
        desk.collections.save(&id);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), later);
        crate::testing::remove_dir_retry(&dir);
    }

    impl Machine {
        /// A pass of this machine's own index over `folders`, joined.
        fn pass(&mut self, lib: &mut greycard_library::Library, folders: &[&Path]) -> bool {
            let mut files = Vec::new();
            for f in folders {
                files.extend(lib.index_tree(f, &mut |_| {}).unwrap().sets);
            }
            !take(&mut self.collections, &mut self.stacks, &files).is_empty()
        }

        fn names(&self) -> Vec<(String, String)> {
            let mut v: Vec<_> = self
                .collections
                .list
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect();
            v.sort();
            v
        }

        fn stacks(&self) -> Vec<(String, Vec<String>)> {
            let mut v: Vec<_> = self
                .stacks
                .list
                .iter()
                .map(|s| {
                    (
                        s.id.clone(),
                        s.refs.iter().map(|r| r.hash.clone()).collect(),
                    )
                })
                .collect();
            v.sort();
            v
        }
    }

    /// Both machines pass over `folders` until neither takes anything.
    fn settle(a: &mut Machine, b: &mut Machine, folders: &[&Path]) {
        let mut libs = [
            greycard_library::Library::open_in_memory().unwrap(),
            greycard_library::Library::open_in_memory().unwrap(),
        ];
        for _ in 0..8 {
            let [la, lb] = &mut libs;
            let x = b.pass(lb, folders);
            let y = a.pass(la, folders);
            if !x && !y {
                return;
            }
        }
        panic!("did not settle: {:?} {:?}", a.names(), b.names());
    }

    #[test]
    fn a_frame_stacked_on_two_machines_ends_in_one_stack() {
        let dir = crate::testing::scratch_dir("carried-two-stacks");
        let f = dir.join("shoot");
        let frames = shoot(&f, 4);
        let mut a = machine(&dir.join("a"));
        let mut b = machine(&dir.join("b"));
        a.stacks.make(vec![r(0, &frames[0]), r(1, &frames[1])]);
        b.stacks.make(vec![r(1, &frames[1]), r(2, &frames[2])]);
        settle(&mut a, &mut b, &[&f]);
        assert_eq!(a.stacks(), b.stacks());
        let holding = a
            .stacks()
            .iter()
            .filter(|(_, m)| m.contains(&"hash1".to_string()))
            .count();
        assert_eq!(holding, 1, "{:?}", a.stacks());
        crate::testing::remove_dir_retry(&dir);
    }

    /// A deletion is kept before the window goes on: a quit while the
    /// writer waits on a share that does not answer does not bring the
    /// set back at the next launch.
    #[cfg(unix)]
    #[test]
    fn a_deletion_survives_a_quit_while_the_writer_is_held_up() {
        let dir = crate::testing::scratch_dir("carried-quit");
        let f = dir.join("shoot");
        let slow = dir.join("slow");
        let frames = shoot(&f, 2);
        let other = shoot(&slow, 1);
        let data = dir.join("a");
        let mut a = machine(&data);
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        a.collections.list.push(picks);
        a.collections.save(&id);
        drop(a);
        let collections = data.join(crate::collections::DIR);
        let mut a = machine_with(
            &data,
            Carrier::start_limited(&collections, std::time::Duration::from_millis(300)),
        );
        // A folder whose share stalls: its deletion's file a pipe no one
        // writes to, so a read of it never returns.
        let mut y = crate::collections::Collection::new("Slow");
        y.refs = vec![r(9, &other[0])];
        let yid = y.id.clone();
        std::fs::create_dir_all(carry::sets_dir(&slow)).unwrap();
        let fifo = carry::sets_dir(&slow).join(format!("{yid}.gone.json"));
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        a.collections.list.push(y);
        a.collections.save(&yid);
        a.collections.delete(&id);
        drop(a);
        std::fs::remove_file(&fifo).unwrap();
        let mut a = machine(&data);
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        assert!(!a.pass(&mut lib, &[&f]));
        assert!(a.collections.collection(&id).is_none());
        crate::testing::remove_dir_retry(&dir);
    }

    /// An edit whose write failed, the set deleted on the other machine
    /// meanwhile: the edit stands on both once each has passed.
    #[cfg(unix)]
    #[test]
    fn an_edit_whose_write_failed_outlives_a_deletion_made_elsewhere() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::testing::scratch_dir("carried-failed-edit");
        let f = dir.join("shoot");
        let frames = shoot(&f, 2);
        let mut a = machine(&dir.join("a"));
        let mut b = machine(&dir.join("b"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        a.collections.list.push(picks);
        a.collections.save(&id);
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        b.pass(&mut lib, &[&f]);
        let sets = carry::sets_dir(&f);
        std::fs::set_permissions(&sets, std::fs::Permissions::from_mode(0o555)).unwrap();
        b.collections.collection_mut(&id).unwrap().name = "Edited offline".into();
        b.collections.save(&id);
        std::fs::set_permissions(&sets, std::fs::Permissions::from_mode(0o755)).unwrap();
        a.collections.delete(&id);
        settle(&mut a, &mut b, &[&f]);
        assert_eq!(a.names(), b.names());
        assert_eq!(a.names(), [(id, "Edited offline".to_string())]);
        crate::testing::remove_dir_retry(&dir);
    }

    /// A set taken from a backup on the desk, edited there, reaches the
    /// laptop through the same backup.
    #[test]
    fn an_edit_on_the_desk_of_a_set_from_a_backup_reaches_the_laptop() {
        let dir = crate::testing::scratch_dir("carried-backup-edit");
        let (local, archive) = (dir.join("laptop-local"), dir.join("archive"));
        let src = local.join("shoot");
        let dest = archive.join("2026").join("shoot");
        let frames = shoot(&src, 2);
        std::fs::create_dir_all(&dest).unwrap();
        for f in &frames {
            std::fs::copy(f, dest.join(f.file_name().unwrap())).unwrap();
        }
        let mut roots = greycard_library::Roots::default();
        roots.add(&local).unwrap();
        roots.add(&archive).unwrap();
        roots.set_archive(&archive, true);
        roots.set_backup_folder(&src, &archive, &dest);
        let roots_file = dir.join("a").join("roots.json");
        std::fs::create_dir_all(roots_file.parent().unwrap()).unwrap();
        roots.save(&roots_file).unwrap();
        let collections = dir.join("a").join(crate::collections::DIR);
        let mut a = machine_with(
            &dir.join("a"),
            Carrier::in_place(&collections, Some(roots_file)),
        );
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        a.collections.list.push(picks);
        a.collections.save(&id);
        let mut b = machine(&dir.join("b"));
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        b.pass(&mut lib, &[&dest]);
        assert_eq!(
            b.collections.collection(&id).unwrap().refs[0].hint,
            crate::collections::hint_of(&dest.join("IMG_0000.CR3")),
            "the desk's own copy of the frame"
        );
        b.collections.collection_mut(&id).unwrap().name = "Renamed on desk".into();
        b.collections.save(&id);
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        a.pass(&mut lib, &[&dest]);
        assert_eq!(a.names(), b.names());
        crate::testing::remove_dir_retry(&dir);
    }

    /// A set deleted on its first save, while that save's write is still
    /// out on a share that does not answer: the folder already written
    /// takes the deletion, so another machine does not take the set.
    #[cfg(unix)]
    #[test]
    fn a_deletion_while_the_first_write_is_out_reaches_the_folders_it_wrote() {
        let dir = crate::testing::scratch_dir("carried-in-flight");
        let (fast, slow) = (dir.join("a-fast"), dir.join("b-slow"));
        let ff = shoot(&fast, 1);
        let sf = shoot(&slow, 1);
        let data = dir.join("a");
        let collections = data.join(crate::collections::DIR);
        let mut a = machine_with(
            &data,
            Carrier::start_limited(&collections, std::time::Duration::from_secs(2)),
        );
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &ff[0]), r(1, &sf[0])];
        let id = picks.id.clone();
        // The slow folder's deletion file a pipe no one writes to: its
        // read never returns.
        std::fs::create_dir_all(carry::sets_dir(&slow)).unwrap();
        let fifo = carry::sets_dir(&slow).join(format!("{id}.gone.json"));
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        a.collections.list.push(picks);
        a.collections.save(&id);
        std::thread::sleep(std::time::Duration::from_millis(500));
        a.collections.delete(&id);
        let carrier = a.collections.carrier.clone().unwrap();
        assert!(carrier.wait(std::time::Duration::from_secs(10)));
        std::fs::remove_file(&fifo).unwrap();
        let names: Vec<String> = carry::read_folder(&fast)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, [format!("{id}.gone.json")]);
        let mut b = machine(&dir.join("b"));
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        b.pass(&mut lib, &[&fast]);
        assert!(b.names().is_empty(), "{:?}", b.names());
        crate::testing::remove_dir_retry(&dir);
    }

    /// A write that did not answer in time and finished after: the
    /// folder it reached is kept as one the set may be in, so when its
    /// frames leave that folder the copy is taken out of it.
    #[cfg(unix)]
    #[test]
    fn a_write_that_answers_late_is_taken_out_when_its_frames_leave() {
        let dir = crate::testing::scratch_dir("carried-late");
        let (fast, slow) = (dir.join("a-fast"), dir.join("b-slow"));
        let ff = shoot(&fast, 1);
        let sf = shoot(&slow, 1);
        let data = dir.join("a");
        let collections = data.join(crate::collections::DIR);
        let mut a = machine_with(
            &data,
            Carrier::start_limited(&collections, std::time::Duration::from_millis(300)),
        );
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &ff[0]), r(1, &sf[0])];
        let id = picks.id.clone();
        // The slow folder's deletion file a pipe: its read waits until
        // something writes to it, past the folder's limit.
        std::fs::create_dir_all(carry::sets_dir(&slow)).unwrap();
        let fifo = carry::sets_dir(&slow).join(format!("{id}.gone.json"));
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        a.collections.list.push(picks);
        a.collections.save(&id);
        let carrier = a.collections.carrier.clone().unwrap();
        assert!(carrier.wait(std::time::Duration::from_secs(10)));
        assert!(carry::read_folder(&slow).is_empty(), "still out");
        // The share answers: the write goes on, after its time.
        std::fs::write(&fifo, "not a deletion").unwrap();
        let copy = carry::sets_dir(&slow).join(format!("{id}.json"));
        let start = std::time::Instant::now();
        while !copy.exists() || carrier.out_on(&slow) {
            assert!(start.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // The slow frame out of the set: its copy goes from there.
        a.collections.collection_mut(&id).unwrap().refs.truncate(1);
        a.collections.save(&id);
        assert!(carrier.wait(std::time::Duration::from_secs(10)));
        assert!(carry::read_folder(&slow).is_empty(), "taken out");
        assert_eq!(carry::read_folder(&fast).len(), 1);
        crate::testing::remove_dir_retry(&dir);
    }

    /// A copy of a set deleted here, in a folder no record of this
    /// machine lists (a late write's, or one written by a machine that
    /// had not seen the deletion): the pass that finds it buries it
    /// there, and another machine passing that folder does not take
    /// the set.
    #[test]
    fn a_copy_of_a_deleted_set_found_later_is_buried_where_it_lies() {
        let dir = crate::testing::scratch_dir("carried-found-late");
        let (f, g) = (dir.join("shoot"), dir.join("other"));
        let frames = shoot(&f, 2);
        shoot(&g, 1);
        let mut a = machine(&dir.join("a"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        a.collections.list.push(picks);
        a.collections.save(&id);
        carry::put(&g, &a.collections.item(&id).unwrap()).unwrap();
        a.collections.delete(&id);
        let names = |folder: &Path| -> Vec<String> {
            carry::read_folder(folder)
                .into_iter()
                .map(|f| f.name)
                .collect()
        };
        assert_eq!(names(&f), [format!("{id}.gone.json")]);
        assert_eq!(names(&g), [format!("{id}.json")]);
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        assert!(!a.pass(&mut lib, &[&g]));
        assert_eq!(names(&g), [format!("{id}.gone.json")]);
        let mut b = machine(&dir.join("b"));
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        b.pass(&mut lib, &[&g]);
        assert!(b.names().is_empty(), "{:?}", b.names());
        crate::testing::remove_dir_retry(&dir);
    }

    /// The part file a pass names as left over goes through the
    /// carrier; the copy beside it stays.
    #[test]
    fn the_leftovers_a_pass_names_are_cleared_by_the_writer() {
        let dir = crate::testing::scratch_dir("carried-clear");
        let f = dir.join("shoot");
        let frames = shoot(&f, 2);
        let mut a = machine(&dir.join("a"));
        let mut picks = crate::collections::Collection::new("Picks");
        picks.refs = vec![r(0, &frames[0]), r(1, &frames[1])];
        let id = picks.id.clone();
        a.collections.list.push(picks);
        a.collections.save(&id);
        let name = format!("{id}.json.9-9.part");
        let part = carry::sets_dir(&f).join(&name);
        std::fs::write(&part, "{").unwrap();
        let two_days =
            std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(&part)
            .unwrap()
            .set_modified(two_days)
            .unwrap();
        let mut lib = greycard_library::Library::open_in_memory().unwrap();
        let report = lib.index_tree(&f, &mut |_| {}).unwrap();
        assert_eq!(report.sets_stale.len(), 1, "{:?}", report.sets_stale);
        assert!(report.sets_stale[0].ends_with(&name));
        a.collections
            .carrier
            .as_ref()
            .unwrap()
            .clear(report.sets_stale);
        assert!(!part.exists());
        assert_eq!(carry::read_folder(&f).len(), 1);
        crate::testing::remove_dir_retry(&dir);
    }
}
