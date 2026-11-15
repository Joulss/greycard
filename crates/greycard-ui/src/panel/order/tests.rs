use std::collections::BTreeMap;

use i_slint_backend_testing::ElementHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};

use super::*;
use crate::testing::{click, press, remove_dir_retry, settle, window};

fn names_of(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

fn first_of(name: &str) -> &'static [&'static str] {
    view(name).unwrap().first
}

fn order_of(app: &App, name: &str) -> Vec<String> {
    order_now(app, view(name).unwrap())
}

/// The names of the model the pane of `name` shows, slot by slot.
fn model_of(app: &App, name: &str) -> Vec<String> {
    names(&(view(name).unwrap().get)(app))
}

/// A settings file of the test's own, gone again at the end.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "greycard-order-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        // remove_dir_all allowed: a last run's leftover, before anything is open.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::settings::use_file(Some(dir.join("settings.json")));
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        crate::settings::use_file(None);
        remove_dir_retry(&self.0);
    }
}

fn saved() -> BTreeMap<String, Vec<String>> {
    crate::settings::Settings::load().section_orders
}

// Every apply of `destination` to an order of `len`.
fn apply(len: usize, from: usize, slot: usize) -> Option<Vec<usize>> {
    let to = destination(len, from, slot)?;
    let mut order: Vec<usize> = (0..len).collect();
    let section = order.remove(from);
    order.insert(to, section);
    Some(order)
}

#[test]
fn a_saved_order_keeps_its_known_names_once() {
    assert_eq!(
        reconcile(&["a", "b", "c"], &names_of(&["b", "gone", "b", "a", "c"])),
        names_of(&["b", "a", "c"])
    );
    // None saved is the first order.
    assert_eq!(reconcile(&["a", "b", "c"], &[]), names_of(&["a", "b", "c"]));
}

/// A section a later version adds lands after its neighbor above it in
/// that version's first order, wherever the user dragged that one; not
/// at the foot. One whose neighbors above are all new too follows
/// them; one with none above goes first.
#[test]
fn a_new_section_lands_after_its_first_order_neighbor() {
    // A version with a section after Detail.
    let mut later: Vec<&str> = first_of("develop").to_vec();
    let at = later.iter().position(|n| *n == "detail").unwrap();
    later.insert(at + 1, "texture");
    // Saved by this version, Detail dragged up under MASK.
    let mut saved: Vec<String> = names_of(first_of("develop"));
    saved.retain(|n| n != "detail");
    saved.insert(1, "detail".into());
    let order = reconcile(&later, &saved);
    assert_eq!(order[..3], names_of(&["mask", "detail", "texture"]));
    assert_eq!(order.len(), later.len());
    assert_eq!(order.last().unwrap(), "metadata", "not at the foot");

    // Two new in a row, and one new at the very top.
    let first = ["new-top", "a", "b", "x", "y", "c"];
    let saved = names_of(&["c", "b", "a"]);
    assert_eq!(
        reconcile(&first, &saved),
        names_of(&["new-top", "c", "b", "x", "y", "a"])
    );
}

/// Every move an order of up to six can have: where the section lands
/// is just before the one it was dropped on, or last for the end, and
/// a drop onto itself or just under itself moves nothing.
#[test]
fn every_move_lands_where_it_was_dropped() {
    // Down by one, an adjacent swap: the second dropped under the third.
    assert_eq!(apply(4, 1, 3), Some(vec![0, 2, 1, 3]));
    // Up by one: the third dropped on the second.
    assert_eq!(apply(4, 2, 1), Some(vec![0, 2, 1, 3]));
    // Down by two, and up by two.
    assert_eq!(apply(5, 0, 3), Some(vec![1, 2, 0, 3, 4]));
    assert_eq!(apply(5, 4, 2), Some(vec![0, 1, 4, 2, 3]));
    // To the first place and to the last.
    assert_eq!(apply(4, 3, 0), Some(vec![3, 0, 1, 2]));
    assert_eq!(apply(4, 0, 4), Some(vec![1, 2, 3, 0]));
    assert_eq!(apply(4, 2, 4), Some(vec![0, 1, 3, 2]));
    // Onto itself, or under itself: nothing.
    assert_eq!(apply(4, 2, 2), None);
    assert_eq!(apply(4, 2, 3), None);
    assert_eq!(apply(4, 3, 4), None);
    // Out of the order.
    assert_eq!(apply(4, 4, 0), None);
    assert_eq!(apply(4, 0, 5), None);
    // Every pair: the section is before the one it was dropped on.
    for len in 1..=6 {
        for from in 0..len {
            for slot in 0..=len {
                let got = apply(len, from, slot);
                if slot == from || slot == from + 1 {
                    assert_eq!(got, None, "{len} {from} {slot}");
                    continue;
                }
                let got = got.unwrap();
                let at = got.iter().position(|&n| n == from).unwrap();
                if slot == len {
                    assert_eq!(at, len - 1, "{len} {from} {slot}");
                } else {
                    assert_eq!(got[at + 1], slot, "{len} {from} {slot}");
                }
                let mut rest: Vec<usize> = got.iter().copied().filter(|&n| n != from).collect();
                rest.sort();
                assert_eq!(rest, (0..len).filter(|&n| n != from).collect::<Vec<_>>());
            }
        }
    }
}

/// The left pane looks back six places for the list above and counts
/// seven (`left-before` and `left-shown-count` in app.slint): its
/// model stays that short. Library's and the Loupe's orders come back
/// whole from the one model, whatever either is: FOLDERS once, each
/// view's own in its order.
#[test]
fn the_left_pane_holds_both_views_in_one_model() {
    let library = names_of(first_of("library"));
    let loupe = names_of(first_of("loupe"));
    assert!(pane_order(&[library.clone(), loupe.clone()]).len() <= 7);
    let own = |model: &[String], first: &[&str]| -> Vec<String> {
        model
            .iter()
            .filter(|n| first.contains(&n.as_str()))
            .cloned()
            .collect()
    };
    let lib = ["roots", "folders", "collections", "metadata"];
    let lou = ["folders", "presets", "snapshots", "history"];
    for a in permutations(&lib) {
        for b in permutations(&lou) {
            let (a, b) = (names_of(&a), names_of(&b));
            let model = pane_order(&[a.clone(), b.clone()]);
            assert_eq!(model.len(), 7);
            assert_eq!(own(&model, &lib), a);
            assert_eq!(own(&model, &lou), b);
        }
    }
    // Every view's names are its own once.
    for v in VIEWS {
        let mut sorted = v.first.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), v.first.len(), "{}", v.name);
    }
}

/// Only a moved view is kept, the moves are written as they happen,
/// and a later start shows them; a fresh panel writes none at its
/// close. Reset puts every view back, and the file says nothing again.
#[test]
fn only_a_moved_view_is_kept_and_reset_puts_all_back() {
    let _scratch = Scratch::new("kept");
    let app = window(3);
    install(&app);
    assert!(read(&app).is_empty());
    assert!(
        crate::panel::startup::remember(&app)
            .section_orders
            .is_empty()
    );
    assert!(!app.get_section_order_moved());

    app.invoke_section_moved("crop".into(), 2, 0, 3);
    let crop = names_of(&["perspective", "crop", "rotate"]);
    assert_eq!(order_of(&app, "crop"), crop);
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert_eq!(read(&app), [("crop".to_string(), crop.clone())].into());
    assert_eq!(saved(), read(&app));
    assert!(app.get_section_order_moved());
    // A drop that moves nothing changes nothing.
    app.invoke_section_moved("crop".into(), 0, 1, 1);
    assert_eq!(order_of(&app, "crop"), crop);

    let next = window(3);
    show(&next, &saved());
    assert_eq!(order_of(&next, "crop"), crop);
    assert!(next.get_section_order_moved());

    // Moved back by hand: nothing kept.
    app.invoke_section_moved("crop".into(), 0, 3, 1);
    assert_eq!(order_of(&app, "crop"), names_of(first_of("crop")));
    assert!(saved().is_empty());
    assert!(!app.get_section_order_moved());

    app.invoke_section_moved("develop".into(), 5, 0, 6);
    app.invoke_section_moved("library".into(), 3, 0, 4);
    let file = || std::fs::read_to_string(_scratch.0.join("settings.json")).unwrap();
    assert!(file().contains("\"section_orders\""));
    reset(&app);
    for v in VIEWS {
        assert_eq!(order_of(&app, v.name), names_of(v.first), "{}", v.name);
    }
    assert!(saved().is_empty());
    assert!(!app.get_section_order_moved());
    assert!(
        !file().contains("section_orders"),
        "the key gone from the file"
    );
}

fn find_text(app: &App, label: &str) -> Option<ElementHandle> {
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(0.0, 0.0),
    });
    // A section with an owner (a mask's name) titles itself "·  MASK".
    let after_owner = format!("\u{00b7}  {label}");
    ElementHandle::find_by_element_type_name(app, "Text").find(|e| {
        e.size().height > 0.0
            && e.accessible_label()
                .is_some_and(|l| l == label || l == after_owner.as_str())
    })
}

/// A header's middle, in the window.
fn header(app: &App, label: &str) -> (f32, f32) {
    let e = find_text(app, label).unwrap_or_else(|| panic!("no {label:?}"));
    let (at, size) = (e.absolute_position(), e.size());
    (at.x + size.width / 2.0, at.y + size.height / 2.0)
}

fn pointer(app: &App, event: &str, (x, y): (f32, f32)) {
    let position = LogicalPosition::new(x, y);
    let button = PointerEventButton::Left;
    app.window().dispatch_event(match event {
        "down" => WindowEvent::PointerPressed { position, button },
        "up" => WindowEvent::PointerReleased { position, button },
        _ => WindowEvent::PointerMoved { position },
    });
}

/// Press at `from`, move through `path`, release at its last point,
/// each step a turn of the event loop apart.
fn drag_through(app: &App, from: (f32, f32), path: &[(f32, f32)]) {
    pointer(app, "move", from);
    pointer(app, "down", from);
    for &p in path {
        pointer(app, "move", p);
        slint::platform::update_timers_and_animations();
    }
    pointer(app, "up", *path.last().unwrap());
    settle(app);
}

/// The Develop tab with every section folded, in a window tall enough
/// for all of them.
fn develop(height: f32) -> App {
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, height));
    install(&app);
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, true);
    }
    settle(&app);
    app
}

/// A header dragged moves its section where it is dropped, down past
/// another or up above one, and leaves every fold as it was; the move
/// is written as it happens.
#[test]
fn a_header_dragged_down_or_up_moves_its_section() {
    let _scratch = Scratch::new("drag");
    let app = develop(1400.0);
    let look = header(&app, "LOOK");
    let camera = header(&app, "CAMERA PROFILE");
    let light = header(&app, "LIGHT");
    // Between CAMERA and LIGHT.
    let between = (look.0, (camera.1 + light.1) / 2.0);
    drag_through(&app, look, &[(look.0, look.1 + 12.0), between]);
    let mut want = names_of(first_of("develop"));
    let at = want.iter().position(|n| n == "look").unwrap();
    want.remove(at);
    let camera_at = want.iter().position(|n| n == "camera").unwrap();
    want.insert(camera_at + 1, "look".into());
    assert_eq!(order_of(&app, "develop"), want, "down past CAMERA");
    assert!(app.get_collapsed_look() && app.get_collapsed_camera());
    assert_eq!(saved().get("develop"), Some(&want));
    // On screen too: LOOK now between CAMERA and LIGHT.
    let (look, camera, light) = (
        header(&app, "LOOK"),
        header(&app, "CAMERA PROFILE"),
        header(&app, "LIGHT"),
    );
    assert!(camera.1 < look.1 && look.1 < light.1);

    // LIGHT up above the first section shown.
    let wb = header(&app, "WHITE BALANCE");
    let light = header(&app, "LIGHT");
    drag_through(
        &app,
        light,
        &[(light.0, light.1 - 12.0), (light.0, wb.1 - 10.0)],
    );
    let shown: Vec<String> = order_of(&app, "develop")
        .into_iter()
        .filter(|n| n != "mask")
        .collect();
    assert_eq!(shown[..3], names_of(&["light", "white-balance", "camera"]));
    assert!(app.get_collapsed_light(), "its fold as it was");
    assert_eq!(saved().get("develop"), Some(&order_of(&app, "develop")));
}

/// A press and release that wobbles stays a click and folds: under
/// 8 px it is never a drag, and past it a release back over the header
/// that moves nothing is still the click.
#[test]
fn a_wobbling_click_still_folds() {
    let _scratch = Scratch::new("wobble");
    let app = develop(1400.0);
    let look = header(&app, "LOOK");
    drag_through(&app, look, &[(look.0 + 5.0, look.1 + 3.0)]);
    assert!(!app.get_collapsed_look(), "a small wobble opens it");
    settle(&app);
    let look = header(&app, "LOOK");
    drag_through(
        &app,
        look,
        &[(look.0, look.1 + 12.0), (look.0, look.1 + 2.0)],
    );
    assert!(
        app.get_collapsed_look(),
        "back over itself, the click folds"
    );
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(saved().is_empty());
    // A drag that moved does not fold: released over the header it
    // came from, now in its new place.
    let look = header(&app, "LOOK");
    let wb = header(&app, "WHITE BALANCE");
    let camera = header(&app, "CAMERA PROFILE");
    drag_through(
        &app,
        look,
        &[(look.0, look.1 + 12.0), (look.0, (wb.1 + camera.1) / 2.0)],
    );
    assert_ne!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(app.get_collapsed_look(), "a move is no click");
}

/// Esc with a header in hand puts it back: nothing moves, the release
/// does not fold it, and the next Esc is the window's again.
#[test]
fn esc_cancels_a_drag() {
    let _scratch = Scratch::new("esc");
    let app = develop(1400.0);
    let look = header(&app, "LOOK");
    let light = header(&app, "LIGHT");
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 + 12.0));
    pointer(&app, "move", (look.0, light.1 + 10.0));
    settle(&app);
    press(&app, slint::platform::Key::Escape);
    pointer(&app, "move", (look.0, light.1 + 12.0));
    pointer(&app, "move", look);
    pointer(&app, "up", look);
    settle(&app);
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(app.get_collapsed_look(), "no fold after a cancel");
    assert!(saved().is_empty());
    app.set_wheel_hand(2);
    press(&app, slint::platform::Key::Escape);
    assert_eq!(app.get_wheel_hand(), -1, "Esc the window's again");
}

/// Off the pane sideways a drop is nowhere; past the pane's foot the
/// pane scrolls on to its end and the section goes last, and past its
/// top back up again.
#[test]
fn a_drag_past_the_panes_ends_scrolls_it_and_off_its_side_drops_nothing() {
    let _scratch = Scratch::new("ends");
    let app = develop(640.0);
    let view = ElementHandle::find_by_element_id(&app, "App::panel-view")
        .next()
        .unwrap();
    let (at, size) = (view.absolute_position(), view.size());
    let look = header(&app, "LOOK");
    let light = header(&app, "LIGHT");
    drag_through(
        &app,
        look,
        &[(look.0, look.1 + 12.0), (at.x - 60.0, light.1)],
    );
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(app.get_collapsed_look());

    // Past the foot, held there.
    let look = header(&app, "LOOK");
    let foot = at.y + size.height + 40.0;
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 + 12.0));
    pointer(&app, "move", (look.0, foot));
    assert_eq!(app.get_panel_scroll(), 0.0);
    for _ in 0..200 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(40));
        slint::platform::update_timers_and_animations();
    }
    let scrolled = app.get_panel_scroll();
    assert!(scrolled < 0.0, "scrolled down");
    pointer(&app, "move", (look.0, foot + 1.0));
    pointer(&app, "up", (look.0, foot + 1.0));
    settle(&app);
    assert_eq!(order_of(&app, "develop").last().unwrap(), "look");

    // Past the top: back up to the first place.
    let look = header(&app, "LOOK");
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 - 12.0));
    pointer(&app, "move", (look.0, at.y - 40.0));
    for _ in 0..200 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(40));
        slint::platform::update_timers_and_animations();
    }
    assert_eq!(app.get_panel_scroll(), 0.0, "scrolled back to the top");
    pointer(&app, "up", (look.0, at.y - 40.0));
    settle(&app);
    let shown: Vec<String> = order_of(&app, "develop")
        .into_iter()
        .filter(|n| n != "mask")
        .collect();
    assert_eq!(shown[0], "look");
}

/// A stack taken away under a drag (another tab, by a key) moves
/// nothing at the release, and Esc is the window's again.
#[test]
fn a_stack_taken_away_mid_drag_commits_nothing() {
    let _scratch = Scratch::new("torn");
    let app = develop(1400.0);
    let look = header(&app, "LOOK");
    let light = header(&app, "LIGHT");
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 + 12.0));
    pointer(&app, "move", (look.0, light.1 + 10.0));
    settle(&app);
    app.set_panel_tab("Crop".into());
    settle(&app);
    pointer(&app, "move", (look.0, light.1 + 30.0));
    pointer(&app, "up", (look.0, light.1 + 30.0));
    settle(&app);
    for v in VIEWS {
        assert_eq!(order_of(&app, v.name), names_of(v.first), "{}", v.name);
    }
    assert!(saved().is_empty());
    app.set_wheel_hand(2);
    press(&app, slint::platform::Key::Escape);
    assert_eq!(app.get_wheel_hand(), -1, "Esc the window's again");
}

/// The slots of the stack `id`, in the tree's order.
fn slots(app: &App, id: &str) -> Vec<ElementHandle> {
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(0.0, 0.0),
    });
    let stack = ElementHandle::find_by_element_id(app, &format!("App::{id}"))
        .next()
        .unwrap_or_else(|| panic!("no {id}"));
    stack
        .query_descendants()
        .match_type_name("SectionSlot")
        .find_all()
}

fn sections_in(slot: &ElementHandle) -> usize {
    slot.query_descendants()
        .match_inherits("Section")
        .find_all()
        .len()
}

/// A drop takes one row out of the order and puts it back in: the
/// stack's other sections are not made again, so they keep what they
/// hold. Every slot but the moved one is the one it was. Their order in
/// the tree, which the Tab key walks, is their order on screen.
#[test]
fn a_drop_remakes_only_the_section_it_moves() {
    let _scratch = Scratch::new("remake");
    let app = develop(1400.0);
    let before = slots(&app, "develop-stack");
    assert_eq!(before.len(), first_of("develop").len());
    assert!(before.iter().all(ElementHandle::is_valid));
    app.invoke_section_moved("develop".into(), 6, 1, 7);
    settle(&app);
    let kept = before.iter().filter(|s| s.is_valid()).count();
    assert_eq!(kept, before.len() - 1, "only the moved one is new");
    // Down as up: the one moved, and no section it passes.
    let before = slots(&app, "develop-stack");
    app.invoke_section_moved("develop".into(), 1, 9, 2);
    settle(&app);
    let kept = before.iter().filter(|s| s.is_valid()).count();
    assert_eq!(kept, before.len() - 1, "a move down remakes one");

    let after = slots(&app, "develop-stack");
    let ys: Vec<f32> = after
        .iter()
        .filter(|s| s.size().height > 0.0)
        .map(|s| s.absolute_position().y)
        .collect();
    assert!(
        ys.windows(2).all(|w| w[0] < w[1]),
        "tree order is screen order: {ys:?}"
    );

    // The whole model replaced, as a reset does: every one made anew.
    reset(&app);
    settle(&app);
    assert!(after.iter().all(|s| !s.is_valid()));
}

/// Develop and the Masks tab share one order, and the six sections a
/// mask edits too are one instance for both: a switch between the tabs
/// keeps them, with their state, and makes only the others.
#[test]
fn develop_and_masks_share_one_order_and_their_six_sections() {
    let _scratch = Scratch::new("shared");
    let app = develop(1400.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    app.set_panel_tab("Masks".into());
    app.invoke_new_mask();
    app.invoke_target_changed(1);
    app.set_panel_tab("Develop".into());
    settle(&app);
    let named = |app: &App| -> Vec<(String, ElementHandle)> {
        slots(app, "develop-stack")
            .into_iter()
            .zip(order_of(app, "develop"))
            .filter(|(s, _)| sections_in(s) == 1)
            .map(|(s, n)| {
                let section = s
                    .query_descendants()
                    .match_inherits("Section")
                    .find_first()
                    .unwrap();
                (n, section)
            })
            .collect()
    };
    let on_develop = named(&app);
    app.set_panel_tab("Masks".into());
    settle(&app);
    let kept: Vec<&str> = on_develop
        .iter()
        .filter(|(_, s)| s.is_valid())
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(
        kept,
        ["light", "color", "curves", "mixer", "grading", "tint"],
        "the six shared sections are the same ones"
    );

    // A move on one tab is the other's too.
    app.invoke_section_moved("develop".into(), 10, 0, 11);
    let order = order_of(&app, "develop");
    assert_eq!(order[0], "tint");
    let on_masks: Vec<String> = named(&app).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        on_masks,
        [
            "tint",
            "mask",
            "white-balance",
            "light",
            "color",
            "curves",
            "mixer",
            "grading",
            "detail"
        ]
    );
}

/// Every name in every view's order holds exactly one section on the
/// tab or pane that shows it, and nothing else does: a name the panel
/// spells otherwise would leave its section out, silently.
#[test]
fn every_name_in_every_order_is_a_section() {
    let app = develop(1400.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    let check = |app: &App, id: &str, view: &str, want: &[&str]| {
        settle(app);
        let held: Vec<String> = slots(app, id)
            .iter()
            .zip(model_of(app, view))
            .map(|(s, n)| (sections_in(s), n))
            .inspect(|(count, n)| assert!(*count <= 1, "{view}: {n} holds {count}"))
            .filter(|(count, _)| *count == 1)
            .map(|(_, n)| n)
            .collect();
        assert_eq!(held, names_of(want), "{view} in {id}");
    };
    // Develop shows all but MASK; the Masks tab its own nine.
    let develop: Vec<&str> = first_of("develop")
        .iter()
        .copied()
        .filter(|n| *n != "mask")
        .collect();
    check(&app, "develop-stack", "develop", &develop);
    app.set_panel_tab("Masks".into());
    app.invoke_new_mask();
    app.invoke_target_changed(1);
    check(
        &app,
        "develop-stack",
        "develop",
        &[
            "mask",
            "white-balance",
            "light",
            "color",
            "curves",
            "mixer",
            "grading",
            "tint",
            "detail",
        ],
    );
    for (tab, id, view) in [
        ("Cull", "cull-stack", "cull"),
        ("Crop", "crop-stack", "crop"),
        ("Retouch", "retouch-stack", "retouch"),
    ] {
        app.set_panel_tab(tab.into());
        check(&app, id, view, first_of(view));
    }
    app.set_folder_tree_shown(true);
    check(&app, "left-stack", "loupe", first_of("loupe"));
    app.set_grid_open(true);
    check(&app, "left-stack", "library", first_of("library"));
}

/// The left pane in Library order `order`, the folder tree shown or not.
fn library(app: &App, order: &[&str], tree: bool) {
    show(app, &[("library".to_string(), names_of(order))].into());
    app.set_grid_open(true);
    app.set_folder_tree_shown(tree);
    settle(app);
}

fn permutations(list: &[&'static str]) -> Vec<Vec<&'static str>> {
    if list.len() <= 1 {
        return vec![list.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..list.len() {
        let mut rest = list.to_vec();
        let head = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, head);
            out.push(p);
        }
    }
    out
}

/// The Export queue's row is under the collections in every Library
/// order, the folder tree shown or not: once, and with no other list
/// between them.
#[test]
fn the_export_queue_row_follows_the_collections() {
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    let titles = [
        ("roots", "ROOTS"),
        ("folders", "FOLDERS"),
        ("collections", "COLLECTIONS"),
        ("metadata", "METADATA"),
    ];
    for order in permutations(first_of("library")) {
        for tree in [false, true] {
            library(&app, &order, tree);
            let queue: Vec<_> = ElementHandle::find_by_element_type_name(&app, "RootRow")
                .filter(|e| {
                    e.size().height > 0.0
                        && e.query_descendants()
                            .match_predicate(|t| {
                                t.accessible_label().as_deref() == Some("Export queue")
                            })
                            .find_first()
                            .is_some()
                })
                .collect();
            assert_eq!(queue.len(), 1, "{order:?} tree {tree}");
            let queue = queue[0].absolute_position().y;
            let collections = header(&app, "COLLECTIONS").1;
            assert!(collections < queue, "{order:?} tree {tree}");
            for (name, title) in titles {
                if name == "collections" || (name == "folders" && !tree) {
                    continue;
                }
                let y = header(&app, title).1;
                assert!(
                    !(collections < y && y < queue),
                    "{title} between COLLECTIONS and the queue: {order:?} tree {tree}"
                );
            }
        }
    }
}

/// A list the view hides takes no room wherever the order puts it: the
/// lists on show stand exactly where they stand with it last. Library
/// with no folder tree, and the Loupe in culling, where only the
/// folders show.
#[test]
fn a_hidden_list_takes_no_room() {
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    let at = |app: &App| -> Vec<f32> {
        ["ROOTS", "COLLECTIONS", "METADATA"]
            .iter()
            .map(|t| header(app, t).1)
            .collect()
    };
    library(
        &app,
        &["roots", "collections", "metadata", "folders"],
        false,
    );
    let last = at(&app);
    for order in [
        ["folders", "roots", "collections", "metadata"],
        ["roots", "folders", "collections", "metadata"],
        ["roots", "collections", "folders", "metadata"],
    ] {
        library(&app, &order, false);
        assert_eq!(at(&app), last, "{order:?}");
    }

    // The Loupe in culling: the folders alone, where they would be first.
    app.set_grid_open(false);
    app.set_folder_tree_shown(true);
    app.set_culling(true);
    show(&app, &BTreeMap::new());
    settle(&app);
    let first = header(&app, "FOLDERS").1;
    show(
        &app,
        &[(
            "loupe".to_string(),
            names_of(&["presets", "snapshots", "history", "folders"]),
        )]
        .into(),
    );
    settle(&app);
    assert_eq!(header(&app, "FOLDERS").1, first);
}

/// New mask brings the MASK section into view wherever the order has
/// put it: at the foot of a long panel it is scrolled to, so the name
/// taking the keys is on screen; first, the panel stays at its top.
#[test]
fn new_mask_brings_its_section_into_view() {
    let _scratch = Scratch::new("mask");
    let app = develop(1000.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    // MASK last, under everything.
    let len = first_of("develop").len();
    app.invoke_section_moved("develop".into(), 0, len as i32, 1);
    app.set_panel_tab("Masks".into());
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, false);
    }
    settle(&app);
    let new_mask = header(&app, "New mask");
    click(&app, new_mask.0, new_mask.1);
    settle(&app);
    assert_eq!(app.get_target(), 1);
    // The view as it is now, under the list grown by the new row.
    let view = ElementHandle::find_by_element_id(&app, "App::panel-view")
        .next()
        .unwrap();
    let (top, bottom) = (
        view.absolute_position().y,
        view.absolute_position().y + view.size().height,
    );
    let mask = header(&app, "MASK");
    assert!(
        top <= mask.1 && mask.1 <= bottom,
        "MASK at {} in {top}..{bottom}",
        mask.1
    );
    assert!(app.get_panel_scroll() < 0.0);

    // First in the order: the panel at its top, as ever.
    reset(&app);
    app.set_panel_scroll(-200.0);
    settle(&app);
    let new_mask = header(&app, "New mask");
    click(&app, new_mask.0, new_mask.1);
    settle(&app);
    assert_eq!(app.get_panel_scroll(), 0.0);
}

/// The Settings sheet's Reset section order: there only to be pressed
/// once some view is moved, and pressed, every view is back.
#[test]
fn reset_section_order_in_the_settings() {
    let _scratch = Scratch::new("reset");
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    app.set_settings_category("Interface".into());
    app.set_settings_open(true);
    settle(&app);
    let button = || {
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(0.0, 0.0),
        });
        ElementHandle::find_by_element_type_name(&app, "PanelButton")
            .find(|b| {
                b.query_descendants()
                    .match_predicate(|e| {
                        e.accessible_label().as_deref() == Some("Reset section order")
                    })
                    .find_first()
                    .is_some()
            })
            .expect("the button")
    };
    assert_eq!(button().accessible_enabled(), Some(false));
    app.invoke_section_moved("crop".into(), 2, 0, 3);
    app.invoke_section_moved("loupe".into(), 3, 0, 4);
    settle(&app);
    assert_eq!(button().accessible_enabled(), Some(true));
    let (at, size) = (button().absolute_position(), button().size());
    click(&app, at.x + size.width / 2.0, at.y + size.height / 2.0);
    settle(&app);
    for v in VIEWS {
        assert_eq!(order_of(&app, v.name), names_of(v.first), "{}", v.name);
    }
    assert!(saved().is_empty());
    assert_eq!(button().accessible_enabled(), Some(false));
}

fn element(app: &App, kind: &str) -> Option<ElementHandle> {
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(0.0, 0.0),
    });
    ElementHandle::find_by_element_type_name(app, kind).next()
}

/// The folder tree's first row in sight, and where it is under the
/// FOLDERS header.
fn first_row(app: &App) -> (String, f32) {
    let top = header(app, "FOLDERS").1;
    (0..60)
        .map(|i| format!("r{i:02}"))
        .find_map(|name| {
            let y = ElementHandle::find_by_accessible_label(app, &name)
                .find(|e| e.size().height > 0.0 && e.absolute_position().y > top)
                .map(|e| e.absolute_position().y - top);
            y.map(|y| (name, y))
        })
        .expect("a row in sight")
}

/// A switch between Library and the Loupe, into culling and out, or of
/// the folder tree, remakes no list that stays on show: FOLDERS is one
/// instance across them all, its tree scrolled where it was, and the
/// other lists keep theirs while the tree comes and goes.
#[test]
fn a_switch_of_the_left_panes_view_remakes_no_list_it_keeps() {
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    let rows: Vec<crate::FolderRow> = (0..60)
        .map(|i| crate::FolderRow {
            name: format!("r{i:02}").into(),
            path: format!("/r{i:02}").into(),
            depth: 1,
            count: 1,
            ..Default::default()
        })
        .collect();
    app.set_folder_tree(slint::ModelRc::new(slint::VecModel::from(rows)));
    app.set_grid_open(true);
    app.set_folder_tree_shown(true);
    settle(&app);
    let folders = element(&app, "FoldersSection").expect("FOLDERS");
    let pane = ElementHandle::find_by_element_id(&app, "FoldersSection::folder-pane")
        .next()
        .expect("the tree's pane");
    assert_eq!(first_row(&app).0, "r00");
    pane.scroll(0.0, -90.0);
    settle(&app);
    let scrolled = first_row(&app);
    assert_ne!(scrolled.0, "r00", "the tree scrolled");
    let keeps = |app: &App, what: &str| {
        settle(app);
        assert!(folders.is_valid(), "FOLDERS remade: {what}");
        let at = first_row(app);
        assert!(
            at.0 == scrolled.0 && (at.1 - scrolled.1).abs() < 0.5,
            "the tree's scroll lost: {what}: {at:?} {scrolled:?}"
        );
    };
    app.set_grid_open(false);
    keeps(&app, "to the Loupe");
    app.set_culling(true);
    keeps(&app, "into culling");
    app.set_culling(false);
    keeps(&app, "out of culling");
    app.set_grid_open(true);
    keeps(&app, "back to Library");

    // The tree going and coming: Library's other lists stay made...
    let others: Vec<ElementHandle> = ["RootsSection", "CollectionsSection", "MetaSection"]
        .iter()
        .map(|k| element(&app, k).unwrap_or_else(|| panic!("{k}")))
        .collect();
    app.set_folder_tree_shown(false);
    settle(&app);
    app.set_folder_tree_shown(true);
    settle(&app);
    assert!(
        others.iter().all(ElementHandle::is_valid),
        "Library's lists remade"
    );
    // ...and so do the Loupe's.
    app.set_grid_open(false);
    settle(&app);
    let others: Vec<ElementHandle> = ["PresetsSection", "SnapshotsSection", "HistorySection"]
        .iter()
        .map(|k| element(&app, k).unwrap_or_else(|| panic!("{k}")))
        .collect();
    app.set_folder_tree_shown(false);
    settle(&app);
    app.set_folder_tree_shown(true);
    settle(&app);
    assert!(
        others.iter().all(ElementHandle::is_valid),
        "the Loupe's lists remade"
    );
}

/// Another button pressed in the middle of a drag puts the section back
/// and ends the drag: nothing moves at the left's release, the release
/// over the header does not fold, the scrolling stops, and Esc is the
/// window's again.
#[test]
fn another_button_mid_drag_ends_it() {
    let _scratch = Scratch::new("buttons");
    let app = develop(640.0);
    let view = ElementHandle::find_by_element_id(&app, "App::panel-view")
        .next()
        .unwrap();
    let foot = view.absolute_position().y + view.size().height + 20.0;
    let look = header(&app, "LOOK");
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 + 12.0));
    pointer(&app, "move", (look.0, foot));
    settle(&app);
    let right = PointerEventButton::Right;
    let at = LogicalPosition::new(look.0, foot);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: right,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: right,
    });
    settle(&app);
    let scroll = app.get_panel_scroll();
    for _ in 0..25 {
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(40));
        slint::platform::update_timers_and_animations();
    }
    assert_eq!(app.get_panel_scroll(), scroll, "the scrolling stopped");
    // Back over the header and let go.
    app.set_panel_scroll(0.0);
    settle(&app);
    let look = header(&app, "LOOK");
    pointer(&app, "move", look);
    pointer(&app, "up", look);
    settle(&app);
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(app.get_collapsed_look(), "no fold");
    assert!(saved().is_empty());
    app.set_wheel_hand(2);
    press(&app, slint::platform::Key::Escape);
    assert_eq!(app.get_wheel_hand(), -1, "Esc the window's again");
    // The next click is a click.
    let look = header(&app, "LOOK");
    click(&app, look.0, look.1);
    settle(&app);
    assert!(!app.get_collapsed_look());
}

/// A change of what the left pane shows under a drag that leaves its
/// stack there (the folder tree coming in) cancels the drag: the release
/// moves nothing, and Esc is the window's.
#[test]
fn a_view_change_under_a_drag_cancels_it() {
    let _scratch = Scratch::new("viewchange");
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    app.set_grid_open(true);
    settle(&app);
    let roots = header(&app, "ROOTS");
    let metadata = header(&app, "METADATA");
    pointer(&app, "move", roots);
    pointer(&app, "down", roots);
    pointer(&app, "move", (roots.0, roots.1 + 12.0));
    pointer(&app, "move", (roots.0, metadata.1 + 20.0));
    settle(&app);
    app.set_folder_tree_shown(true);
    settle(&app);
    pointer(&app, "move", (roots.0, metadata.1 + 22.0));
    pointer(&app, "up", (roots.0, metadata.1 + 22.0));
    settle(&app);
    assert_eq!(order_of(&app, "library"), names_of(first_of("library")));
    assert!(saved().is_empty());
    app.set_wheel_hand(2);
    press(&app, slint::platform::Key::Escape);
    assert_eq!(app.get_wheel_hand(), -1, "Esc the window's");
}

/// New mask at the most masks makes none, and leaves nothing waiting:
/// a mask chosen later does not scroll the panel to MASK.
#[test]
fn new_mask_at_the_most_masks_leaves_no_reveal_waiting() {
    use slint::Model;
    let _scratch = Scratch::new("maxmasks");
    let app = develop(800.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    let len = first_of("develop").len();
    app.invoke_section_moved("develop".into(), 0, len as i32, 1);
    app.set_panel_tab("Masks".into());
    for _ in 0..greycard_edit::MAX_MASKS {
        app.invoke_new_mask();
    }
    app.invoke_target_changed(0);
    // Every section open: MASK, last, is far past the view's foot.
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, false);
    }
    settle(&app);
    let made = app.get_adjustment_names().row_count();
    // M, as the button: the list of sixteen may hide the button. M
    // wants a frame on show.
    app.set_selected(0);
    press(&app, "m");
    settle(&app);
    assert_eq!(app.get_adjustment_names().row_count(), made, "none made");
    assert_eq!(app.get_target(), 0);
    app.set_panel_scroll(0.0);
    app.invoke_target_changed(3);
    settle(&app);
    assert!(app.get_target() > 0);
    assert_eq!(app.get_panel_scroll(), 0.0, "no reveal waiting");
}

/// With no list on show (culling with no tree) there is no stack, so no
/// gap is left for an empty one at the pane's foot.
#[test]
fn no_list_on_show_leaves_no_stack() {
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    app.set_culling(true);
    settle(&app);
    assert!(
        ElementHandle::find_by_element_id(&app, "App::left-stack")
            .next()
            .is_none(),
        "no stack with nothing in it"
    );
    app.set_folder_tree_shown(true);
    settle(&app);
    assert!(element(&app, "FoldersSection").is_some());
}

/// No slot is on show under the dragged one: what a call from the Rust
/// side passes when it means the model's own places alone.
const NO_NEXT: usize = usize::MAX;

/// Every drop on Develop's stack, up and down, from each place to each:
/// only the section moved is made anew, and every one it passes keeps
/// its instance. MASK from first to last remakes MASK alone.
#[test]
fn every_drop_on_develop_remakes_only_the_one_moved() {
    let _scratch = Scratch::new("everydevelop");
    let app = develop(1400.0);
    let len = first_of("develop").len();
    for from in 0..len {
        for slot in 0..=len {
            let before = slots(&app, "develop-stack");
            assert_eq!(before.len(), len);
            let moves = moved(&app, "develop", from, slot, NO_NEXT);
            assert_eq!(moves, destination(len, from, slot).is_some());
            settle(&app);
            let kept = before.iter().filter(|s| s.is_valid()).count();
            let least = if moves { len - 1 } else { len };
            assert_eq!(kept, least, "{from} to {slot}");
        }
    }
}

/// Every drop in Library's and the Loupe's view of the left pane keeps
/// every row the move need not touch (the longest run in the same order
/// before and after), and FOLDERS, unless it is the one dragged, keeps
/// its instance and its tree's scroll.
#[test]
fn every_drop_on_the_left_pane_keeps_folders_and_its_scroll() {
    let _scratch = Scratch::new("everyleft");
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    let rows: Vec<crate::FolderRow> = (0..60)
        .map(|i| crate::FolderRow {
            name: format!("r{i:02}").into(),
            path: format!("/r{i:02}").into(),
            depth: 1,
            count: 1,
            ..Default::default()
        })
        .collect();
    app.set_folder_tree(slint::ModelRc::new(slint::VecModel::from(rows)));
    app.set_folder_tree_shown(true);
    for (grid, name) in [(true, "library"), (false, "loupe")] {
        app.set_grid_open(grid);
        settle(&app);
        let len = model_of(&app, name).len();
        for from in 0..len {
            for slot in 0..=len {
                let model = model_of(&app, name);
                let own = |n: &String| first_of(name).contains(&n.as_str());
                if !own(&model[from]) || (slot < len && !own(&model[slot])) {
                    continue;
                }
                let dragged = model[from].clone();
                // The tree scrolled a little, to see that it stays so.
                if dragged != "folders" {
                    let pane =
                        ElementHandle::find_by_element_id(&app, "FoldersSection::folder-pane")
                            .next()
                            .expect("the tree's pane");
                    pane.scroll(0.0, -60.0);
                    settle(&app);
                }
                let folders = element(&app, "FoldersSection").expect("FOLDERS");
                let scrolled = first_row(&app);
                let before = slots(&app, "left-stack");
                assert_eq!(before.len(), len);
                moved(&app, name, from, slot, NO_NEXT);
                settle(&app);
                let after = model_of(&app, name);
                let least = kept_rows(&model, &after).len();
                let kept = before.iter().filter(|s| s.is_valid()).count();
                assert_eq!(kept, least, "{name} {from} to {slot}: {model:?} {after:?}");
                if dragged != "folders" {
                    assert!(
                        folders.is_valid(),
                        "{name} {from} to {slot}: FOLDERS remade"
                    );
                    assert_eq!(
                        first_row(&app),
                        scrolled,
                        "{name} {from} to {slot}: the scroll"
                    );
                }
                reset(&app);
                settle(&app);
            }
        }
    }
}

/// A drop just under itself on screen moves nothing, though hidden
/// slots lie between: the Rust side refuses it, and a wobble back over
/// the header there is the click it was, in Library with no tree (FOLDERS
/// hidden under ROOTS) and on the Masks tab (CAMERA PROFILE, Develop's,
/// hidden under WHITE BALANCE).
#[test]
fn a_drop_just_under_itself_past_hidden_slots_moves_nothing() {
    let _scratch = Scratch::new("hiddennext");
    let app = window(3);
    app.window()
        .set_size(slint::LogicalSize::new(1500.0, 1000.0));
    install(&app);
    // ROOTS 0, FOLDERS 1 hidden, COLLECTIONS 2: a drop on COLLECTIONS.
    assert!(!moved(&app, "library", 0, 2, 2));
    assert!(saved().is_empty());

    app.set_grid_open(true);
    settle(&app);
    let wobble = |app: &App, title: &str| {
        let at = header(app, title);
        pointer(app, "move", at);
        pointer(app, "down", at);
        pointer(app, "move", (at.0, at.1 + 12.0));
        slint::platform::update_timers_and_animations();
        for dy in [6.0, 7.0, 8.0, 9.0] {
            pointer(app, "move", (at.0, at.1 + dy));
            slint::platform::update_timers_and_animations();
        }
        pointer(app, "up", (at.0, at.1 + 9.0));
        settle(app);
    };
    let folded = app.get_collapsed_roots();
    wobble(&app, "ROOTS");
    assert_eq!(order_of(&app, "library"), names_of(first_of("library")));
    assert!(saved().is_empty(), "nothing written");
    assert!(!app.get_section_order_moved());
    assert_ne!(app.get_collapsed_roots(), folded, "the click folds");

    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    app.set_grid_open(false);
    app.set_panel_tab("Masks".into());
    app.invoke_new_mask();
    app.invoke_target_changed(1);
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, true);
    }
    settle(&app);
    let folded = app.get_collapsed_wb();
    wobble(&app, "WHITE BALANCE");
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(saved().is_empty(), "nothing written");
    assert_ne!(app.get_collapsed_wb(), folded, "the click folds");
}

/// A cancel that leaves the header in hand on screen (Esc, held with no
/// release yet) lets the drag go at once: no line, no scrolling, and the
/// next drag, of another section, moves just that one.
#[test]
fn a_cancel_lets_the_drag_go_without_a_release() {
    let _scratch = Scratch::new("cancelheld");
    let app = develop(1400.0);
    let look = header(&app, "LOOK");
    let light = header(&app, "LIGHT");
    pointer(&app, "move", look);
    pointer(&app, "down", look);
    pointer(&app, "move", (look.0, look.1 + 12.0));
    pointer(&app, "move", (look.0, light.1 + 10.0));
    settle(&app);
    press(&app, slint::platform::Key::Escape);
    settle(&app);
    // Released elsewhere, then another section dragged.
    pointer(&app, "up", (look.0, light.1 + 10.0));
    settle(&app);
    let wb = header(&app, "WHITE BALANCE");
    let camera = header(&app, "CAMERA PROFILE");
    let light = header(&app, "LIGHT");
    drag_through(
        &app,
        wb,
        &[(wb.0, wb.1 + 12.0), (wb.0, (camera.1 + light.1) / 2.0)],
    );
    let mut want = names_of(first_of("develop"));
    let at = want.iter().position(|n| n == "white-balance").unwrap();
    want.remove(at);
    let camera_at = want.iter().position(|n| n == "camera").unwrap();
    want.insert(camera_at + 1, "white-balance".into());
    assert_eq!(order_of(&app, "develop"), want);
    assert!(app.get_collapsed_look(), "LOOK never folded");
}

/// A frozen frame shows none of the develop tabs, so New mask by M
/// makes nothing there, and nothing is left to scroll to MASK when the
/// frame thaws: the reveal is only ever for a mask just made on show.
#[test]
fn new_mask_on_a_frozen_frame_leaves_no_reveal_waiting() {
    let _scratch = Scratch::new("frozenmask");
    let app = develop(800.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    let len = first_of("develop").len();
    app.invoke_section_moved("develop".into(), 0, len as i32, 1);
    app.set_panel_tab("Masks".into());
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, false);
    }
    app.set_selected(0);
    app.set_frozen(true);
    settle(&app);
    press(&app, "m");
    settle(&app);
    assert_eq!(app.get_target(), 0, "no mask made");
    app.set_frozen(false);
    app.invoke_new_mask();
    app.set_panel_scroll(0.0);
    app.invoke_target_changed(1);
    settle(&app);
    assert!(app.get_target() > 0);
    assert_eq!(app.get_panel_scroll(), 0.0, "no reveal waiting");
}

/// The window's own left release, as the side buttons' hook sees it
/// before Slint does (`panel::side_buttons`).
fn hook_release(app: &App) {
    use slint::winit_030::winit::event::{DeviceId, ElementState, MouseButton};
    let mut hook = crate::panel::side_buttons::SideButtons::default();
    hook.event(
        app,
        &slint::winit_030::winit::event::WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    );
}

/// LOOK in hand, dragged under LIGHT.
fn hold_look(app: &App) -> (f32, f32) {
    let look = header(app, "LOOK");
    let light = header(app, "LIGHT");
    pointer(app, "move", look);
    pointer(app, "down", look);
    pointer(app, "move", (look.0, look.1 + 12.0));
    pointer(app, "move", (look.0, light.1 + 10.0));
    // A turn of the loop, not `settle`, whose move to the corner would
    // take the drop off the pane.
    slint::platform::update_timers_and_animations();
    (look.0, light.1 + 10.0)
}

/// The release, the window's hook first as in the window, and Esc then
/// the window's: the wheel in hand put down.
fn release_and_esc(app: &App, at: (f32, f32)) {
    hook_release(app);
    pointer(app, "up", at);
    settle(app);
    app.set_wheel_hand(2);
    press(app, slint::platform::Key::Escape);
    assert_eq!(app.get_wheel_hand(), -1, "Esc the window's again");
}

/// A pane put away (F8, F7, Tab) or the lightbox opened under a drag
/// cancels it: the release moves nothing, and Esc is the window's.
#[test]
fn a_pane_put_away_or_the_lightbox_under_a_drag_cancels_it() {
    let _scratch = Scratch::new("panesaway");
    for hide in [
        App::set_right_hidden as fn(&App, bool),
        App::set_left_hidden,
        App::set_lightbox,
    ] {
        let app = develop(1400.0);
        let at = hold_look(&app);
        hide(&app, true);
        settle(&app);
        release_and_esc(&app, at);
        hide(&app, false);
        settle(&app);
        assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
        assert!(saved().is_empty());
    }
}

/// MASK in hand when its mask is deleted (the section goes, its stack
/// stays): the window's release ends the drag, nothing moves, and Esc
/// is the window's.
#[test]
fn a_mask_deleted_under_its_drag_leaves_esc_the_windows() {
    let _scratch = Scratch::new("maskgone");
    let app = develop(1400.0);
    let (_state, _worker) = crate::testing::state_for(&app, Vec::new());
    app.set_panel_tab("Masks".into());
    app.invoke_new_mask();
    app.invoke_target_changed(1);
    for (_, _, set) in crate::panel::edit::FOLDS {
        set(&app, true);
    }
    settle(&app);
    let mask = header(&app, "MASK");
    let light = header(&app, "LIGHT");
    pointer(&app, "move", mask);
    pointer(&app, "down", mask);
    pointer(&app, "move", (mask.0, mask.1 + 12.0));
    pointer(&app, "move", (mask.0, light.1 + 10.0));
    settle(&app);
    app.invoke_delete_adjustment();
    settle(&app);
    assert_eq!(app.get_target(), 0, "the mask gone, MASK with it");
    release_and_esc(&app, (mask.0, light.1 + 10.0));
    assert_eq!(order_of(&app, "develop"), names_of(first_of("develop")));
    assert!(saved().is_empty());
}

/// The window's hook hears a release before the header in hand does,
/// and takes nothing from it: the drop lands as ever.
#[test]
fn the_windows_release_hook_never_takes_a_drop() {
    let _scratch = Scratch::new("hookdrop");
    let app = develop(1400.0);
    let at = hold_look(&app);
    hook_release(&app);
    pointer(&app, "up", at);
    settle(&app);
    let order = order_of(&app, "develop");
    let look = order.iter().position(|n| n == "look").unwrap();
    let light = order.iter().position(|n| n == "light").unwrap();
    assert!(look > light, "LOOK dropped under LIGHT: {order:?}");
}
