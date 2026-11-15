//! Section order is UI state, keyed by view and stable section name.

use std::collections::BTreeMap;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::App;

const VIEWS: [&str; 7] = [
    "library", "loupe", "cull", "masks", "develop", "crop", "retouch",
];

fn current(app: &App, view: &str) -> Vec<String> {
    let model = match view {
        "library" => app.get_library_order(),
        "loupe" => app.get_loupe_order(),
        "cull" => app.get_cull_order(),
        "masks" => app.get_masks_order(),
        "develop" => app.get_develop_order(),
        "crop" => app.get_crop_order(),
        "retouch" => app.get_retouch_order(),
        _ => return Vec::new(),
    };
    model.iter().map(|name| name.to_string()).collect()
}

fn set(app: &App, view: &str, order: &[String]) {
    let model = ModelRc::new(VecModel::from(
        order.iter().map(SharedString::from).collect::<Vec<_>>(),
    ));
    match view {
        "library" => app.set_library_order(model),
        "loupe" => app.set_loupe_order(model),
        "cull" => app.set_cull_order(model),
        "masks" => app.set_masks_order(model),
        "develop" => app.set_develop_order(model),
        "crop" => app.set_crop_order(model),
        "retouch" => app.set_retouch_order(model),
        _ => {}
    }
}

/// Keep known names once, then append newly introduced sections in their
/// default order. A view's temporarily hidden sections stay in this list.
fn reconcile(defaults: &[String], saved: &[String]) -> Vec<String> {
    let mut order = Vec::with_capacity(defaults.len());
    for name in saved.iter().chain(defaults) {
        if defaults.contains(name) && !order.contains(name) {
            order.push(name.clone());
        }
    }
    order
}

pub(crate) fn show(app: &App, saved: &BTreeMap<String, Vec<String>>) {
    for view in VIEWS {
        let defaults = current(app, view);
        let order = reconcile(&defaults, saved.get(view).map(Vec::as_slice).unwrap_or(&[]));
        set(app, view, &order);
    }
}

pub(crate) fn read(app: &App) -> BTreeMap<String, Vec<String>> {
    VIEWS
        .into_iter()
        .map(|view| (view.to_string(), current(app, view)))
        .collect()
}

pub(crate) fn install(app: &App) {
    let weak = app.as_weak();
    app.on_section_moved(move |view, from, slot| {
        let Some(app) = weak.upgrade() else { return };
        let view = view.as_str();
        if !VIEWS.contains(&view) {
            return;
        }
        let mut order = current(&app, view);
        let (from, slot) = (from as usize, slot as usize);
        if from >= order.len() || slot > order.len() || from == slot || from + 1 == slot {
            return;
        }
        let name = order.remove(from);
        order.insert(if slot > from { slot - 1 } else { slot }, name);
        set(&app, view, &order);
        crate::settings::Settings::change(|settings| {
            settings.section_orders.insert(view.to_string(), order);
        });
    });
}

#[cfg(test)]
mod tests {
    use slint::ComponentHandle;

    use super::*;
    use crate::testing::{drag, labeled, remove_dir_retry, settle, window};

    #[test]
    fn restores_known_sections_once_and_appends_new_ones() {
        let defaults = ["a", "b", "c"].map(str::to_string);
        let saved = ["b", "gone", "b", "a"].map(str::to_string);
        assert_eq!(
            reconcile(&defaults, &saved),
            ["b", "a", "c"].map(str::to_string).to_vec()
        );
    }

    #[test]
    fn moving_a_develop_section_to_the_first_slot_keeps_its_fold_and_order() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-section-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::settings::use_file(Some(dir.join("settings.json")));

        let app = window(3);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 900.0));
        install(&app);
        for (_, _, set) in crate::panel::edit::FOLDS {
            set(&app, true);
        }
        app.set_collapsed_wb(false);
        settle(&app);

        let (look, _) = labeled(&app, "LOOK");
        let (wb, size) = labeled(&app, "WHITE BALANCE");
        assert!(wb.y > look.y);
        let x = wb.x + size.width / 2.0;
        let y = wb.y + size.height / 2.0;
        drag(&app, &[(x, y), (x, y - 8.0), (x, look.y - 2.0)]);
        settle(&app);

        let order = current(&app, "develop");
        assert_eq!(order[0], "white-balance");
        assert_eq!(order[1], "look");
        assert!(!app.get_collapsed_wb());
        assert!(app.get_collapsed_look());
        assert_eq!(
            crate::settings::Settings::load().section_orders["develop"],
            order
        );

        let next = window(3);
        show(&next, &crate::settings::Settings::load().section_orders);
        assert_eq!(current(&next, "develop"), order);

        crate::settings::use_file(None);
        remove_dir_retry(&dir);
    }

    #[test]
    fn moves_only_the_requested_view() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-section-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::settings::use_file(Some(dir.join("settings.json")));
        let app = window(3);
        install(&app);
        let develop = current(&app, "develop");
        let crop = current(&app, "crop");
        app.invoke_section_moved("crop".into(), 2, 0);
        assert_eq!(
            current(&app, "crop"),
            ["perspective", "crop", "rotate"]
                .map(str::to_string)
                .to_vec()
        );
        assert_eq!(current(&app, "develop"), develop);
        app.invoke_section_moved("crop".into(), 0, 1);
        assert_eq!(
            current(&app, "crop"),
            ["perspective", "crop", "rotate"]
                .map(str::to_string)
                .to_vec()
        );
        assert_ne!(current(&app, "crop"), crop);
        crate::settings::use_file(None);
        remove_dir_retry(&dir);
    }

    #[test]
    fn library_sections_move_without_changing_their_height_weights() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-section-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::settings::use_file(Some(dir.join("settings.json")));
        let app = window(3);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 900.0));
        app.set_grid_open(true);
        app.set_folder_tree_shown(true);
        app.set_share_folders(2.0);
        install(&app);
        settle(&app);

        let (roots, _) = labeled(&app, "ROOTS");
        let (folders, size) = labeled(&app, "FOLDERS");
        assert!(folders.y > roots.y);
        let x = folders.x + size.width / 2.0;
        let y = folders.y + size.height / 2.0;
        drag(&app, &[(x, y), (x, y - 8.0), (x, roots.y - 2.0)]);
        settle(&app);

        assert_eq!(current(&app, "library")[0], "folders");
        assert_eq!(app.get_share_folders(), 2.0);
        assert_eq!(current(&app, "loupe")[0], "folders");
        crate::settings::use_file(None);
        remove_dir_retry(&dir);
    }
}
