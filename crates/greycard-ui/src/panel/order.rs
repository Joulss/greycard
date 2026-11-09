//! The Develop sections' order, kept by stable name between runs.
//! The Slint list supplies the default order and the section bodies;
//! this only reconciles old settings and moves a name on drop.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::App;

pub(crate) fn read(app: &App) -> Vec<String> {
    app.get_develop_order()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

pub(crate) fn show(app: &App, kept: &[String]) {
    let defaults = read(app);
    let mut order = Vec::with_capacity(defaults.len());
    for name in kept.iter().chain(defaults.iter()) {
        if defaults.contains(name) && !order.contains(name) {
            order.push(name.clone());
        }
    }
    set(app, &order);
}

fn set(app: &App, order: &[String]) {
    app.set_develop_order(ModelRc::new(VecModel::from(
        order.iter().map(SharedString::from).collect::<Vec<_>>(),
    )));
}

pub(crate) fn install(app: &App) {
    let weak = app.as_weak();
    app.on_develop_section_moved(move |from, slot| {
        let Some(app) = weak.upgrade() else { return };
        let mut order = read(&app);
        let (from, slot) = (from as usize, slot as usize);
        if from >= order.len() || slot > order.len() || from == slot || from + 1 == slot {
            return;
        }
        let moved = order.remove(from);
        order.insert(if slot > from { slot - 1 } else { slot }, moved);
        set(&app, &order);
        crate::settings::Settings::change(|settings| settings.develop_order = order);
    });
}

#[cfg(test)]
mod tests {
    use slint::ComponentHandle;

    use super::*;
    use crate::testing::{drag, labeled, remove_dir_retry, settle, window};

    #[test]
    fn a_develop_section_dragged_to_the_top_keeps_its_fold_and_order() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-develop-order-{}-{}",
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
        let (wb, wb_size) = labeled(&app, "WHITE BALANCE");
        assert!(wb.y > look.y, "White balance starts below Look");
        let x = wb.x + wb_size.width / 2.0;
        let from = wb.y + wb_size.height / 2.0;
        drag(&app, &[(x, from), (x, from - 8.0), (x, look.y - 2.0)]);
        settle(&app);

        let order = read(&app);
        assert_eq!(order[0], "white-balance");
        assert_eq!(order[1], "look");
        assert!(!app.get_collapsed_wb(), "the moved section stays open");
        assert!(app.get_collapsed_look(), "the other fold stays closed");
        assert_eq!(crate::settings::Settings::load().develop_order, order);

        let next = window(3);
        show(&next, &crate::settings::Settings::load().develop_order);
        assert_eq!(read(&next), order);

        crate::settings::use_file(None);
        remove_dir_retry(&dir);
    }
}
