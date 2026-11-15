//! A mouse's back and forward side buttons over the whole window.
//!
//! Slint hands a press to the topmost TouchArea under the pointer,
//! whatever the button, and nothing beneath it hears it, so no element
//! of ours sees a side button everywhere: over the picture the viewport
//! has it, over the strip a cell, over a pane a slider. We read the
//! window's own events instead, before Slint routes them, and ask the
//! window (`side-button`) whether the press is its: in Cull a step to
//! the frame before or after, as the arrows step. A press taken goes no
//! further, nor does its release; one not taken goes on to Slint as
//! before, so the grid's side buttons still go to the folders visited.
//!
//! We act on the press, as the grid's side buttons do and as the arrows
//! do. A button does not repeat, and a second press without a release
//! between is not taken again.

use slint::ComponentHandle;
use slint::winit_030::EventResult;
use slint::winit_030::winit::event::{ElementState, MouseButton, WindowEvent};

use crate::App;

/// What the side buttons have heard: each button held, and whether its
/// press was taken; Shift as the keyboard last reported it.
#[derive(Debug, Default)]
pub(crate) struct SideButtons {
    held: [bool; 2],
    taken: [bool; 2],
    shift: bool,
}

impl SideButtons {
    /// One of the window's events: a side button's press or release,
    /// or a change of the modifiers. PreventDefault for a press the
    /// window took and for that press's release.
    pub(crate) fn event(&mut self, app: &App, event: &WindowEvent) -> EventResult {
        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
            }
            // A button held as the window lost the keyboard may never
            // report its release here.
            WindowEvent::Focused(false) => {
                self.held = [false; 2];
                self.taken = [false; 2];
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let i = match button {
                    MouseButton::Back => 0,
                    MouseButton::Forward => 1,
                    _ => return EventResult::Propagate,
                };
                let taken = match state {
                    ElementState::Pressed if !self.held[i] => {
                        self.held[i] = true;
                        self.taken[i] = app.invoke_side_button(i == 0, self.shift);
                        self.taken[i]
                    }
                    ElementState::Pressed => self.taken[i],
                    ElementState::Released => {
                        self.held[i] = false;
                        std::mem::take(&mut self.taken[i])
                    }
                };
                if taken {
                    return EventResult::PreventDefault;
                }
            }
            _ => {}
        }
        EventResult::Propagate
    }
}

/// The side buttons heard on `app`'s window from now on. Nothing on a
/// backend other than winit's.
pub(crate) fn install(app: &App) {
    use slint::winit_030::WinitWindowAccessor;
    let weak = app.as_weak();
    let mut buttons = SideButtons::default();
    app.window().on_winit_window_event(move |_, event| {
        weak.upgrade()
            .map_or(EventResult::Propagate, |app| buttons.event(&app, event))
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use slint::platform::{Key, PointerEventButton};
    use slint::winit_030::winit::event::{DeviceId, Modifiers};
    use slint::winit_030::winit::keyboard::ModifiersState;

    use super::*;
    use crate::testing::{click_with, folder, press, state_for, window};

    fn button(state: ElementState, back: bool) -> WindowEvent {
        WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: if back {
                MouseButton::Back
            } else {
                MouseButton::Forward
            },
        }
    }

    /// A click of a side button as winit reports it: what the window's
    /// events did with the press and with the release.
    fn side(b: &mut SideButtons, app: &App, back: bool) -> (bool, bool) {
        let prevented = |r| matches!(r, EventResult::PreventDefault);
        let down = prevented(b.event(app, &button(ElementState::Pressed, back)));
        let up = prevented(b.event(app, &button(ElementState::Released, back)));
        (down, up)
    }

    fn steps_of(app: &App) -> Rc<RefCell<Vec<(i32, bool)>>> {
        let steps = Rc::new(RefCell::new(Vec::new()));
        let seen = steps.clone();
        app.on_step(move |by, extend| seen.borrow_mut().push((by, extend)));
        steps
    }

    /// In Cull, back and forward are the arrows' step, taken with their
    /// release; Shift adds, as Shift with an arrow does. A held button
    /// steps once, even should its press come twice.
    #[test]
    fn back_and_forward_step_in_cull() {
        let app = window(5);
        let steps = steps_of(&app);
        let mut b = SideButtons::default();
        app.set_culling(true);
        assert_eq!(side(&mut b, &app, true), (true, true));
        assert_eq!(side(&mut b, &app, false), (true, true));
        // The arrows, for the record of what the buttons match.
        press(&app, Key::LeftArrow);
        press(&app, Key::RightArrow);
        assert_eq!(
            *steps.borrow(),
            [(-1, false), (1, false), (-1, false), (1, false)]
        );
        steps.borrow_mut().clear();
        b.event(
            &app,
            &WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::SHIFT)),
        );
        side(&mut b, &app, false);
        b.event(
            &app,
            &WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())),
        );
        side(&mut b, &app, true);
        assert_eq!(*steps.borrow(), [(1, true), (-1, false)]);
        steps.borrow_mut().clear();
        let prevented = |r| matches!(r, EventResult::PreventDefault);
        assert!(prevented(
            b.event(&app, &button(ElementState::Pressed, true))
        ));
        assert!(prevented(
            b.event(&app, &button(ElementState::Pressed, true))
        ));
        assert!(prevented(
            b.event(&app, &button(ElementState::Released, true))
        ));
        assert_eq!(*steps.borrow(), [(-1, false)]);
        // The Cull of a developed frame too.
        app.set_culling(false);
        app.set_cull_developed(true);
        side(&mut b, &app, false);
        assert_eq!(*steps.borrow(), [(-1, false), (1, false)]);
        // Other buttons are Slint's as ever.
        let other = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        };
        assert!(matches!(b.event(&app, &other), EventResult::Propagate));
    }

    /// Develop, the Library grid, and the lightbox step nothing, and the
    /// press goes on to Slint: the grid's own side buttons still go to
    /// the folders visited.
    #[test]
    fn nothing_steps_in_develop_or_the_library() {
        use i_slint_backend_testing::ElementHandle;
        let app = window(3);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (_state, _worker) = state_for(&app, folder(3));
        let steps = steps_of(&app);
        let grid_steps = Rc::new(RefCell::new(0));
        let seen = grid_steps.clone();
        app.on_grid_step(move |_, _, _, _| *seen.borrow_mut() += 1);
        let visits = Rc::new(RefCell::new(Vec::new()));
        let seen = visits.clone();
        app.on_visit_folder(move |back| seen.borrow_mut().push(back));
        let mut b = SideButtons::default();
        assert_eq!(side(&mut b, &app, true), (false, false), "Develop");
        press(&app, "g");
        assert!(app.get_grid_open());
        assert_eq!(side(&mut b, &app, false), (false, false), "the grid");
        press(&app, "l");
        assert!(app.get_lightbox());
        assert_eq!(side(&mut b, &app, true), (false, false), "the lightbox");
        press(&app, "l");
        // Cull with the grid over it is the grid.
        app.set_culling(true);
        assert!(app.get_grid_open());
        assert_eq!(side(&mut b, &app, true), (false, false), "Cull's grid");
        assert!(steps.borrow().is_empty());
        // What Slint does with the press it is passed on: the grid's.
        app.invoke_grid_range(0.0, 773.0, 8, 0.0);
        let cell = ElementHandle::find_by_element_id(&app, "GridSheet::ct")
            .next()
            .expect("a cell on the sheet");
        let (at, size) = (cell.absolute_position(), cell.size());
        click_with(
            &app,
            PointerEventButton::Back,
            at.x + size.width / 2.0,
            at.y + size.height / 2.0,
        );
        assert_eq!(*visits.borrow(), [true]);
        assert!(steps.borrow().is_empty());
        assert_eq!(*grid_steps.borrow(), 0);
    }

    /// A sheet, a menu, or a text field on top: nothing steps under it,
    /// and the press is left to what is on top.
    #[test]
    fn a_sheet_menu_or_field_on_top_keeps_the_frame() {
        let app = window(5);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (_state, _worker) = state_for(&app, folder(5));
        let steps = steps_of(&app);
        let mut b = SideButtons::default();
        app.set_culling(true);
        app.set_keys_open(true);
        assert_eq!(side(&mut b, &app, true), (false, false), "the keys sheet");
        app.set_keys_open(false);
        app.set_export_open(true);
        assert_eq!(side(&mut b, &app, false), (false, false), "export");
        app.set_export_open(false);
        app.global::<crate::Typing>().set_boxes(1);
        assert_eq!(side(&mut b, &app, true), (false, false), "a text box");
        app.global::<crate::Typing>().set_boxes(0);
        app.global::<crate::Typing>().set_editing(true);
        assert_eq!(side(&mut b, &app, true), (false, false), "a typed value");
        app.global::<crate::Typing>().set_editing(false);
        assert!(steps.borrow().is_empty());
        // The frame menu up over the strip.
        let (x, y) = crate::testing::strip_cell(&app, 1);
        click_with(&app, PointerEventButton::Right, x, y);
        assert!(app.get_menu_up());
        assert_eq!(side(&mut b, &app, true), (false, false), "a menu");
        press(&app, Key::Escape);
        assert!(!app.get_menu_up());
        assert!(steps.borrow().is_empty());
        // With all of them gone, the step.
        assert_eq!(side(&mut b, &app, true), (true, true));
        assert_eq!(*steps.borrow(), [(-1, false)]);
    }

    /// The ends as the arrows have them: back on the first frame and
    /// forward on the last stay where they are, and each step lands
    /// where the arrow's would, through the browser's own step.
    #[test]
    fn the_ends_hold_as_the_arrows_do() {
        let app = window(3);
        let (_state, _worker) = state_for(&app, folder(3));
        let mut b = SideButtons::default();
        app.set_culling(true);
        app.set_selected(0);
        side(&mut b, &app, true);
        assert_eq!(app.get_selected(), 0, "back on the first");
        side(&mut b, &app, false);
        assert_eq!(app.get_selected(), 1);
        side(&mut b, &app, false);
        side(&mut b, &app, false);
        assert_eq!(app.get_selected(), 2, "forward on the last");
        // The arrows from the same place land the same.
        press(&app, Key::RightArrow);
        assert_eq!(app.get_selected(), 2);
        side(&mut b, &app, true);
        press(&app, Key::LeftArrow);
        assert_eq!(app.get_selected(), 0);
        press(&app, Key::LeftArrow);
        assert_eq!(app.get_selected(), 0);
    }
}
