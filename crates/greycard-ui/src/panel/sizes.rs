//! The panes' sizes, kept between runs: the left pane's and the
//! develop panel's widths and the strip's pictures' height, as their
//! edges were last dragged. The drag and the ranges are the window's
//! (`left-size`, `right-size`, and `strip-size` in app.slint); this
//! only reads them out for the settings file and puts them back at the
//! start.

use crate::App;
use crate::settings::Settings;

/// The left pane's width the window starts at, and a double-click on
/// its edge goes back to.
pub(crate) const LEFT: f32 = 240.0;
/// The develop panel's.
pub(crate) const RIGHT: f32 = 320.0;

/// The sizes the last run left; one not kept is the default.
pub(crate) fn show(app: &App, kept: &Settings) {
    app.set_left_size(kept.left_width.unwrap_or(LEFT));
    app.set_right_size(kept.right_width.unwrap_or(RIGHT));
    app.set_strip_size(kept.strip_photo.unwrap_or(crate::strip::PHOTO));
}

/// A size as the file keeps it: none at its default.
fn kept(size: f32, default: f32) -> Option<f32> {
    (size != default).then_some(size)
}

/// The left pane's width now, as the file keeps it.
pub(crate) fn left(app: &App) -> Option<f32> {
    kept(app.get_left_size(), LEFT)
}

/// The develop panel's.
pub(crate) fn right(app: &App) -> Option<f32> {
    kept(app.get_right_size(), RIGHT)
}

/// The strip's pictures' height.
pub(crate) fn strip(app: &App) -> Option<f32> {
    kept(app.get_strip_size(), crate::strip::PHOTO)
}

#[cfg(test)]
mod tests {
    use i_slint_backend_testing::ElementHandle;
    use slint::ComponentHandle;

    use super::*;
    use crate::testing::{click, drag, settle, window};

    const W: f32 = 1500.0;
    const H: f32 = 900.0;

    fn sized(app: &App, w: f32, h: f32) {
        app.window().set_size(slint::LogicalSize::new(w, h));
        settle(app);
        settle(app);
    }

    fn loupe() -> App {
        let app = window(40);
        sized(&app, W, H);
        app
    }

    /// An element's left, top, width, and height in the window; none
    /// when it is not there.
    fn at(app: &App, id: &str) -> Option<(f32, f32, f32, f32)> {
        ElementHandle::find_by_element_id(app, &format!("App::{id}"))
            .next()
            .map(|e| {
                let (p, s) = (e.absolute_position(), e.size());
                (p.x, p.y, s.width, s.height)
            })
    }

    /// A point on a drag edge's line: 100 px down a pane's, clear of
    /// the arrow at the viewport's middle, and 700 along the strip's.
    fn edge(app: &App, id: &str) -> (f32, f32) {
        let (x, y, w, h) = at(app, id).unwrap_or_else(|| panic!("{id}"));
        if w > h {
            (700.0, y + h / 2.0)
        } else {
            (x + w / 2.0, y + 100.0)
        }
    }

    /// The pane's line an edge runs along: the left pane's right, the
    /// panel's left, the strip's top.
    fn line(app: &App, id: &str) -> f32 {
        let (x, y, w, h) = at(app, id).unwrap_or_else(|| panic!("{id}"));
        match id {
            "left-edge" => x + w,
            "right-edge" => x,
            _ => {
                assert!(w > h);
                y
            }
        }
    }

    fn drag_by(app: &App, id: &str, dx: f32, dy: f32) {
        let (x, y) = edge(app, id);
        drag(
            app,
            &[(x, y), (x + dx / 2.0, y + dy / 2.0), (x + dx, y + dy)],
        );
        settle(app);
    }

    fn double_click(app: &App, id: &str) {
        let (x, y) = edge(app, id);
        click(app, x, y);
        click(app, x, y);
        settle(app);
    }

    /// The viewport's width and height, logical pixels.
    fn view(app: &App) -> (f32, f32) {
        let (_, _, w, h) = at(app, "view").expect("the viewport");
        (w, h)
    }

    #[test]
    fn the_defaults_are_the_windows() {
        let app = loupe();
        assert_eq!(app.get_left_size(), LEFT);
        assert_eq!(app.get_right_size(), RIGHT);
        assert_eq!(app.get_strip_size(), crate::strip::PHOTO);
        assert_eq!(app.get_strip_photo(), crate::strip::PHOTO);
        // The edges sit inside the panes, along their lines.
        assert_eq!(line(&app, "left-edge"), LEFT);
        assert_eq!(line(&app, "right-edge"), W - RIGHT);
        assert_eq!(line(&app, "strip-edge"), H - app.get_strip_h());
        assert_eq!(view(&app).0, W - LEFT - RIGHT);
    }

    #[test]
    fn the_left_edge_drags_the_pane_within_its_range() {
        let app = loupe();
        let (vw, vh) = view(&app);
        drag_by(&app, "left-edge", 60.0, 0.0);
        assert_eq!(app.get_left_size(), 300.0);
        assert_eq!(line(&app, "left-edge"), 300.0);
        // The viewport gives what the pane took, and its height stays.
        assert_eq!(view(&app), (vw - 60.0, vh));
        // Past a third of the window: held there.
        drag_by(&app, "left-edge", 600.0, 0.0);
        assert_eq!(app.get_left_size(), W / 3.0);
        // Under the floor: held there.
        drag_by(&app, "left-edge", -600.0, 0.0);
        assert_eq!(app.get_left_size(), 200.0);
        // A double-click goes back to the default.
        double_click(&app, "left-edge");
        assert_eq!(app.get_left_size(), LEFT);
        assert_eq!(view(&app), (vw, vh));
    }

    #[test]
    fn the_left_edge_drags_over_the_grid_too() {
        let app = loupe();
        app.set_grid_open(true);
        settle(&app);
        assert!(
            at(&app, "right-edge").is_none(),
            "the grid covers the panel"
        );
        assert!(at(&app, "strip-edge").is_none(), "and the strip");
        drag_by(&app, "left-edge", 40.0, 0.0);
        assert_eq!(app.get_left_size(), 280.0);
    }

    #[test]
    fn the_develop_panels_edge_drags_it_within_its_range() {
        let app = loupe();
        let (vw, _) = view(&app);
        drag_by(&app, "right-edge", -100.0, 0.0);
        assert_eq!(app.get_right_size(), 420.0);
        assert_eq!(line(&app, "right-edge"), W - 420.0);
        assert_eq!(view(&app).0, vw - 100.0);
        drag_by(&app, "right-edge", -600.0, 0.0);
        assert_eq!(app.get_right_size(), W / 3.0);
        drag_by(&app, "right-edge", 600.0, 0.0);
        assert_eq!(app.get_right_size(), 280.0);
        double_click(&app, "right-edge");
        assert_eq!(app.get_right_size(), RIGHT);
    }

    #[test]
    fn the_strips_edge_drags_its_pictures_height_within_its_range() {
        let app = loupe();
        let (vw, vh) = view(&app);
        let strip_h = app.get_strip_h();
        drag_by(&app, "strip-edge", 0.0, -60.0);
        assert_eq!(app.get_strip_size(), crate::strip::PHOTO + 60.0);
        assert_eq!(app.get_strip_photo(), crate::strip::PHOTO + 60.0);
        assert_eq!(app.get_strip_h(), strip_h + 60.0);
        assert_eq!(view(&app), (vw, vh - 60.0));
        let layout = crate::cells::strip_layout(&app);
        let (_, width) = layout.place(0).unwrap();
        assert_eq!(
            width,
            crate::strip::cell_width(0.0, crate::strip::PHOTO + 60.0)
        );
        // The ceiling: the whole strip a third of the window's height.
        drag_by(&app, "strip-edge", 0.0, -900.0);
        assert!(
            (app.get_strip_h() - H / 3.0).abs() < 0.01,
            "{}",
            app.get_strip_h()
        );
        // The floor.
        drag_by(&app, "strip-edge", 0.0, 900.0);
        assert_eq!(app.get_strip_size(), crate::strip::LEAST_PHOTO);
        double_click(&app, "strip-edge");
        assert_eq!(app.get_strip_size(), crate::strip::PHOTO);
        assert_eq!(app.get_strip_h(), strip_h);
    }

    /// A window made shorter or narrower holds each size to its new
    /// ceiling, never under the default, and gives it back as it grows.
    #[test]
    fn a_smaller_window_holds_the_sizes_to_its_ceilings() {
        let app = loupe();
        app.set_left_size(480.0);
        app.set_right_size(480.0);
        app.set_strip_size(200.0);
        settle(&app);
        assert_eq!(line(&app, "left-edge"), 480.0);
        sized(&app, 900.0, 600.0);
        assert_eq!(line(&app, "left-edge"), 300.0);
        assert_eq!(line(&app, "right-edge"), 900.0 - 320.0);
        // The strip a third of the window's height.
        assert!(
            (app.get_strip_h() - 200.0).abs() < 0.01,
            "{}",
            app.get_strip_h()
        );
        assert!(app.get_strip_photo() < 200.0);
        // A third of 360 is short of the default strip: the default.
        sized(&app, 900.0, 360.0);
        assert_eq!(app.get_strip_photo(), crate::strip::PHOTO);
        sized(&app, W, H);
        assert_eq!(line(&app, "left-edge"), 480.0);
        assert_eq!(line(&app, "right-edge"), W - 480.0);
        assert_eq!(app.get_strip_photo(), 200.0);
    }

    #[test]
    fn the_sizes_are_kept_and_read_back_at_the_next_start() {
        let app = loupe();
        assert_eq!(
            (left(&app), right(&app), strip(&app)),
            (None, None, None),
            "nothing written for a size never dragged"
        );
        drag_by(&app, "left-edge", 50.0, 0.0);
        drag_by(&app, "strip-edge", 0.0, -30.0);
        let kept = crate::panel::startup::remember(&app);
        assert_eq!(kept.left_width, Some(290.0));
        assert_eq!(kept.right_width, None);
        assert_eq!(kept.strip_photo, Some(crate::strip::PHOTO + 30.0));
        // Through the file's own text.
        let text = serde_json::to_string(&kept).unwrap();
        assert!(!text.contains("right_width"), "{text}");
        let read_back: Settings = serde_json::from_str(&text).unwrap();
        let next = loupe();
        show(&next, &read_back);
        settle(&next);
        assert_eq!(line(&next, "left-edge"), 290.0);
        assert_eq!(next.get_right_size(), RIGHT);
        assert_eq!(next.get_strip_photo(), crate::strip::PHOTO + 30.0);
    }

    #[test]
    fn a_hidden_pane_comes_back_at_its_size() {
        let app = loupe();
        drag_by(&app, "left-edge", 70.0, 0.0);
        drag_by(&app, "right-edge", -70.0, 0.0);
        drag_by(&app, "strip-edge", 0.0, -40.0);
        let strip_h = app.get_strip_h();
        app.set_left_hidden(true);
        app.set_right_hidden(true);
        app.set_strip_hidden(true);
        settle(&app);
        for id in ["left-edge", "right-edge", "strip-edge"] {
            assert!(at(&app, id).is_none(), "{id} goes with its pane");
        }
        assert_eq!(view(&app).0, W);
        app.set_left_hidden(false);
        app.set_right_hidden(false);
        app.set_strip_hidden(false);
        settle(&app);
        assert_eq!(line(&app, "left-edge"), LEFT + 70.0);
        assert_eq!(line(&app, "right-edge"), W - RIGHT - 70.0);
        assert_eq!(app.get_strip_h(), strip_h);
    }

    #[test]
    fn the_lightbox_puts_sized_panes_away_and_back() {
        let app = loupe();
        drag_by(&app, "left-edge", 70.0, 0.0);
        drag_by(&app, "right-edge", -70.0, 0.0);
        drag_by(&app, "strip-edge", 0.0, -40.0);
        let strip_h = app.get_strip_h();
        app.invoke_lightbox_enter();
        settle(&app);
        assert!(app.get_left_hidden() && app.get_right_hidden() && app.get_strip_hidden());
        for id in ["left-edge", "right-edge", "strip-edge"] {
            assert!(at(&app, id).is_none(), "{id} away in the lightbox");
        }
        // The sizes kept through it.
        let kept = crate::panel::startup::remember(&app);
        assert_eq!(kept.left_width, Some(LEFT + 70.0));
        app.invoke_lightbox_leave();
        app.set_grid_open(false);
        settle(&app);
        assert_eq!(line(&app, "left-edge"), LEFT + 70.0);
        assert_eq!(line(&app, "right-edge"), W - RIGHT - 70.0);
        assert_eq!(app.get_strip_h(), strip_h);
    }

    /// A drag on any edge leaves the picture where it was: the point of
    /// it at the viewport's middle and the zoom, fitted or not, as the
    /// window's own resize does.
    #[test]
    fn the_picture_keeps_its_place() {
        for zoom in [0.0, 1.0] {
            let app = window(1);
            sized(&app, W, H);
            let (state, _worker) = crate::testing::retouch_state(&app);
            {
                let mut st = state.borrow_mut();
                st.develop.image_size = (6000, 4000);
                st.develop.center = (2500.0, 1800.0);
                st.develop.zoom = zoom;
            }
            let was = view(&app);
            drag_by(&app, "left-edge", 80.0, 0.0);
            drag_by(&app, "right-edge", -80.0, 0.0);
            drag_by(&app, "strip-edge", 0.0, -50.0);
            assert_eq!(view(&app), (was.0 - 160.0, was.1 - 50.0));
            let st = state.borrow();
            assert_eq!(st.develop.center, (2500.0, 1800.0), "zoom {zoom}");
            assert_eq!(st.develop.zoom, zoom);
        }
    }

    /// Culling's Picture row in the narrowest panel: its two buttons
    /// whole, the label over them; at the default width, beside them.
    #[test]
    fn the_culling_picture_row_keeps_its_buttons_whole() {
        let app = loupe();
        app.set_culling(true);
        app.set_panel_tab("Cull".into());
        settle(&app);
        let rows = |app: &App| {
            let button = ElementHandle::find_by_accessible_label(app, "Camera JPEG pictures")
                .next()
                .expect("the Camera JPEG button");
            let label = ElementHandle::find_by_element_type_name(app, "Text")
                .find(|e| e.accessible_label().is_some_and(|l| l == "Picture"))
                .expect("the Picture label");
            (
                label.absolute_position().y,
                button.absolute_position().y,
                button.size().width,
            )
        };
        // At the default the button is as wide as its words need.
        let (label, button, wants) = rows(&app);
        assert!(label >= button, "beside at the default: {label} {button}");
        app.set_right_size(280.0);
        settle(&app);
        settle(&app);
        let (label, button, width) = rows(&app);
        assert!(label < button, "over at 280: {label} {button}");
        assert!(width >= wants, "whole at 280: {width} {wants}");
    }

    /// The strip stays still about its selected frame as its edge is
    /// dragged, from the first move on: the frame is whole on screen at
    /// every step, and its left edge moves by no more than its own cell
    /// grows. Far along the strip, and at its very end (End).
    #[test]
    fn the_strip_holds_its_selected_frame_as_its_edge_is_dragged() {
        use slint::platform::{PointerEventButton, WindowEvent};
        for selected in [60, 98] {
            let app = window(99);
            sized(&app, W, H);
            app.set_selected(selected);
            settle(&app);
            let at = |app: &App| {
                let (left, width) = crate::cells::strip_layout(app).place(selected).unwrap();
                (left - app.get_strip_scroll(), width)
            };
            let whole = |app: &App, (left, width): (f32, f32)| {
                assert!(
                    left >= 0.0 && left + width <= app.get_strip_view() + 0.5,
                    "frame {selected} whole: {left} {width} in {}",
                    app.get_strip_view()
                );
            };
            let mut was = at(&app);
            assert!(app.get_strip_scroll() > 1000.0, "far along the strip");
            whole(&app, was);
            let (x, y) = edge(&app, "strip-edge");
            let event = |e: WindowEvent| app.window().dispatch_event(e);
            event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(x, y),
            });
            event(WindowEvent::PointerPressed {
                position: slint::LogicalPosition::new(x, y),
                button: PointerEventButton::Left,
            });
            for step in 1..=20 {
                event(WindowEvent::PointerMoved {
                    position: slint::LogicalPosition::new(x, y - 2.0 * step as f32),
                });
                slint::platform::update_timers_and_animations();
                let now = at(&app);
                whole(&app, now);
                assert!(
                    (now.0 - was.0).abs() <= (now.1 - was.1) + 1.0,
                    "frame {selected}, step {step}: {was:?} to {now:?}"
                );
                was = now;
            }
            event(WindowEvent::PointerReleased {
                position: slint::LogicalPosition::new(x, y - 40.0),
                button: PointerEventButton::Left,
            });
            settle(&app);
            assert_eq!(app.get_strip_photo(), crate::strip::PHOTO + 40.0);
        }
    }

    /// A picture made at the strip's old size that lands after a drag
    /// made the strip taller is asked for again at the new size, once,
    /// as one landing late on a grown grid is.
    #[test]
    fn a_late_picture_on_a_taller_strip_is_asked_for_again() {
        let app = loupe();
        let files = crate::testing::folder(40);
        let (state, _worker) = crate::testing::state_for(&app, files.clone());
        app.set_grid_open(false);
        settle(&app);
        drag_by(&app, "strip-edge", 0.0, -69.0);
        assert_eq!(state.borrow().views.thumb_want, 240);
        let (first, _) = state
            .borrow()
            .views
            .strip_shown
            .expect("the strip's frames");
        let row = first as usize;
        let late = crate::worker::Outcome::Thumbnail {
            index: row,
            frame: crate::versions::Key::file(files[row].clone()),
            size: 176,
            width: 2,
            height: 2,
            rgb: vec![0; 12],
            cached: false,
            seconds: 0.0,
        };
        let asked = |st: &crate::State| st.views.thumb_asked[row];
        assert!(asked(&state.borrow()) < 240);
        crate::panel::deliver::deliver(&app, late);
        assert_eq!(
            asked(&state.borrow()),
            240,
            "asked again at the strip's size"
        );
    }

    /// The strip asks the thumbnail pool for the long edge its row
    /// needs: the default's at the default, larger as it grows, and
    /// back down as it shrinks.
    #[test]
    fn a_taller_strip_asks_for_larger_pictures() {
        let app = loupe();
        let files = crate::testing::folder(40);
        let (state, _worker) = crate::testing::state_for(&app, files);
        app.set_grid_open(false);
        settle(&app);
        app.invoke_strip_range(0, 5);
        assert_eq!(state.borrow().views.thumb_want, crate::worker::THUMB_WIDTH);
        drag_by(&app, "strip-edge", 0.0, -69.0);
        assert_eq!(app.get_strip_photo(), 160.0);
        assert!(state.borrow().views.strip_shown.is_some());
        assert_eq!(state.borrow().views.thumb_want, 240);
        // The cells drawn are the taller ones.
        let rings: Vec<f32> = ElementHandle::find_by_element_id(&app, "Filmstrip::frame")
            .map(|e| e.size().height)
            .collect();
        assert!(!rings.is_empty());
        let band = app.global::<crate::Theme>().get_space_1();
        for h in rings {
            assert_eq!(h, 160.0 + band + 2.0 * crate::strip::RING);
        }
        double_click(&app, "strip-edge");
        assert_eq!(state.borrow().views.thumb_want, crate::worker::THUMB_WIDTH);
    }
}
