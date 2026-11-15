use std::path::Path;
use std::rc::Rc;

use greycard_core::CameraProfile;
use greycard_core::color::{Matrix3, resolve_white_balance};
use greycard_edit::grading::{Grading, Range, Wheel};
use greycard_edit::mixer::BANDS;
use greycard_edit::{Edit, LocalWhite, Tint, WhiteBalance, WhiteShift};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::worker::Worker;
use crate::{App, Shared, State, finish, wheel};

/// The panel's white balance key for an edit's white balance.
pub(crate) fn white_key(wb: &WhiteBalance) -> WhiteKey {
    match *wb {
        WhiteBalance::AsShot => (true, 0.0, 0.0),
        WhiteBalance::Custom { temperature, tint } => (false, temperature as f32, tint as f32),
        // A picture's sliders are its offset, mired and Duv.
        WhiteBalance::Offset { mired, tint } => (false, mired as f32, tint as f32),
    }
}

/// Eight band values from the panel's model, short ones padded.
pub(crate) fn band_values(model: ModelRc<f32>) -> [f32; BANDS] {
    let mut out = [0.0; BANDS];
    for (i, v) in model.iter().take(BANDS).enumerate() {
        out[i] = v;
    }
    out
}

pub(crate) fn band_model(values: &[f32; BANDS]) -> ModelRc<f32> {
    ModelRc::new(VecModel::from(values.to_vec()))
}

/// Put the chosen band's values on the mixer's sliders.
/// The sliders' values into the chosen band's place in the arrays.
pub(crate) fn store_mixer_band(app: &App) {
    let band = (app.get_mixer_band().max(0) as usize).min(BANDS - 1);
    let mut hue = band_values(app.get_mixer_hues());
    let mut sat = band_values(app.get_mixer_saturations());
    let mut lum = band_values(app.get_mixer_luminances());
    hue[band] = app.get_mixer_hue();
    sat[band] = app.get_mixer_saturation();
    lum[band] = app.get_mixer_luminance();
    app.set_mixer_hues(band_model(&hue));
    app.set_mixer_saturations(band_model(&sat));
    app.set_mixer_luminances(band_model(&lum));
}

/// The chosen band's weight onto the black and white's slider, and
/// the filter its weights are onto the row of filters.
pub(crate) fn show_bw_band(app: &App) {
    let weights = band_values(app.get_bw_weights());
    let band = (app.get_bw_band().max(0) as usize).min(BANDS - 1);
    app.set_bw_weight(weights[band]);
    // The filter is the shape of the weights alone, so the strength
    // is left out of this: the row says Red while the slider says
    // how much of it.
    let bw = greycard_edit::BlackWhite {
        enabled: app.get_bw_enabled(),
        weights,
        ..greycard_edit::BlackWhite::OFF
    };
    app.set_bw_filter(bw.filter().map(|f| f.name()).unwrap_or("").into());
}

pub(crate) fn show_mixer_band(app: &App) {
    let band = (app.get_mixer_band().max(0) as usize).min(BANDS - 1);
    app.set_mixer_hue(band_values(app.get_mixer_hues())[band]);
    app.set_mixer_saturation(band_values(app.get_mixer_saturations())[band]);
    app.set_mixer_luminance(band_values(app.get_mixer_luminances())[band]);
}

/// The wheels' three values as the panel holds them.
pub(crate) fn grade_values(model: ModelRc<f32>) -> [f32; 3] {
    let mut out = [0.0; 3];
    for (i, v) in model.iter().take(3).enumerate() {
        out[i] = v;
    }
    out
}

pub(crate) fn grade_model(values: &[f32; 3]) -> ModelRc<f32> {
    ModelRc::new(VecModel::from(values.to_vec()))
}

/// Put the chosen wheel's values on the grading's sliders.
pub(crate) fn show_grade_range(app: &App) {
    let i = (app.get_grade_range().max(0) as usize).min(2);
    app.set_grade_hue(grade_values(app.get_grade_hues())[i]);
    app.set_grade_saturation(grade_values(app.get_grade_saturations())[i]);
}

/// Make wheel `i` the sliders' wheel: its values onto them, and the
/// wheels redrawn, which is also what puts an enlarged one at its
/// size.
pub(crate) fn choose_grade_range(app: &App, i: usize) {
    app.set_grade_range(i.min(2) as i32);
    show_grade_range(app);
    draw_wheels(app);
    app.window().request_redraw();
}

/// `wheel-hand` for the tint's ring; 0 to 2 are the grading wheels.
pub(crate) const TINT_HAND: i32 = 3;

/// Which of a wheel's two numbers its drag keeps, by the keys held:
/// Shift the hue, Ctrl (⌘ on a Mac, which Slint reports as Control)
/// the amount, and Shift when both are down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hold {
    #[default]
    Free,
    Hue,
    Amount,
}

impl Hold {
    pub(crate) fn of(shift: bool, control: bool) -> Self {
        if shift {
            Hold::Hue
        } else if control {
            Hold::Amount
        } else {
            Hold::Free
        }
    }
}

/// A wheel's hue and amount after a press or a drag at `(x, y)`,
/// from `(hue0, amount0)`, the wheel's when the hold began under one
/// and as it stands under none: Shift's hold keeps the hue and reads
/// the amount along its ray (nothing at the center and past it),
/// Ctrl's keeps the amount, and a free drag takes both from the
/// pointer (the hue kept at the center, where there is none to read).
pub(crate) fn held_pick(x: f32, y: f32, hold: Hold, (hue0, amount0): (f32, f32)) -> (f32, f32) {
    if hold == Hold::Hue {
        return (hue0, wheel::pick_along(x, y, hue0));
    }
    let (hue, amount) = wheel::pick(x, y);
    match hold {
        Hold::Amount => (hue.unwrap_or(hue0), amount0),
        _ => (hue.unwrap_or(hue0), amount),
    }
}

/// A drag in a wheel, from its press to its release: the wheel
/// (`wheel-hand`'s numbering), the hold its keys make, the wheel's
/// hue and amount when that hold began, and where the pointer last
/// was. The keys are read at the press, at every move, and at every
/// key that goes down or up in between, so a hold can begin or end
/// at any point of the drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WheelDrag {
    pub(crate) hand: i32,
    hold: Hold,
    from: (f32, f32),
    at: (f32, f32),
}

impl WheelDrag {
    /// A press in wheel `hand` at `at`, the wheel at `now`.
    pub(crate) fn press(hand: i32, hold: Hold, now: (f32, f32), at: (f32, f32)) -> Self {
        WheelDrag {
            hand,
            hold,
            from: now,
            at,
        }
    }

    /// The keys as they stand, the wheel at `now`: a hold taken up
    /// keeps the hue or the amount the wheel has at that moment, not
    /// the press's, so the point does not jump back; one let go of
    /// frees the point. Whether the hold changed, for the caller to
    /// put the point where the pointer now says without waiting for
    /// it to move.
    pub(crate) fn keys(&mut self, hold: Hold, now: (f32, f32)) -> bool {
        if hold == self.hold {
            return false;
        }
        self.hold = hold;
        self.from = now;
        true
    }

    /// The wheel's hue and amount with the pointer at `at`, the wheel
    /// at `now`.
    pub(crate) fn point(&mut self, at: (f32, f32), now: (f32, f32)) -> (f32, f32) {
        self.at = at;
        let from = if self.hold == Hold::Free {
            now
        } else {
            self.from
        };
        held_pick(at.0, at.1, self.hold, from)
    }

    /// Where the pointer last was.
    pub(crate) fn at(&self) -> (f32, f32) {
        self.at
    }

    pub(crate) fn hold(&self) -> Hold {
        self.hold
    }
}

/// Wheel `hand`'s hue and amount as the panel holds them.
fn hand_values(app: &App, hand: i32) -> (f32, f32) {
    if hand == TINT_HAND {
        return (app.get_tint_hue(), app.get_tint_amount());
    }
    let i = (hand.max(0) as usize).min(2);
    (
        grade_values(app.get_grade_hues())[i],
        grade_values(app.get_grade_saturations())[i],
    )
}

/// Wheel `hand`'s picture, pixels across, as it is drawn now.
fn hand_size(app: &App, hand: i32) -> usize {
    if hand == TINT_HAND {
        RING
    } else if app.get_grade_big() == hand {
        WHEEL_BIG
    } else {
        WHEEL_SMALL
    }
}

/// The drag's wheel set from the pointer at `at`, redrawn, and its
/// guide put on the panel.
fn drag_to(app: &App, d: &mut WheelDrag, at: (f32, f32)) {
    let (hue, amount) = d.point(at, hand_values(app, d.hand));
    if d.hand == TINT_HAND {
        app.set_tint_hue(hue);
        app.set_tint_amount(amount);
        show_tint(app);
        app.window().request_redraw();
    } else {
        let i = (d.hand.max(0) as usize).min(2);
        let mut hues = grade_values(app.get_grade_hues());
        let mut sats = grade_values(app.get_grade_saturations());
        (hues[i], sats[i]) = (hue, amount);
        app.set_grade_hues(grade_model(&hues));
        app.set_grade_saturations(grade_model(&sats));
        choose_grade_range(app, i);
    }
    app.set_wheel_guide(wheel_guide(d.hold(), (hue, amount), hand_size(app, d.hand)));
}

/// A move in the drag's wheel, with the keys as the move carries them.
/// The drag, if it is wheel `hand`'s and that wheel is still in hand.
/// A wheel put down mid-drag (Esc, another section's control, or the
/// panel shown another edit: another frame, another target, an undo)
/// ends the drag and its hold there, the guide with them: its values
/// are no longer what the drag began from. Slint passes a move or a
/// key on only from a wheel whose own press is held, so a drag whose
/// wheel went away with a view change is never read either; the next
/// press replaces it.
fn live_drag(app: &App, drag: &std::cell::Cell<Option<WheelDrag>>, hand: i32) -> Option<WheelDrag> {
    let d = drag.get()?;
    if d.hand == hand && app.get_wheel_hand() == hand {
        return Some(d);
    }
    drag_released(app, drag);
    None
}

fn drag_moved(
    app: &App,
    drag: &std::cell::Cell<Option<WheelDrag>>,
    hand: i32,
    at: (f32, f32),
    hold: Hold,
) {
    let Some(mut d) = live_drag(app, drag, hand) else {
        return;
    };
    d.keys(hold, hand_values(app, d.hand));
    drag_to(app, &mut d, at);
    drag.set(Some(d));
}

/// A key gone down or up in a drag: a changed hold takes effect with
/// the pointer where it is, no move needed.
fn drag_keys(app: &App, drag: &std::cell::Cell<Option<WheelDrag>>, hand: i32, hold: Hold) {
    let Some(mut d) = live_drag(app, drag, hand) else {
        return;
    };
    if d.keys(hold, hand_values(app, d.hand)) {
        let at = d.at();
        drag_to(app, &mut d, at);
    }
    drag.set(Some(d));
}

/// The drag over, and its guide gone.
fn drag_released(app: &App, drag: &std::cell::Cell<Option<WheelDrag>>) {
    drag.set(None);
    app.set_wheel_guide(crate::WheelGuide::default());
}

/// The marker's outer radius in a hue circle's picture, as a fraction
/// of its width: [`draw_hue_circle`]'s dark rim, a thirty-second of
/// the picture and half again.
const MARKER_RIM: f32 = 1.5 / 32.0;

/// The guide a held drag draws in a wheel `n` pixels across whose hue
/// and amount are `(hue, amount)`: under Shift, the hue's line from
/// the center to the rim; under Ctrl, the circle the amount's radius
/// draws; none for a free drag. In the picture's coordinates, 0 to 1
/// across and down, on [`draw_hue_circle`]'s center and radius, and
/// broken where the marker sits, so the marker reads as over it.
pub(crate) fn wheel_guide(hold: Hold, (hue, amount): (f32, f32), n: usize) -> crate::WheelGuide {
    let n = n as f32;
    // The picture's center and rim, its pixels' centers at (i + 0.5) / n.
    let rim = 0.5 - 1.0 / n;
    let (mx, my) = wheel::place(hue, amount);
    // The marker's distance from the center.
    let t = ((2.0 * mx - 1.0).hypot(2.0 * my - 1.0)) * rim;
    let (s, c) = hue.to_radians().sin_cos();
    let at = |d: f32| (0.5 + d * c, 0.5 - d * s);
    let g = MARKER_RIM;
    match hold {
        Hold::Free => crate::WheelGuide::default(),
        Hold::Hue => {
            // Center to the marker's near rim, and its far rim to the
            // wheel's; a stroke the marker covers whole is none.
            let near = (t - g).max(0.0);
            let far = (t + g).min(rim);
            let (x0, y0) = at(0.0);
            let (x1, y1) = at(near);
            let (x2, y2) = at(far);
            let (x3, y3) = at(rim);
            crate::WheelGuide {
                kind: 1,
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                r: 0.0,
            }
        }
        Hold::Amount => {
            // The arc's ends where the circle meets the marker's rim:
            // a chord of `g` either side of the marker. A circle the
            // marker covers whole is none.
            let r = if g < 2.0 * t { t } else { 0.0 };
            let off = if r > 0.0 {
                2.0 * (g / (2.0 * t)).asin()
            } else {
                0.0
            };
            let on = |a: f32| (0.5 + r * a.cos(), 0.5 - r * a.sin());
            let a = hue.to_radians();
            let (x0, y0) = on(a + off);
            let (x1, y1) = on(a - off);
            crate::WheelGuide {
                kind: 2,
                x0,
                y0,
                x1,
                y1,
                r,
                ..Default::default()
            }
        }
    }
}

/// `edit` without the section of the wheel `hand`: its grading, or
/// its tint, in the global look and every mask's.
pub(crate) fn without_wheel(mut edit: Edit, hand: i32) -> Edit {
    let clear = |grading: &mut Grading, tint: &mut Tint| {
        if hand == TINT_HAND {
            *tint = Tint::default();
        } else {
            *grading = Grading::default();
        }
    };
    clear(&mut edit.grading, &mut edit.tint);
    for a in &mut edit.adjustments {
        clear(&mut a.look.grading, &mut a.look.tint);
    }
    edit
}

/// Wheel `hand` taken in hand by a press in it: ringed, and the rest
/// of the panel's edit kept to tell when another section moves.
fn take_wheel(st: &mut State, app: &App, hand: i32) {
    let edit = crate::panel::edit::read_edit(app, &st.develop.edit, st.develop.target);
    st.develop.wheel_rest = Some(without_wheel(edit, hand));
    app.set_wheel_hand(hand);
}

/// The wheel in hand put down if a control outside its section has
/// moved the panel's edit since it was taken.
pub(crate) fn keep_wheel(st: &mut State, app: &App) {
    let hand = app.get_wheel_hand();
    if hand < 0 {
        return;
    }
    let edit = crate::panel::edit::read_edit(app, &st.develop.edit, st.develop.target);
    if st.develop.wheel_rest.as_ref() != Some(&without_wheel(edit, hand)) {
        app.set_wheel_hand(-1);
        st.develop.wheel_rest = None;
    }
}

/// The Oklab chroma a pick onto a wheel at nothing reads as the
/// wheel's whole amount.
const PICK_CHROMA: f32 = 0.1;

/// A wheel's amount after a pick of a color of Oklab `chroma`: its
/// own, or with none to keep, the color's chroma over
/// [`PICK_CHROMA`], so a pick onto an empty wheel shows.
fn picked_amount(amount: f32, chroma: f32) -> f32 {
    if amount > 0.0 {
        amount
    } else {
        (chroma / PICK_CHROMA).clamp(0.0, 1.0)
    }
}

/// The wheel in hand turned to `hue` at its own amount, as a pick
/// from the picture sets it, or at an amount from the picked color's
/// `chroma` if it had none: one change, one step of the history.
pub(crate) fn set_hand_hue(app: &App, hand: i32, hue: f32, chroma: f32) {
    let hue = hue.rem_euclid(360.0);
    if hand == TINT_HAND {
        app.set_tint_hue(hue);
        app.set_tint_amount(picked_amount(app.get_tint_amount(), chroma));
        show_tint(app);
    } else {
        let i = (hand.max(0) as usize).min(2);
        let mut hues = grade_values(app.get_grade_hues());
        hues[i] = hue;
        app.set_grade_hues(grade_model(&hues));
        let mut sats = grade_values(app.get_grade_saturations());
        sats[i] = picked_amount(sats[i], chroma);
        app.set_grade_saturations(grade_model(&sats));
        choose_grade_range(app, i);
    }
    app.invoke_view_changed();
}

/// The tint as the panel shows it: the section's switch, the hue
/// round the circle and the amount held to its ends, which is what
/// everything downstream of the two sliders wants.
pub(crate) fn panel_tint(app: &App) -> Tint {
    Tint {
        enabled: app.get_tint_enabled(),
        ..Tint::new(
            app.get_tint_hue().rem_euclid(360.0),
            app.get_tint_amount().clamp(0.0, 1.0),
        )
    }
}

/// The tint's ring, its swatch and its Hue reading: the ring's marker
/// where the two sliders put it, a square of what a mid grey becomes
/// under the tint as it is set, which is the tint's whole effect, and
/// the hue to a tenth of a degree, which is finer than Slint's own
/// rounding says. With the switch off the swatch still shows the tint
/// the sliders hold, dimmed with the ring, as a switched-off section's
/// controls keep their values.
pub(crate) fn show_tint(app: &App) {
    let tint = Tint {
        enabled: true,
        ..panel_tint(app)
    };
    let [a, b] = tint.applied(greycard_edit::tint::MID_LIGHTNESS, 0.0, 0.0);
    let c = finish::oklab_to_srgb([greycard_edit::tint::MID_LIGHTNESS, a, b])
        .map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
    app.set_tint_color(slint::Color::from_rgb_u8(c[0], c[1], c[2]));
    app.set_tint_hue_text(format!("{:.1}\u{b0}", tint.hue).into());
    app.set_tint_image(draw_hue_circle(tint.hue, tint.amount, RING));
}

/// The grading as the panel shows it.
pub(crate) fn read_grading(app: &App) -> Grading {
    let hues = grade_values(app.get_grade_hues());
    let sats = grade_values(app.get_grade_saturations());
    let mut grading = Grading {
        enabled: app.get_grading_enabled(),
        balance: app.get_grade_balance().clamp(-1.0, 1.0),
        ..Grading::default()
    };
    for range in Range::ALL {
        *grading.wheel_mut(range) = Wheel {
            hue: hues[range.index()],
            saturation: sats[range.index()],
        };
    }
    grading
}

/// Redraw the three wheels' pictures, the enlarged one at its size.
pub(crate) fn draw_wheels(app: &App) {
    let grading = read_grading(app);
    let big = app.get_grade_big();
    let images: Vec<slint::Image> = Range::ALL
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let n = if big == i as i32 {
                WHEEL_BIG
            } else {
                WHEEL_SMALL
            };
            let w = grading.wheel(*r);
            draw_hue_circle(w.hue, w.saturation, n)
        })
        .collect();
    app.set_grade_images(ModelRc::new(VecModel::from(images)));
}

/// The wheels' pictures, pixels across: in the row, and enlarged.
pub(crate) const WHEEL_SMALL: usize = 96;

pub(crate) const WHEEL_BIG: usize = 320;

/// The tint ring's picture, pixels across. It is shown at 176 and
/// redrawn on every move of either slider, so it is drawn nearer the
/// size it is seen at than the enlarged wheel is: enough over 176 to
/// stay crisp at a display scale of one and a half, and no more.
pub(crate) const RING: usize = 256;

/// The Oklab lightness and chroma a hue circle is painted at, the
/// grading wheels' and the tint ring's rim: the same numbers the
/// `hue-circle` gradient on the Hue sliders' tracks was worked out
/// at, so a ring and its slider name the same color at a hue.
pub(crate) const CIRCLE_LIGHTNESS: f32 = 0.72;

pub(crate) const CIRCLE_CHROMA: f32 = 0.13;

/// A hue circle's picture: Oklab's hues around, chroma outward to
/// the sliders' own, at one lightness, with a marker where a hue and
/// a strength put it. Y up, so the marker's angle is the hue as
/// `grading.rs` and `tint.rs` both have it. The three grading wheels
/// and the tint's ring are this one picture; all that differs is
/// what each calls the two numbers. `GREYCARD_UI_WHEEL=FILE` dumps
/// the last one drawn.
pub(crate) fn draw_hue_circle(hue: f32, strength: f32, n: usize) -> slint::Image {
    let radius = n as f32 / 2.0 - 1.0;
    let center = (n as f32 - 1.0) / 2.0;
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(n as u32, n as u32);
    let px = buf.make_mut_slice();
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - center, center - y as f32);
            let d = (dx * dx + dy * dy).sqrt();
            let alpha = ((radius - d) + 0.5).clamp(0.0, 1.0);
            let chroma = CIRCLE_CHROMA * (d / radius).min(1.0);
            let (a, b) = if d > 0.0 {
                (chroma * dx / d, chroma * dy / d)
            } else {
                (0.0, 0.0)
            };
            let rgb =
                finish::oklab_to_srgb([CIRCLE_LIGHTNESS, a, b]).map(|v| (v * 255.0).round() as u8);
            px[y * n + x] = slint::Rgba8Pixel {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
                a: (alpha * 255.0).round() as u8,
            };
        }
    }
    // The marker: where the hue and the strength are, a white dot
    // with a dark rim.
    let (mx, my) = wheel::place(hue, strength);
    let (cx, cy) = (
        center + (2.0 * mx - 1.0) * radius,
        center - (2.0 * my - 1.0) * radius,
    );
    for y in 0..n {
        for x in 0..n {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let p = &mut px[y * n + x];
            let dot = n as f32 / 32.0;
            if d < dot {
                *p = slint::Rgba8Pixel {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                };
            } else if d < dot * 1.5 {
                *p = slint::Rgba8Pixel {
                    r: 0x14,
                    g: 0x14,
                    b: 0x14,
                    a: 255,
                };
            }
        }
    }
    if let Some(path) = std::env::var_os("GREYCARD_UI_WHEEL")
        && let Some(img) = image::RgbaImage::from_raw(n as u32, n as u32, buf.as_bytes().to_vec())
        && let Err(e) = img.save(&path)
    {
        tracing::warn!("wheel {}: {e}", Path::new(&path).display());
    }
    slint::Image::from_rgba8(buf)
}

/// The panel's white balance: as shot, temperature, tint.
pub(crate) type WhiteKey = (bool, f32, f32);

/// The working-space matrix that takes the image on the GPU from the
/// white balance it was developed at to the panel's: the panel's camera
/// to working matrix, the ratio of the gains, and the inverse of the
/// base's. Exact for the matrix and the gains; the demosaic and the
/// highlights saw the old gains, which is what the develop that follows
/// corrects.
/// The camera profile the develop is converting through: the edit's
/// DCP when it names one that reads, else the file's own.
///
/// Resolved once per choice and kept. The preview matrices were built
/// through whichever profile was in hand, so a change throws them
/// away; the picture on the GPU is left alone, since the develop that
/// follows the change replaces it and brings its own base.
pub(crate) fn white_profile(st: &mut State) -> Option<CameraProfile> {
    let want = st.develop.edit.camera.profile.name();
    if st
        .develop
        .white_profile
        .as_ref()
        .is_none_or(|(had, _)| had != want)
    {
        let want = want.to_string();
        let chosen = st
            .develop
            .edit
            .camera
            .profile(&greycard_edit_io::Stores::user())
            .map(|p| p.camera.clone());
        let embedded = st.develop.frame.as_ref().map(|(_, p)| (**p).clone());
        st.develop.white_profile = chosen.or(embedded).map(|p| (want, p));
        st.develop.white_cache = None;
    }
    st.develop.white_profile.as_ref().map(|(_, p)| p.clone())
}

pub(crate) fn preview_white(st: &mut State, panel: WhiteKey) -> Matrix3 {
    const IDENTITY: Matrix3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    // A picture, by its name: from the light it was developed at to
    // the one the sliders name, with no profile in it; nothing until
    // its own develop has said what that light was.
    if open_is_picture(st) {
        return st
            .develop
            .base_white
            .and_then(|b| b.picture)
            .and_then(|(own, shown)| {
                greycard_edit::white::picture_light(&panel_white(panel, true), own)
                    .and_then(|light| greycard_edit::white::adaptation(shown, light))
            })
            .unwrap_or(IDENTITY);
    }
    // Before the cache: resolving the profile is what notices a change
    // of profile, and that empties the cache.
    let Some(profile) = white_profile(st) else {
        return IDENTITY;
    };
    if let Some((key, m)) = st.develop.white_cache
        && key == panel
    {
        return m;
    }
    let (Some(base), Some((frame, _))) = (st.develop.base_white, st.develop.frame.as_ref()) else {
        return IDENTITY;
    };
    let white = panel_white(panel, false);
    let m = match resolve_white_balance(frame, &profile, white.white_point()) {
        // The arithmetic a mask's own white balance is made with too,
        // so the preview and a local cannot read one white two ways.
        // The matrix to a resolved white reads no temperature; the
        // base's stands in where it has none.
        Ok(wb) => match WhiteShift::from_parts(
            profile,
            base.gains,
            base.matrix,
            base.temp_tint.unwrap_or(wb.temp_tint),
        )
        .to(&wb)
        {
            Some(m) => m,
            None => {
                // Per frame while a slider moves; the decode warned
                // once about the file already.
                tracing::debug!("preview white balance: the base matrix has no inverse; identity");
                IDENTITY
            }
        },
        Err(e) => {
            tracing::debug!("preview white balance: {e}; identity");
            IDENTITY
        }
    };
    st.develop.white_cache = Some((panel, m));
    m
}

/// What the masks' own white balances are made matrices from: the
/// white the picture on the GPU was developed at and the profile it
/// was resolved through. `None` for a picture that is not a raw.
///
/// A picture held up while the next frame's develop is on its way
/// keeps what its own masks were made from (`Develop::shown_white`):
/// the frame and its profile are the next one's by then, and its
/// masks would otherwise flash back to the global's white.
///
/// The white a mask's offset is from is the panel's, `panel`, not the
/// develop's: while the global's slider moves ahead of the develop, a
/// mask that follows it moves with the rest of the picture rather
/// than holding still until the develop lands. Once it lands the two
/// are one white.
pub(crate) fn local_white_shift(st: &mut State, panel: WhiteKey) -> Option<WhiteShift> {
    if st.develop.held.is_some() {
        return st.develop.shown_white.clone();
    }
    let global = global_white(st, panel);
    if open_is_picture(st) {
        let shift = picture_own(st).and(global).map(WhiteShift::picture);
        st.develop.shown_white = shift.clone();
        return shift;
    }
    let monochrome = st
        .develop
        .frame
        .as_ref()
        .is_some_and(|(f, _)| f.layout == greycard_core::raw::SensorLayout::Monochrome);
    let shift = match (white_profile(st), st.develop.base_white) {
        (_, Some(base)) if monochrome => base
            .temp_tint
            .map(|own| WhiteShift::neutral(global.unwrap_or(own))),
        (Some(profile), Some(base)) => base.temp_tint.map(|own| {
            WhiteShift::from_parts(profile, base.gains, base.matrix, global.unwrap_or(own))
        }),
        _ => None,
    };
    st.develop.shown_white = shift.clone();
    shift
}

/// The global white balance as a temperature and tint, As shot
/// resolved to the kelvin and Duv the camera's gains mean: where a
/// mask's own starts when it is turned on, so nothing jumps.
pub(crate) fn global_temp_tint(st: &mut State, panel: WhiteKey) -> Option<(f32, f32)> {
    global_white(st, panel).map(|tt| (tt.cct as f32, tt.duv as f32))
}

/// [`global_temp_tint`] at full precision: As shot as the develop
/// resolves it, so a mask's offset at the develop's own white is from
/// that white to the last bit.
fn global_white(st: &mut State, panel: WhiteKey) -> Option<greycard_core::TempTint> {
    if open_is_picture(st) {
        let own = picture_own(st)?;
        return greycard_edit::white::picture_light(&panel_white(panel, true), own);
    }
    let (as_shot, temperature, tint) = panel;
    if !as_shot {
        return Some(greycard_core::TempTint {
            cct: temperature as f64,
            duv: tint as f64,
        });
    }
    let profile = white_profile(st)?;
    let (frame, _) = st.develop.frame.as_ref()?;
    greycard_core::color::as_shot_temp_tint(frame, &profile).ok()
}

/// The panel's mask white balance: on, the light its sliders name, or
/// with Follow global on the offset they name, the Temperature slider
/// then in mired; switched off, that kept; and off with none kept,
/// following the global.
pub(crate) fn panel_local_white(app: &App) -> LocalWhite {
    let (temperature, tint) = (
        app.get_local_temperature() as f64,
        app.get_local_tint() as f64,
    );
    match (
        app.get_local_wb_on(),
        app.get_local_wb_kept(),
        app.get_local_wb_relative(),
    ) {
        (true, _, false) => LocalWhite::Absolute { temperature, tint },
        (true, _, true) => LocalWhite::Relative {
            mired: temperature,
            tint,
        },
        (false, true, false) => LocalWhite::Kept { temperature, tint },
        (false, true, true) => LocalWhite::KeptRelative {
            mired: temperature,
            tint,
        },
        (false, false, _) => LocalWhite::Off,
    }
}

/// The panel's global white as the temperature and tint a mask's
/// offset is from.
fn panel_global(st: &mut State, app: &App) -> Option<greycard_core::TempTint> {
    let key = (app.get_as_shot(), app.get_temperature(), app.get_tint());
    global_white(st, key)
}

/// Where a mask's sliders start that hold no light of their own: the
/// global's white, held to their range; as an offset, none.
fn start_at_global(st: &mut State, app: &App) {
    // A picture's mask names no light: its white is an offset from the
    // picture's, and Follow global is not offered.
    if app.get_picture_open() {
        app.set_local_wb_relative(true);
    }
    if app.get_local_wb_relative() {
        app.set_local_temperature(0.0);
        app.set_local_tint(0.0);
        return;
    }
    let key = (app.get_as_shot(), app.get_temperature(), app.get_tint());
    if let Some((temperature, tint)) = global_temp_tint(st, key) {
        app.set_local_temperature(temperature.clamp(2000.0, 12000.0));
        app.set_local_tint(tint.clamp(-0.05, 0.05));
    }
}

/// The light a Neutral pick names for a mask, as the panel stores it:
/// the light, or with Follow global on, the light less the global's.
pub(crate) fn local_from_light(
    st: &mut State,
    app: &App,
    light: greycard_core::TempTint,
) -> Option<(f32, f32)> {
    let named = LocalWhite::Absolute {
        temperature: light.cct,
        tint: light.duv,
    };
    // Held to the range first, so the offset is from the light the
    // sliders would show, not from one past it.
    let named = named.clamped();
    let stored = if app.get_local_wb_relative() {
        named.to_relative(panel_global(st, app)?)
    } else {
        named
    };
    match stored.clamped() {
        LocalWhite::Absolute { temperature, tint }
        | LocalWhite::Relative {
            mired: temperature,
            tint,
        } => Some((temperature as f32, tint as f32)),
        _ => None,
    }
}

/// The sliders' temperature and tint once `which` ("temperature" or
/// "tint") is put back to the camera's own, the other kept; none for
/// a name that is neither.
pub(crate) fn slider_reset(
    panel: (f32, f32),
    camera: (f32, f32),
    which: &str,
) -> Option<(f32, f32)> {
    match which {
        "temperature" => Some((camera.0, panel.1)),
        "tint" => Some((panel.0, camera.1)),
        _ => None,
    }
}

/// The panel's global white balance. On a `picture` the sliders are an
/// offset from its own white, mired and Duv, and none is As shot.
pub(crate) fn panel_white((as_shot, temperature, tint): WhiteKey, picture: bool) -> WhiteBalance {
    if as_shot {
        WhiteBalance::AsShot
    } else if picture {
        WhiteBalance::offset_of(temperature as f64, tint as f64)
    } else {
        WhiteBalance::Custom {
            temperature: temperature as f64,
            tint: tint as f64,
        }
    }
}

/// Whether the open file is a picture that is not a raw, by its name:
/// known the moment it is chosen, where what its decode says lands
/// later.
pub(crate) fn open_is_picture(st: &State) -> bool {
    st.current
        .and_then(|c| st.path_at(c))
        .is_some_and(|p| greycard_core::picture::is_picture_path(p))
}

/// Whether the open file is a monochrome sensor's, as far as the
/// session knows: its decode said so, now or on an earlier open. One
/// not yet decoded is taken for a color frame until its decode lands.
pub(crate) fn open_is_monochrome(st: &State) -> bool {
    st.current
        .and_then(|c| st.path_at(c))
        .is_some_and(|p| st.develop.monochromes.contains(p))
}

/// The open frame decoded (`st.develop.frame`): whether it is a
/// monochrome sensor's, kept for the session and put on the panel,
/// where the white balance rests on one (the window puts the white's
/// dropper down as it does). The edit's white is left as it is.
pub(crate) fn learn_monochrome(st: &mut State, app: &App) {
    let monochrome = st
        .develop
        .frame
        .as_ref()
        .is_some_and(|(f, _)| f.layout == greycard_core::raw::SensorLayout::Monochrome);
    if let Some(path) = st.current.and_then(|c| st.path_at(c)).cloned() {
        if monochrome {
            st.develop.monochromes.insert(path);
        } else {
            st.develop.monochromes.remove(&path);
        }
    }
    app.set_monochrome_open(monochrome);
}

/// Whether a change to the white is refused: on a monochrome frame no
/// white tints the picture, and the edit keeps the one it holds. One
/// that reached the panel anyway is taken back, the edit's whites shown
/// again, so nothing reads it into the edit.
pub(crate) fn white_refused(st: &State, app: &App) -> bool {
    if !app.get_monochrome_open() {
        return false;
    }
    let edit = &st.develop.edit;
    crate::panel::edit::show_white(st, edit, app);
    if st.develop.target.is_some() {
        let local = edit.target_look(st.develop.target).white_balance;
        crate::panel::edit::show_local_white(local, app);
    }
    true
}

/// The open picture's own white, the one its offset is from, once its
/// develop has said it; none for a raw.
pub(crate) fn picture_own(st: &State) -> Option<greycard_core::TempTint> {
    if !open_is_picture(st) {
        return None;
    }
    st.develop
        .base_white
        .and_then(|b| b.picture)
        .map(|(own, _)| own)
}

/// As shot's switch, the panel already showing where it went: on, the
/// sliders' custom white remembered for the open frame and the camera's
/// put on them; off, the remembered white given back, if it is this
/// frame's. A raw's camera white is a kelvin and Duv, a picture's the
/// offset of none; the rule is one.
fn as_shot_switched(st: &mut State, app: &App) {
    let key = st.current.and_then(|c| st.key_of(c));
    if app.get_as_shot() {
        st.develop.before_as_shot = key.map(|k| (k, (app.get_temperature(), app.get_tint())));
        if let Some((temperature, tint)) = st.develop.as_shot_white {
            app.set_temperature(temperature);
            app.set_tint(tint);
        }
    } else if let Some((owner, (temperature, tint))) = st.develop.before_as_shot.take()
        && key.as_ref() == Some(&owner)
    {
        app.set_temperature(temperature);
        app.set_tint(tint);
    }
}

/// The white As shot replaced, forgotten: the white was set some other
/// way (a slider, Auto, the Neutral dropper, an undo or redo, a reset, a
/// preset, a paste or a sync), so switching As shot off has nothing to
/// give back. Another frame opened forgets it too.
pub(crate) fn forget_before_as_shot(st: &mut State) {
    st.develop.before_as_shot = None;
}

/// The white balance presets on Auto's menu, in its order: the key the
/// menu passes back (its English words; the menu shows them
/// translated) and the light it names in the global sliders' own
/// terms, kelvin and Duv, the scale As shot reads a camera's gains
/// into. The daylights are the CIE D-series at those temperatures,
/// which sit about 0.003 above the Planckian locus; Tungsten is CIE
/// illuminant A (2856 K, on the locus); Fluorescent and Flash are
/// Adobe's presets, whose tint of +21 is 0.007 at the DNG SDK's 3000
/// tint units a unit of uv, and whose +10 on the daylights is the
/// same 0.003 the D-series has.
pub(crate) const WHITE_PRESETS: [(&str, f32, f32); 6] = [
    ("Daylight", 5500.0, 0.003),
    ("Cloudy", 6500.0, 0.003),
    ("Shade", 7500.0, 0.003),
    ("Tungsten", 2850.0, 0.0),
    ("Fluorescent", 3800.0, 0.007),
    ("Flash", 5500.0, 0.0),
];

/// A preset's temperature and tint, by its key.
pub(crate) fn white_preset(key: &str) -> Option<(f32, f32)> {
    WHITE_PRESETS
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|&(_, temperature, tint)| (temperature, tint))
}

pub(crate) fn install(app: &App, state: &Rc<Shared>, _worker: &Rc<Worker>) {
    // As shot switched on the panel: on, the custom white it replaces
    // is remembered and the sliders show the camera's white again; off,
    // the remembered white comes back, or with none the sliders stay
    // at the camera's. Either way the picture develops as a slider's
    // move does, one step. Moving a slider turns it off on the panel's
    // side, in the same change, so the move is one step, and gives
    // nothing back (`forget_before_as_shot`).
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_as_shot_toggled(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if white_refused(&state.borrow(), &app) {
                return;
            }
            as_shot_switched(&mut state.borrow_mut(), &app);
            app.invoke_develop_changed();
        });
    }
    // A preset from Auto's menu: the sliders at the light it names
    // and As shot off, in one change, so it is one step, as Auto's is.
    // A raw's only: a picture's sliders are an offset from its own
    // white, which no preset's light is read against.
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_wb_preset(move |key| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if app.get_picture_open() || white_refused(&state.borrow(), &app) {
                return;
            }
            let Some((temperature, tint)) = white_preset(key.as_str()) else {
                tracing::warn!("white balance: no preset {key:?}");
                return;
            };
            app.set_as_shot(false);
            app.set_temperature(temperature);
            app.set_tint(tint);
            app.invoke_stop_placing();
            app.invoke_develop_changed();
        });
    }
    // A double-click on Temperature or Tint: that slider alone back
    // to the camera's value, the other left where it stands, and
    // nothing else: As shot is left as it is, off after a move, even
    // when both sliders then stand at the camera's values. Turning As
    // shot on is what puts both back. One step, as a move is.
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_wb_slider_reset(move |which| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if white_refused(&state.borrow(), &app) {
                return;
            }
            let Some(camera) = state.borrow().develop.as_shot_white else {
                return;
            };
            let panel = (app.get_temperature(), app.get_tint());
            let Some((temperature, tint)) = slider_reset(panel, camera, which.as_str()) else {
                return;
            };
            app.set_temperature(temperature);
            app.set_tint(tint);
            app.invoke_develop_changed();
        });
    }
    // A mask's own white balance switched. Off, the sliders keep the
    // light they name, as every switched-off section keeps its values,
    // so the switch compares the two. On, that light comes back; with
    // none kept (a mask new, or reset) it starts where the global
    // stands, As shot read as the kelvin and Duv it means, so the
    // picture under the mask holds still until a slider moves.
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_local_wb_toggled(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if white_refused(&state.borrow(), &app) {
                return;
            }
            if app.get_local_wb_on() {
                if !app.get_local_wb_kept() {
                    start_at_global(&mut state.borrow_mut(), &app);
                }
                app.set_local_wb_kept(true);
            }
            app.invoke_view_changed();
        });
    }
    // A double-click on its Temperature or Tint: both are one light,
    // so both are forgotten, the switch goes off and the mask follows
    // the global; the sliders show the global's white, where the next
    // switch on starts, and Follow global goes off with the rest.
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_local_wb_reset(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if white_refused(&state.borrow(), &app) {
                return;
            }
            app.set_local_wb_on(false);
            app.set_local_wb_kept(false);
            app.set_local_wb_relative(false);
            start_at_global(&mut state.borrow_mut(), &app);
            app.invoke_view_changed();
        });
    }
    // Follow global switched: the sliders' numbers change meaning, so
    // they are turned into the other at the global's white as it
    // stands, the offset from it or the light it makes, and the
    // picture under the mask holds still. Without a global to take
    // them from (not a raw), the switch goes back.
    {
        let (state, app_weak) = (state.clone(), app.as_weak());
        app.on_local_wb_relative_toggled(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            if white_refused(&state.borrow(), &app) {
                return;
            }
            let relative = app.get_local_wb_relative();
            let Some(global) = panel_global(&mut state.borrow_mut(), &app) else {
                app.set_local_wb_relative(!relative);
                return;
            };
            let (temperature, tint) = (
                app.get_local_temperature() as f64,
                app.get_local_tint() as f64,
            );
            let moved = if relative {
                LocalWhite::Absolute { temperature, tint }.to_relative(global)
            } else {
                LocalWhite::Relative {
                    mired: temperature,
                    tint,
                }
                .to_absolute(global)
            };
            if let LocalWhite::Absolute { temperature, tint }
            | LocalWhite::Relative {
                mired: temperature,
                tint,
            } = moved.clamped()
            {
                app.set_local_temperature(temperature as f32);
                app.set_local_tint(tint as f32);
            }
            app.invoke_view_changed();
        });
    }
    // The color mixer: the sliders show the chosen band; a change on
    // them goes into that band's place in the arrays.
    {
        let app_weak = app.as_weak();
        app.on_mixer_band_changed(move || {
            if let Some(app) = app_weak.upgrade() {
                show_mixer_band(&app);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_mixer_changed(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            store_mixer_band(&app);
            app.invoke_view_changed();
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_mixer_reset(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let zero = [0.0; BANDS];
            app.set_mixer_hues(band_model(&zero));
            app.set_mixer_saturations(band_model(&zero));
            app.set_mixer_luminances(band_model(&zero));
            show_mixer_band(&app);
            app.invoke_view_changed();
        });
    }
    // Black and white: the swatch picks the band, the slider is that
    // band's weight, and a filter lays a whole set of weights over
    // them.
    {
        let app_weak = app.as_weak();
        app.on_bw_band_changed(move || {
            if let Some(app) = app_weak.upgrade() {
                show_bw_band(&app);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_bw_changed(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let mut weights = band_values(app.get_bw_weights());
            let band = (app.get_bw_band().max(0) as usize).min(BANDS - 1);
            weights[band] = app.get_bw_weight();
            app.set_bw_weights(band_model(&weights));
            show_bw_band(&app);
            app.invoke_view_changed();
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_bw_filter_changed(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let filter = greycard_edit::bw::Filter::from_name(app.get_bw_filter().as_str())
                .unwrap_or(greycard_edit::bw::Filter::None);
            app.set_bw_weights(band_model(&filter.weights()));
            // A filter is a choice of conversion, so it turns the
            // section on rather than sitting there doing nothing.
            app.set_bw_enabled(true);
            show_bw_band(&app);
            app.invoke_view_changed();
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_bw_reset(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            app.set_bw_weights(band_model(&[0.0; BANDS]));
            app.set_bw_strength(1.0);
            show_bw_band(&app);
            app.invoke_view_changed();
        });
    }
    // Color grading: the sliders act on the chosen wheel. A press or
    // a drag in a wheel chooses it and sets its hue and strength from
    // the pointer; release records the edit. The press takes the
    // wheel in hand. Shift keeps the wheel's hue and Ctrl its
    // strength for as long as either is held, taken up or let go of
    // at the press, at a move, or at the key itself (`grade-keys`).
    // One drag at a time, the grading's or the tint's.
    let drag: Rc<std::cell::Cell<Option<WheelDrag>>> = Rc::default();
    {
        let (state, app_weak, drag) = (state.clone(), app.as_weak(), drag.clone());
        app.on_grade_press(move |i, x, y, shift, control| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let w = (i.max(0) as usize).min(2) as i32;
            let mut d = WheelDrag::press(w, Hold::of(shift, control), hand_values(&app, w), (x, y));
            take_wheel(&mut state.borrow_mut(), &app, w);
            drag_to(&app, &mut d, (x, y));
            drag.set(Some(d));
        });
    }
    {
        let (app_weak, drag) = (app.as_weak(), drag.clone());
        app.on_grade_move(move |i, x, y, shift, control| {
            if let Some(app) = app_weak.upgrade() {
                let i = (i.max(0) as usize).min(2) as i32;
                drag_moved(&app, &drag, i, (x, y), Hold::of(shift, control));
            }
        });
    }
    {
        let (app_weak, drag) = (app.as_weak(), drag.clone());
        app.on_grade_keys(move |i, shift, control| {
            if let Some(app) = app_weak.upgrade() {
                let i = (i.max(0) as usize).min(2) as i32;
                drag_keys(&app, &drag, i, Hold::of(shift, control));
            }
        });
    }
    // A wheel's name: enlarging or shrinking it chooses it too, and
    // moves nothing in it, since nothing in it was touched.
    {
        let app_weak = app.as_weak();
        app.on_grade_chose(move |i| {
            if let Some(app) = app_weak.upgrade() {
                choose_grade_range(&app, (i.max(0) as usize).min(2));
            }
        });
    }
    {
        let (app_weak, drag) = (app.as_weak(), drag.clone());
        app.on_grade_release(move || {
            if let Some(app) = app_weak.upgrade() {
                drag_released(&app, &drag);
                app.invoke_view_changed();
            }
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_grade_changed(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let i = (app.get_grade_range().max(0) as usize).min(2);
            let mut hues = grade_values(app.get_grade_hues());
            let mut sats = grade_values(app.get_grade_saturations());
            hues[i] = app.get_grade_hue().rem_euclid(360.0);
            sats[i] = app.get_grade_saturation().clamp(0.0, 1.0);
            app.set_grade_hues(grade_model(&hues));
            app.set_grade_saturations(grade_model(&sats));
            draw_wheels(&app);
            app.invoke_view_changed();
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_grade_double(move |i| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let i = (i.max(0) as usize).min(2);
            let mut hues = grade_values(app.get_grade_hues());
            let mut sats = grade_values(app.get_grade_saturations());
            hues[i] = 0.0;
            sats[i] = 0.0;
            app.set_grade_hues(grade_model(&hues));
            app.set_grade_saturations(grade_model(&sats));
            choose_grade_range(&app, i);
            app.invoke_view_changed();
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_grade_reset(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            app.set_grade_hues(grade_model(&[0.0; 3]));
            app.set_grade_saturations(grade_model(&[0.0; 3]));
            app.set_grade_balance(0.0);
            show_grade_range(&app);
            draw_wheels(&app);
            app.invoke_view_changed();
        });
    }
    // The tint: a hue and an amount, off at nothing, the last thing
    // the Oklab pass does. The swatch follows the sliders.
    {
        let app_weak = app.as_weak();
        app.on_tint_changed(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            show_tint(&app);
            app.invoke_view_changed();
        });
    }
    // The ring: angle is the hue, distance from the center is the
    // amount, and a drag says both at once. The picture follows the
    // pointer; release is the one history entry the drag leaves, as
    // a grading wheel's is. A double click is Reset: off.
    // Shift and Ctrl hold the hue or the amount as the wheels' do.
    {
        let (state, app_weak, drag) = (state.clone(), app.as_weak(), drag.clone());
        app.on_tint_press(move |x, y, shift, control| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let mut d = WheelDrag::press(
                TINT_HAND,
                Hold::of(shift, control),
                hand_values(&app, TINT_HAND),
                (x, y),
            );
            take_wheel(&mut state.borrow_mut(), &app, TINT_HAND);
            drag_to(&app, &mut d, (x, y));
            drag.set(Some(d));
        });
    }
    {
        let (app_weak, drag) = (app.as_weak(), drag.clone());
        app.on_tint_move(move |x, y, shift, control| {
            if let Some(app) = app_weak.upgrade() {
                drag_moved(&app, &drag, TINT_HAND, (x, y), Hold::of(shift, control));
            }
        });
    }
    {
        let (app_weak, drag) = (app.as_weak(), drag.clone());
        app.on_tint_keys(move |shift, control| {
            if let Some(app) = app_weak.upgrade() {
                drag_keys(&app, &drag, TINT_HAND, Hold::of(shift, control));
            }
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_tint_release(move || {
            if let Some(app) = app_weak.upgrade() {
                drag_released(&app, &drag);
                app.invoke_view_changed();
            }
        });
    }
    {
        let app_weak = app.as_weak();
        app.on_tint_reset(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            app.set_tint_hue(0.0);
            app.set_tint_amount(0.0);
            show_tint(&app);
            app.invoke_view_changed();
        });
    }
}

#[cfg(test)]
mod tests {
    use greycard_edit::Edit;

    use super::*;
    use crate::worker::{Outcome, WhiteBase};

    #[test]
    fn a_hue_circle_is_painted_the_colors_its_slider_track_carries() {
        // The `hue-circle` gradient in `app.slint` is Oklab at this
        // lightness and this chroma, a stop every thirty degrees. The
        // ring's rim and the Hue slider's track have to name the same
        // color at a hue or the two controls disagree by eye.
        let at = |hue: f32| {
            let (s, c) = hue.to_radians().sin_cos();
            finish::oklab_to_srgb([CIRCLE_LIGHTNESS, CIRCLE_CHROMA * c, CIRCLE_CHROMA * s])
                .map(|v| (v * 255.0).round() as u8)
        };
        assert_eq!(at(0.0), [0xe6, 0x80, 0xa1]);
        assert_eq!(at(90.0), [0xc4, 0xa0, 0x32]);
        assert_eq!(at(210.0), [0x00, 0xba, 0xd1]);
        assert_eq!(at(330.0), [0xd2, 0x85, 0xcb]);
    }

    #[test]
    fn the_preview_is_the_identity_at_the_base_white_point() {
        // A base and a target with the same gains and matrix: the
        // preview must move nothing, whatever the matrix.
        let base = WhiteBase {
            gains: [1.9, 1.0, 1.7],
            matrix: [[1.6, -0.4, -0.2], [-0.1, 1.3, -0.2], [0.0, -0.3, 1.3]],
            clip: 1.0,
            temp_tint: None,
            picture: None,
        };
        let p = greycard_core::color::white_shift(base.gains, base.matrix, base.gains, base.matrix)
            .unwrap();
        for (r, row) in p.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-5, "{p:?}");
            }
        }
    }

    /// The center of whatever is labeled `label`, scrolled into view.
    fn center(app: &App, label: &str) -> (f32, f32) {
        let (at, size) = crate::testing::labeled(app, label);
        (at.x + size.width / 2.0, at.y + size.height / 2.0)
    }

    /// The grading the panel holds, wheel by wheel.
    fn wheels(app: &App) -> [Wheel; 3] {
        let g = read_grading(app);
        Range::ALL.map(|r| *g.wheel(r))
    }

    /// The sliders' two numbers.
    fn sliders(app: &App) -> Wheel {
        Wheel {
            hue: app.get_grade_hue(),
            saturation: app.get_grade_saturation(),
        }
    }

    /// A drag across the wheel labeled `label`, from its center out to
    /// the right and up: a hue a little over 0 and some strength.
    fn drag_wheel(app: &App, label: &str, up: f32) {
        let (x, y) = center(app, label);
        crate::testing::drag(app, &[(x, y), (x + 10.0, y), (x + 25.0, y - up)]);
    }

    /// The sliders follow the wheel chosen, not the wheel last
    /// dragged: a click in a wheel chooses it and puts its point at
    /// the click, a click on its name chooses it and moves nothing,
    /// and either way its values go on the sliders; a
    /// slider then moves that wheel and no other; the ring and the
    /// name's accent follow `grade-range`. Global and a mask's alike.
    fn the_sliders_follow_the_chosen_wheel(app: &App, state: &Rc<Shared>, prefix: &str) {
        use crate::testing::click;
        // The edit as a save reads it off the panel, and the grading
        // the panel is on in it.
        let edit = || {
            let st = state.borrow();
            crate::panel::edit::read_edit(app, &st.develop.edit, st.develop.target)
        };
        let grading = || edit().target_look(state.borrow().develop.target).grading;

        // Three wheels with three different points, each dragged, the
        // highlights last.
        drag_wheel(app, "Shadows wheel", 5.0);
        drag_wheel(app, "Midtones wheel", 15.0);
        drag_wheel(app, "Highlights wheel", 25.0);
        let before = wheels(app);
        assert_eq!(app.get_grade_range(), 2);
        assert!(
            before[0] != before[1] && before[1] != before[2],
            "{before:?}"
        );
        assert_eq!(sliders(app), before[2]);

        // A click in the shadows wheel, up and to the right of its
        // center: chosen, its point at the click (a hue of 45 degrees),
        // the sliders on its new values, the others unmoved.
        let (x, y) = center(app, "Shadows wheel");
        click(app, x + 20.0, y - 20.0);
        assert_eq!(app.get_grade_range(), 0, "the marker follows the click");
        let clicked = wheels(app);
        assert!((clicked[0].hue - 45.0).abs() < 2.0, "{clicked:?}");
        assert!(clicked[0].saturation > 0.0, "{clicked:?}");
        assert_ne!(clicked[0], before[0], "the click moved its point");
        assert_eq!((clicked[1], clicked[2]), (before[1], before[2]));
        assert_eq!(sliders(app), clicked[0]);

        // A slider moves the shadows, not the highlights last dragged,
        // and the history row says so.
        let was = edit();
        app.set_grade_hue(123.0);
        app.invoke_grade_changed();
        let after = wheels(app);
        assert_eq!(after[0].hue, 123.0);
        assert_eq!(after[0].saturation, clicked[0].saturation);
        assert_eq!((after[1], after[2]), (before[1], before[2]));
        assert_eq!(grading().shadows.hue, 123.0, "the edit has it");
        assert_eq!(
            greycard_edit::history::describe(&was, &edit(), &[]),
            format!("{prefix}Color grading shadows 123\u{b0}")
        );

        // Enlarging the mid-tones chooses them too.
        let (x, y) = center(app, "Enlarge Midtones");
        click(app, x, y);
        // The section grows to the enlarged wheel over its 120ms
        // animation, which the mock clock only runs when told.
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(150));
        assert_eq!(app.get_grade_big(), 1);
        assert_eq!(app.get_grade_range(), 1, "the marker follows the enlarging");
        assert_eq!(sliders(app), before[1]);
        assert_eq!(crate::testing::count_labeled(app, "Enlarged wheel"), 1);
        app.set_grade_saturation(0.9);
        app.invoke_grade_changed();
        let enlarged = wheels(app);
        assert_eq!(enlarged[1].saturation, 0.9);
        assert_eq!(enlarged[1].hue, before[1].hue);
        assert_eq!((enlarged[0], enlarged[2]), (after[0], after[2]));
        assert_eq!(grading().midtones.saturation, 0.9);

        // A click in another wheel, the mid-tones still enlarged,
        // left of its center: chosen, its point at the click (a hue of
        // 180 degrees); the enlarged one keeps its size and its point
        // and loses the mark.
        let (x, y) = center(app, "Highlights wheel");
        click(app, x - 20.0, y);
        assert_eq!(app.get_grade_range(), 2);
        assert_eq!(app.get_grade_big(), 1);
        let pressed = wheels(app);
        assert!((pressed[2].hue - 180.0).abs() < 2.0, "{pressed:?}");
        assert_ne!(pressed[2], enlarged[2]);
        assert_eq!((pressed[0], pressed[1]), (enlarged[0], enlarged[1]));
        assert_eq!(sliders(app), pressed[2]);

        // Shrinking it chooses it again.
        let (x, y) = center(app, "Shrink Midtones");
        click(app, x, y);
        assert_eq!(app.get_grade_big(), -1);
        assert_eq!(app.get_grade_range(), 1);
        assert_eq!(sliders(app), enlarged[1]);
    }

    #[test]
    fn the_global_gradings_sliders_follow_the_chosen_wheel() {
        use crate::testing;
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        the_sliders_follow_the_chosen_wheel(&app, &state, "");
    }

    #[test]
    fn a_masks_gradings_sliders_follow_the_chosen_wheel() {
        use crate::testing;
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Masks".into());
        app.invoke_new_mask();
        assert_eq!(app.get_target(), 1);
        let name = app.get_adjustment_name().to_string();
        the_sliders_follow_the_chosen_wheel(&app, &state, &format!("{name}: "));
        let st = state.borrow();
        let edit = crate::panel::edit::read_edit(&app, &st.develop.edit, st.develop.target);
        assert_eq!(
            edit.grading,
            Grading::default(),
            "the global's wheels untouched"
        );
    }

    /// The tint's switch, on a target: off, the hue and the amount
    /// stay on the panel and in the edit while the tint does nothing;
    /// on again, the same tint. Each flip is a history row of its own,
    /// it survives a trip through `show_edit`, and it touches no other
    /// target's tint.
    fn the_tint_switches_and_keeps_its_values(
        app: &App,
        state: &Rc<Shared>,
        prefix: &str,
        tint_of: impl Fn(&Edit) -> Tint,
        other_of: impl Fn(&Edit) -> Tint,
    ) {
        use greycard_edit::history::describe;
        let edit = || {
            let st = state.borrow();
            crate::panel::edit::read_edit(app, &st.develop.edit, st.develop.target)
        };
        assert!(app.get_tint_enabled(), "a new tint is on");
        app.set_tint_hue(210.0);
        app.set_tint_amount(0.4);
        app.invoke_tint_changed();
        let on = edit();
        let tinted = Tint::new(210.0, 0.4);
        assert_eq!(tint_of(&on), tinted);
        let swatch = app.get_tint_color();
        assert_ne!(swatch, slint::Color::from_rgb_u8(0x8c, 0x8c, 0x8c));

        // Off: the values kept, the vector the shader is handed zero.
        app.set_tint_enabled(false);
        app.invoke_view_changed();
        let off = edit();
        let switched_off = Tint {
            enabled: false,
            ..tinted
        };
        assert_eq!(tint_of(&off), switched_off);
        assert_eq!(tint_of(&off).vector(), [0.0, 0.0]);
        assert_eq!(other_of(&off), Tint::OFF, "the other target's untouched");
        assert_eq!((app.get_tint_hue(), app.get_tint_amount()), (210.0, 0.4));
        assert_eq!(app.get_tint_color(), swatch, "the swatch keeps the tint");
        assert_eq!(describe(&on, &off, &[]), format!("{prefix}Color tint off"));

        // Through `show_edit`: the panel on, then shown the edit, off
        // again with its values, and read back the same.
        app.set_tint_enabled(true);
        app.set_tint_amount(0.0);
        {
            let st = state.borrow();
            crate::panel::edit::show_edit(&st, &off, app, st.develop.target);
        }
        assert!(!app.get_tint_enabled());
        assert_eq!((app.get_tint_hue(), app.get_tint_amount()), (210.0, 0.4));
        assert_eq!(app.get_tint_color(), swatch);
        assert_eq!(edit(), off);

        // On again: the same tint, and its own row.
        app.set_tint_enabled(true);
        app.invoke_view_changed();
        let again = edit();
        assert_eq!(tint_of(&again), tinted);
        assert_eq!(
            describe(&off, &again, &[]),
            format!("{prefix}Color tint on")
        );
    }

    /// Shift held at the press keeps a wheel's hue for the drag and
    /// moves its strength; Ctrl keeps the strength and moves the hue:
    /// each against a plain drag on the same path from the same point,
    /// in a grading wheel and in the tint's ring.
    #[test]
    fn shift_holds_a_wheels_hue_and_ctrl_its_strength() {
        use crate::testing;
        use slint::platform::{Key, WindowEvent};
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 3000.0));
        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        let key = |k: Key, down: bool| {
            let text: slint::SharedString = k.into();
            app.window().dispatch_event(if down {
                WindowEvent::KeyPressed { text }
            } else {
                WindowEvent::KeyReleased { text }
            });
        };
        let start = (200.0, 0.3);
        // A drag on `path` from `start`, `held` down through it, and
        // the wheel's hue and strength after.
        let dragged = |path: &[(f32, f32)],
                       held: Option<Key>,
                       set: &dyn Fn(),
                       read: &dyn Fn() -> (f32, f32)| {
            set();
            // Apart from the last drag, so its press is no double click.
            i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
            if let Some(k) = held {
                key(k, true);
            }
            testing::drag(&app, path);
            if let Some(k) = held {
                key(k, false);
            }
            read()
        };
        // `past`: the path ends on the far side of the center from the
        // held hue, where Shift reads nothing; otherwise it ends near the
        // held hue's ray, where it reads a little under the radius.
        let check =
            |path: &[(f32, f32)], past: bool, set: &dyn Fn(), read: &dyn Fn() -> (f32, f32)| {
                let plain = dragged(path, None, set, read);
                assert!(
                    plain.0 != start.0 && plain.1 != start.1,
                    "a plain drag moves both: {plain:?}"
                );
                let shift = dragged(path, Some(Key::Shift), set, read);
                assert_eq!(shift.0, start.0, "Shift holds the hue");
                if past {
                    assert_eq!(shift.1, 0.0, "past the center is nothing: {shift:?}");
                } else {
                    assert!(
                        shift.1 > 0.0 && shift.1 < plain.1,
                        "along the held hue: {shift:?} for {plain:?}"
                    );
                }
                let ctrl = dragged(path, Some(Key::Control), set, read);
                assert_eq!(ctrl, (plain.0, start.1), "Ctrl holds the strength");
            };

        // Scrolled into view, and measured where the scroll leaves it.
        center(&app, "Shadows wheel");
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
        let (x, y) = center(&app, "Shadows wheel");
        check(
            &[
                (x + 20.0, y - 5.0),
                (x + 25.0, y - 10.0),
                (x + 25.0, y - 15.0),
            ],
            true,
            &|| {
                app.set_grade_hues(grade_model(&[start.0; 3]));
                app.set_grade_saturations(grade_model(&[start.1; 3]));
            },
            &|| (wheels(&app)[0].hue, wheels(&app)[0].saturation),
        );
        assert_eq!(app.get_wheel_hand(), 0, "the wheel pressed is in hand");

        let ring =
            i_slint_backend_testing::ElementHandle::find_by_element_type_name(&app, "CurveEditor")
                .find(|e| e.size().width == 176.0)
                .expect("the tint ring is on screen");
        let (at, size) = (ring.absolute_position(), ring.size());
        let (x, y) = (at.x + size.width / 2.0, at.y + size.height / 2.0);
        check(
            &[
                (x - 30.0, y + 10.0),
                (x - 40.0, y + 20.0),
                (x - 50.0, y + 25.0),
            ],
            false,
            &|| {
                app.set_tint_hue(start.0);
                app.set_tint_amount(start.1);
            },
            &|| (app.get_tint_hue(), app.get_tint_amount()),
        );
        assert_eq!(app.get_wheel_hand(), TINT_HAND);
    }

    #[test]
    fn a_held_hue_reads_its_amount_along_its_ray() {
        use crate::wheel::DEAD;
        let from = (90.0, 0.7);
        // Up, the held hue's way: the amount the radius gives.
        let (hue, amount) = held_pick(
            0.5,
            0.5 + 0.5 * (DEAD + 0.5 * (1.0 - DEAD)),
            Hold::Hue,
            from,
        );
        assert_eq!(hue, 90.0);
        assert!((amount - 0.5).abs() < 1e-5, "{amount}");
        // Down, past the center: nothing, where a free drag reads half.
        let (x, y) = (0.5, 0.5 - 0.5 * (DEAD + 0.5 * (1.0 - DEAD)));
        assert_eq!(held_pick(x, y, Hold::Hue, from), (90.0, 0.0));
        assert!((held_pick(x, y, Hold::Free, from).1 - 0.5).abs() < 1e-5);
    }

    /// Where the window's keys never hear a key (another control has
    /// them), the next move carries it: Shift down mid-drag holds the
    /// hue the wheel had when the move came, and up frees it.
    #[test]
    fn a_move_carries_the_keys_into_the_drag() {
        use crate::testing;
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 3000.0));
        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        // The keys' own path cut, so only a move can tell.
        app.on_grade_keys(|_, _, _| {});
        let key = |k: Key, down: bool| {
            let text: slint::SharedString = k.into();
            app.window().dispatch_event(if down {
                WindowEvent::KeyPressed { text }
            } else {
                WindowEvent::KeyReleased { text }
            });
        };
        let at = |p: (f32, f32)| slint::LogicalPosition::new(p.0, p.1);
        let to = |p: (f32, f32)| {
            app.window()
                .dispatch_event(WindowEvent::PointerMoved { position: at(p) })
        };
        let read = || (wheels(&app)[0].hue, wheels(&app)[0].saturation);
        center(&app, "Shadows wheel");
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
        let (cx, cy) = center(&app, "Shadows wheel");
        to((cx + 25.0, cy));
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: at((cx + 25.0, cy)),
            button: PointerEventButton::Left,
        });
        to((cx + 20.0, cy - 20.0));
        let before = read();
        key(Key::Shift, true);
        assert_eq!(app.get_wheel_guide().kind, 0, "not heard until a move");
        to((cx - 5.0, cy - 30.0));
        assert_eq!(app.get_wheel_guide().kind, 1);
        assert_eq!(read().0, before.0, "the hue the wheel had at the move");
        key(Key::Shift, false);
        to((cx - 6.0, cy - 30.0));
        assert_eq!(app.get_wheel_guide().kind, 0);
        assert!(read().0 > before.0 + 40.0, "free again: {:?}", read());
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: at((cx - 6.0, cy - 30.0)),
            button: PointerEventButton::Left,
        });
    }

    /// A window on the Develop tab over two frames, a drag begun in the
    /// Shadows wheel and still held, and the pointer's way to move it.
    fn mid_drag() -> (App, Rc<Shared>, Rc<Worker>, (f32, f32)) {
        use crate::testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        let app = testing::window(2);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 3000.0));
        let (state, worker) = testing::state_for(&app, testing::folder(2));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        center(&app, "Shadows wheel");
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
        let (cx, cy) = center(&app, "Shadows wheel");
        let at = slint::LogicalPosition::new(cx + 25.0, cy);
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position: at });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: at,
            button: PointerEventButton::Left,
        });
        drag_to_point(&app, (cx + 20.0, cy - 20.0));
        assert_eq!(app.get_wheel_hand(), 0);
        (app, state, worker, (cx, cy))
    }

    fn drag_to_point(app: &App, (x, y): (f32, f32)) {
        app.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(x, y),
            });
    }

    fn key_event(app: &App, k: slint::platform::Key, down: bool) {
        use slint::platform::WindowEvent;
        let text: slint::SharedString = k.into();
        app.window().dispatch_event(if down {
            WindowEvent::KeyPressed { text }
        } else {
            WindowEvent::KeyReleased { text }
        });
    }

    fn release_at(app: &App, (x, y): (f32, f32)) {
        app.window()
            .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                position: slint::LogicalPosition::new(x, y),
                button: slint::platform::PointerEventButton::Left,
            });
    }

    /// After a drag whose wheel went away under it: the wheels set
    /// anew, and Shift or Ctrl alone, down and up, must leave them be
    /// and show no guide.
    fn keys_leave_the_wheels_alone(app: &App) {
        use slint::platform::Key;
        app.set_grade_hues(grade_model(&[200.0; 3]));
        app.set_grade_saturations(grade_model(&[0.1; 3]));
        for k in [Key::Control, Key::Shift] {
            key_event(app, k, true);
            assert_eq!(app.get_wheel_guide().kind, 0, "{k:?}: no guide");
            assert_eq!(
                wheels(app)[0],
                Wheel {
                    hue: 200.0,
                    saturation: 0.1
                },
                "{k:?} down"
            );
            key_event(app, k, false);
            assert_eq!(
                wheels(app)[0],
                Wheel {
                    hue: 200.0,
                    saturation: 0.1
                },
                "{k:?} up"
            );
        }
    }

    /// The reviewer's case: C to the Cull tab mid-drag takes the wheel
    /// away and its release never comes; back on Develop, a key alone
    /// must not take up the dead drag.
    #[test]
    fn a_drag_whose_wheel_went_to_cull_hears_no_keys() {
        let (app, _state, _worker, (cx, cy)) = mid_drag();
        crate::testing::press(&app, "c");
        assert_eq!(app.get_panel_tab(), crate::panel::cull::CULL_TAB);
        release_at(&app, (cx, cy));
        crate::testing::press(&app, "d");
        assert_eq!(app.get_panel_tab(), "Develop");
        keys_leave_the_wheels_alone(&app);
    }

    #[test]
    fn a_drag_whose_wheel_went_with_the_tab_hears_no_keys() {
        let (app, _state, _worker, (cx, cy)) = mid_drag();
        app.set_panel_tab("Crop".into());
        release_at(&app, (cx, cy));
        app.set_panel_tab("Develop".into());
        keys_leave_the_wheels_alone(&app);
    }

    /// Enlarging the wheel mid-drag swaps its small one for the big:
    /// the drag was the small one's.
    #[test]
    fn a_drag_whose_wheel_was_enlarged_hears_no_keys() {
        let (app, _state, _worker, (cx, cy)) = mid_drag();
        app.set_grade_big(0);
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
        release_at(&app, (cx, cy));
        keys_leave_the_wheels_alone(&app);
    }

    /// Another frame opened mid-drag: the drag and its hold end there;
    /// the pointer and the keys move nothing on the new frame.
    #[test]
    fn a_frame_change_ends_the_drag() {
        use slint::platform::Key;
        let (app, _state, _worker, (cx, cy)) = mid_drag();
        key_event(&app, Key::Shift, true);
        assert_eq!(app.get_wheel_guide().kind, 1);
        app.invoke_select(1);
        let fresh = wheels(&app);
        drag_to_point(&app, (cx - 20.0, cy + 10.0));
        assert_eq!(wheels(&app), fresh, "the move is the dead drag's");
        assert_eq!(app.get_wheel_guide().kind, 0, "no guide");
        key_event(&app, Key::Shift, false);
        key_event(&app, Key::Control, true);
        assert_eq!(wheels(&app), fresh);
        assert_eq!(app.get_wheel_guide().kind, 0);
        key_event(&app, Key::Control, false);
        release_at(&app, (cx - 20.0, cy + 10.0));
    }

    /// Esc puts the wheel down mid-drag, as it does a wheel at rest:
    /// the drag and its hold end with it, the guide gone; the pointer
    /// and the keys move nothing after.
    #[test]
    fn esc_ends_a_held_drag() {
        use slint::platform::Key;
        let (app, _state, _worker, (cx, cy)) = mid_drag();
        key_event(&app, Key::Shift, true);
        assert_eq!(app.get_wheel_guide().kind, 1);
        crate::testing::press(&app, Key::Escape);
        assert_eq!(app.get_wheel_hand(), -1);
        assert_eq!(
            app.get_wheel_guide().kind,
            0,
            "the guide went with the hold"
        );
        let put_down = wheels(&app);
        drag_to_point(&app, (cx - 20.0, cy + 10.0));
        key_event(&app, Key::Shift, false);
        key_event(&app, Key::Control, true);
        assert_eq!(wheels(&app), put_down);
        assert_eq!(app.get_wheel_guide().kind, 0);
        key_event(&app, Key::Control, false);
        release_at(&app, (cx - 20.0, cy + 10.0));
    }

    #[test]
    fn shift_wins_over_ctrl() {
        assert_eq!(Hold::of(false, false), Hold::Free);
        assert_eq!(Hold::of(true, false), Hold::Hue);
        assert_eq!(Hold::of(false, true), Hold::Amount);
        assert_eq!(Hold::of(true, true), Hold::Hue);
    }

    /// A hold taken up mid-drag keeps what the wheel has then, not
    /// what it had at the press; let go of, the point is the
    /// pointer's again; one key for the other starts from where the
    /// first left the wheel.
    #[test]
    fn a_hold_begins_and_ends_where_the_drag_is() {
        use crate::wheel::place;
        let mut d = WheelDrag::press(0, Hold::Free, (200.0, 0.3), place(0.0, 0.8));
        let mut now = d.point(place(0.0, 0.8), (200.0, 0.3));
        assert!(now.0.abs() < 0.01 && (now.1 - 0.8).abs() < 1e-4, "{now:?}");
        now = d.point(place(60.0, 0.5), now);
        // Shift down: the hue it has now, 60, kept.
        assert!(d.keys(Hold::Hue, now));
        assert!(!d.keys(Hold::Hue, now), "the same keys change nothing");
        let held = d.point(place(150.0, 0.9), now);
        assert_eq!(held.0, now.0, "the hue at the key, not the press's");
        assert!(held.1 < now.1, "150 is past square to 60: {held:?}");
        // Shift up: the pointer's own.
        assert!(d.keys(Hold::Free, held));
        let free = d.point(d.at(), held);
        assert!((free.0 - 150.0).abs() < 0.01 && (free.1 - 0.9).abs() < 1e-4);
        // Ctrl down: the amount it has now, 0.9, kept.
        assert!(d.keys(Hold::Amount, free));
        let held = d.point(place(300.0, 0.2), free);
        assert!((held.0 - 300.0).abs() < 0.01);
        assert_eq!(held.1, free.1);
        // Shift with it: the hue it has now, kept, and the amount free.
        assert!(d.keys(Hold::of(true, true), held));
        let both = d.point(place(300.0, 0.4), held);
        assert_eq!(both.0, held.0);
        assert!((both.1 - 0.4).abs() < 1e-4);
    }

    #[test]
    fn the_guide_is_the_held_hues_line_or_the_held_amounts_circle() {
        let n = WHEEL_SMALL;
        let rim = 0.5 - 1.0 / n as f32;
        let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).hypot(a.1 - b.1);
        // The marker's center, in the picture, y down.
        let marker = |hue: f32, amount: f32| {
            let (x, y) = wheel::place(hue, amount);
            (0.5 + (2.0 * x - 1.0) * rim, 0.5 - (2.0 * y - 1.0) * rim)
        };
        assert_eq!(
            wheel_guide(Hold::Free, (120.0, 0.5), n),
            crate::WheelGuide::default()
        );

        // Shift: center to rim along 90 degrees, up the picture, broken
        // at the marker by its rim.
        let g = wheel_guide(Hold::Hue, (90.0, 0.5), n);
        let m = marker(90.0, 0.5);
        assert_eq!(g.kind, 1);
        assert_eq!((g.x0, g.y0), (0.5, 0.5));
        assert!(near((g.x3, g.y3), (0.5, 0.5 - rim)) < 1e-5);
        for p in [(g.x1, g.y1), (g.x2, g.y2)] {
            assert!((p.0 - 0.5).abs() < 1e-5, "on the ray: {g:?}");
            assert!((near(p, m) - MARKER_RIM).abs() < 1e-5, "{g:?}");
        }
        assert!(g.y1 > m.1 && m.1 > g.y2, "the marker between: {g:?}");
        // At nothing the marker covers the center: no inner stroke.
        let g = wheel_guide(Hold::Hue, (0.0, 0.0), n);
        assert_eq!((g.x0, g.y0), (g.x1, g.y1));

        // Ctrl: the circle through the marker, its gap the marker's.
        let g = wheel_guide(Hold::Amount, (210.0, 0.6), n);
        let m = marker(210.0, 0.6);
        assert_eq!(g.kind, 2);
        assert!((g.r - near(m, (0.5, 0.5))).abs() < 1e-5, "{g:?}");
        for p in [(g.x0, g.y0), (g.x1, g.y1)] {
            assert!((near(p, (0.5, 0.5)) - g.r).abs() < 1e-5, "{g:?}");
            assert!((near(p, m) - MARKER_RIM).abs() < 1e-5, "{g:?}");
        }
        // At nothing the marker covers the whole circle: none drawn.
        assert_eq!(wheel_guide(Hold::Amount, (210.0, 0.0), n).r, 0.0);
    }

    /// Shift or Ctrl pressed in the middle of a drag holds what the
    /// wheel has then, at once, without the pointer moving, with the
    /// guide on; let go, the point is the pointer's again at once and
    /// the guide off; Shift wins over Ctrl; the drag's release takes
    /// the guide down whatever is held. A grading wheel and the
    /// tint's ring alike.
    #[test]
    fn a_key_held_mid_drag_holds_the_wheel_from_there() {
        use crate::testing;
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 3000.0));
        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        let key = |k: Key, down: bool| {
            let text: slint::SharedString = k.into();
            app.window().dispatch_event(if down {
                WindowEvent::KeyPressed { text }
            } else {
                WindowEvent::KeyReleased { text }
            });
        };
        let at = |p: (f32, f32)| slint::LogicalPosition::new(p.0, p.1);
        let to = |p: (f32, f32)| {
            app.window()
                .dispatch_event(WindowEvent::PointerMoved { position: at(p) })
        };
        let kind = || app.get_wheel_guide().kind;
        // The angle of the pointer `(dx, dy)` from a wheel's center,
        // y down, as a hue.
        let angle = |dx: f32, dy: f32| (-dy).atan2(dx).to_degrees().rem_euclid(360.0);
        let apart = |a: f32, b: f32| (a - b).abs().min(360.0 - (a - b).abs());

        let check = |(cx, cy): (f32, f32), read: &dyn Fn() -> (f32, f32)| {
            i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
            let p = (cx + 25.0, cy);
            to(p);
            app.window().dispatch_event(WindowEvent::PointerPressed {
                position: at(p),
                button: PointerEventButton::Left,
            });
            assert_eq!(kind(), 0, "a free drag shows no guide");
            to((cx + 20.0, cy - 20.0));
            let before = read();
            assert!(apart(before.0, 45.0) < 2.0, "{before:?}");

            // Shift down, the pointer still: the guide on, nothing moved.
            key(Key::Shift, true);
            assert_eq!(kind(), 1, "Shift's line, at once");
            let held = read();
            assert_eq!(held.0, before.0);
            assert!((held.1 - before.1).abs() < 1e-4, "{held:?} {before:?}");
            // A move: the hue at the key, not the press's (0).
            to((cx - 5.0, cy - 30.0));
            assert_eq!(read().0, before.0, "the hue Shift found");
            assert_eq!(kind(), 1);

            // Shift up, the pointer still: free at once, to the pointer.
            key(Key::Shift, false);
            assert_eq!(kind(), 0, "the guide off");
            let free = read();
            assert!(apart(free.0, angle(-5.0, -30.0)) < 2.0, "{free:?}");

            // Ctrl down: the amount it has now, kept.
            key(Key::Control, true);
            assert_eq!(kind(), 2, "Ctrl's circle, at once");
            to((cx + 30.0, cy + 5.0));
            let ctrl = read();
            assert_eq!(ctrl.1, free.1, "the amount Ctrl found");
            assert!(apart(ctrl.0, angle(30.0, 5.0)) < 2.0, "{ctrl:?}");

            // Shift with it: Shift's, from the hue there.
            key(Key::Shift, true);
            assert_eq!(kind(), 1, "Shift wins");
            to((cx - 20.0, cy - 20.0));
            assert_eq!(read().0, ctrl.0, "the hue Shift found");

            // The drag released with both held: the guide goes with it.
            app.window().dispatch_event(WindowEvent::PointerReleased {
                position: at((cx - 20.0, cy - 20.0)),
                button: PointerEventButton::Left,
            });
            assert_eq!(kind(), 0, "no drag, no guide");
            let after = read();
            key(Key::Shift, false);
            key(Key::Control, false);
            assert_eq!(kind(), 0);
            assert_eq!(read(), after, "keys outside a drag move nothing");
        };

        // Scrolled into view, and measured where the scroll leaves it.
        center(&app, "Shadows wheel");
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(1));
        check(center(&app, "Shadows wheel"), &|| {
            (wheels(&app)[0].hue, wheels(&app)[0].saturation)
        });
        assert_eq!(app.get_wheel_hand(), 0);

        let ring =
            i_slint_backend_testing::ElementHandle::find_by_element_type_name(&app, "CurveEditor")
                .find(|e| e.size().width == 176.0)
                .expect("the tint ring is on screen");
        let (pos, size) = (ring.absolute_position(), ring.size());
        check(
            (pos.x + size.width / 2.0, pos.y + size.height / 2.0),
            &|| (app.get_tint_hue(), app.get_tint_amount()),
        );
        assert_eq!(app.get_wheel_hand(), TINT_HAND);
    }

    /// A pick from the picture onto a wheel at nothing gives it an
    /// amount from the color's chroma, a grey none; onto a wheel with
    /// an amount it keeps that. One history step each.
    #[test]
    fn a_pick_onto_an_empty_wheel_sets_its_amount() {
        use crate::testing;
        let app = testing::window(1);
        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        // Past the save's wait, so each change is its own step.
        let rest = || {
            i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(
                crate::panel::edit::SAVE_MS + 100,
            ));
            slint::platform::update_timers_and_animations();
        };
        let steps = || {
            rest();
            app.get_history_rows().row_count()
        };
        let tint = || (app.get_tint_hue(), app.get_tint_amount());
        let mid = || {
            let g = read_grading(&app);
            (g.midtones.hue, g.midtones.saturation)
        };

        // The tint: at nothing, a saturated pick sets an amount.
        app.set_tint_amount(0.0);
        let was = steps();
        set_hand_hue(&app, TINT_HAND, 40.0, 0.06);
        assert_eq!(tint().0, 40.0);
        assert!((tint().1 - 0.6).abs() < 1e-6, "{:?}", tint());
        assert_eq!(steps(), was + 1, "one step");
        // At an amount, the amount kept.
        app.set_tint_amount(0.4);
        set_hand_hue(&app, TINT_HAND, 120.0, 0.25);
        assert_eq!(tint(), (120.0, 0.4));
        // A grey onto nothing stays at nothing.
        app.set_tint_amount(0.0);
        set_hand_hue(&app, TINT_HAND, 200.0, 0.0);
        assert_eq!(tint(), (200.0, 0.0));

        // A grading wheel likewise, the others untouched; a chroma past
        // the reference is the whole amount.
        let was = steps();
        set_hand_hue(&app, 1, 300.0, 0.3);
        assert_eq!(mid(), (300.0, 1.0));
        assert_eq!(steps(), was + 1, "one step");
        let g = read_grading(&app);
        assert_eq!((g.shadows.saturation, g.highlights.saturation), (0.0, 0.0));
        app.set_grade_saturations(grade_model(&[0.0, 0.4, 0.0]));
        set_hand_hue(&app, 1, 10.0, 0.08);
        assert_eq!(mid(), (10.0, 0.4));
        app.set_grade_saturations(grade_model(&[0.0; 3]));
        set_hand_hue(&app, 1, 20.0, 0.0);
        assert_eq!(mid(), (20.0, 0.0));
    }

    /// A release with no press on the ring, as the click that closes a
    /// menu brings, is no end of a drag: it must not re-render.
    #[test]
    fn a_lone_release_on_the_tint_ring_is_nothing() {
        use crate::testing;
        use slint::platform::WindowEvent;
        let app = testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 3000.0));
        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        let releases = Rc::new(std::cell::Cell::new(0));
        {
            let releases = releases.clone();
            app.on_tint_release(move || releases.set(releases.get() + 1));
        }
        let ring =
            i_slint_backend_testing::ElementHandle::find_by_element_type_name(&app, "CurveEditor")
                .find(|e| e.size().width == 176.0)
                .expect("the tint ring is on screen");
        let (at, size) = (ring.absolute_position(), ring.size());
        let (x, y) = (at.x + size.width / 2.0, at.y + size.height / 2.0);
        let position = slint::LogicalPosition::new(x, y);
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
        assert_eq!(releases.get(), 0, "no press, no release");
        testing::click(&app, x, y);
        assert_eq!(releases.get(), 1, "a click on the ring is one");
    }

    #[test]
    fn the_global_tint_switches_off_and_on() {
        use crate::testing;
        let app = testing::window(1);
        let (state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        the_tint_switches_and_keeps_its_values(
            &app,
            &state,
            "",
            |e| e.tint,
            |e| e.adjustments.first().map_or(Tint::OFF, |a| a.look.tint),
        );
    }

    #[test]
    fn a_masks_tint_switches_off_and_on_and_hides_the_overlay() {
        use crate::testing;
        let app = testing::window(1);
        let (state, _worker) = testing::state_for(&app, testing::folder(1));
        app.invoke_select(0);
        app.set_panel_tab("Masks".into());
        app.invoke_new_mask();
        assert_eq!(app.get_target(), 1);
        let name = app.get_adjustment_name().to_string();
        the_tint_switches_and_keeps_its_values(
            &app,
            &state,
            &format!("{name}: "),
            |e| e.adjustments[0].look.tint,
            |e| e.tint,
        );
        // The switch is a look control: with the overlay showing, a
        // flip either way hides it, as a slider does.
        for on in [false, true] {
            app.set_show_mask(true);
            app.set_tint_enabled(on);
            app.invoke_view_changed();
            assert!(!app.get_show_mask(), "switched {on} hides the overlay");
        }
    }

    // A monochrome sensor's frame: the WHITE BALANCE rests.

    /// A click on the switch at the right of the header titled `title`.
    fn click_header_switch(app: &App, title: &str) {
        use i_slint_backend_testing::ElementHandle;
        let (head, size) = crate::testing::labeled(app, title);
        let y = head.y + size.height / 2.0;
        let (at, size) = ElementHandle::find_by_element_id(app, "Switch::root")
            .map(|e| (e.absolute_position(), e.size()))
            .find(|(at, s)| y >= at.y - 4.0 && y <= at.y + s.height + 4.0)
            .unwrap_or_else(|| panic!("no switch on {title:?}"));
        crate::testing::click(app, at.x + size.width / 2.0, at.y + size.height / 2.0);
        slint::platform::update_timers_and_animations();
    }

    /// The control `id` names ("Toggle::root", "PanelButton::root")
    /// around the words `label`, scrolled into view.
    fn control_around(app: &App, id: &str, label: &str) -> i_slint_backend_testing::ElementHandle {
        use i_slint_backend_testing::ElementHandle;
        let (at, size) = crate::testing::labeled(app, label);
        let (x, y) = (at.x + size.width / 2.0, at.y + size.height / 2.0);
        ElementHandle::find_by_element_id(app, id)
            .find(|e| {
                let (p, s) = (e.absolute_position(), e.size());
                x >= p.x && x <= p.x + s.width && y >= p.y && y <= p.y + s.height
            })
            .unwrap_or_else(|| panic!("no {id} around {label:?}"))
    }

    /// Whether the control `id` names around the words `label` takes a
    /// hand, as it tells a screen reader.
    fn control_enabled(app: &App, id: &str, label: &str) -> bool {
        control_around(app, id, label)
            .accessible_enabled()
            .unwrap_or_else(|| panic!("{id} around {label:?} says nothing of it"))
    }

    /// What a screen reader hears of `e`: its role, its name, whether
    /// it takes a hand, and, for a switch, whether it is on.
    fn heard(
        e: &i_slint_backend_testing::ElementHandle,
    ) -> (
        Option<i_slint_backend_testing::AccessibleRole>,
        String,
        Option<bool>,
        Option<bool>,
    ) {
        let checked = e
            .accessible_checkable()
            .unwrap_or(false)
            .then(|| e.accessible_checked().unwrap_or(false));
        (
            e.accessible_role(),
            e.accessible_label().unwrap_or_default().to_string(),
            e.accessible_enabled(),
            checked,
        )
    }

    /// The one element a screen reader knows as `label` in `role`,
    /// scrolled into view.
    fn the_one(
        app: &App,
        role: i_slint_backend_testing::AccessibleRole,
        label: &str,
    ) -> i_slint_backend_testing::ElementHandle {
        use i_slint_backend_testing::ElementHandle;
        crate::testing::labeled(app, label);
        let found: Vec<_> = ElementHandle::find_by_accessible_label(app, label)
            .filter(|e| e.accessible_role() == Some(role))
            .collect();
        assert_eq!(found.len(), 1, "one {role:?} named {label:?}");
        found.into_iter().next().unwrap()
    }

    /// What a screen reader hears of the white balance's controls, on
    /// a color frame and on a monochrome one: As shot a switch named by
    /// its label, on or off; Neutral a button named by its own text,
    /// which is heard once; Undo, a picture alone, a button named Undo
    /// (by its own label, as every such button in the window is; one
    /// without would take its pointer words); a mask's section switch
    /// named by the section's title,
    /// on or off as it shows. Each says whether it takes a hand.
    #[test]
    fn a_screen_reader_hears_the_white_balance() {
        use i_slint_backend_testing::AccessibleRole::{Button, Switch, Text};
        let m = mono("mono-heard", true);
        let app = &m.app;
        for (row, monochrome) in [(1, false), (0, true)] {
            app.set_panel_tab("Develop".into());
            app.invoke_tab_changed("Develop".into());
            m.open(row);
            assert_eq!(app.get_monochrome_open(), monochrome);
            let live = Some(!monochrome);
            let as_shot = the_one(app, Switch, "As shot");
            assert_eq!(
                heard(&as_shot),
                (
                    Some(Switch),
                    "As shot".into(),
                    live,
                    Some(app.get_as_shot())
                ),
            );
            assert_eq!(
                crate::testing::count_labeled(app, "As shot"),
                1,
                "heard once"
            );
            let neutral = control_around(app, "PanelButton::root", "Neutral");
            assert_eq!(heard(&neutral), (Some(Button), String::new(), live, None));
            assert_eq!(the_one(app, Text, "Neutral").accessible_role(), Some(Text));
            assert_eq!(
                crate::testing::count_labeled(app, "Neutral"),
                1,
                "heard once"
            );
            // The picture-alone button itself.
            let undo: Vec<_> = i_slint_backend_testing::ElementHandle::find_by_element_id(
                app,
                "PanelButton::root",
            )
            .filter(|e| e.accessible_label().as_deref() == Some("Undo"))
            .collect();
            assert_eq!(undo.len(), 1, "one button named Undo by its words");
            assert_eq!(heard(&undo[0]).0, Some(Button));

            app.set_panel_tab("Masks".into());
            app.invoke_tab_changed("Masks".into());
            app.invoke_target_changed(1);
            m.settle();
            let switch = the_one(app, Switch, "WHITE BALANCE");
            // On in the edit either way: shown off where it does not act.
            assert!(app.get_local_wb_on());
            assert_eq!(
                heard(&switch),
                (
                    Some(Switch),
                    "WHITE BALANCE".into(),
                    live,
                    Some(!monochrome)
                ),
            );
        }
    }

    /// The mixer's switch is dead while the black and white has its
    /// section: a click on it neither switches it nor falls through to
    /// the header row under it, which would fold the section. Live
    /// again, the click switches it and folds nothing either.
    #[test]
    fn a_dead_switch_takes_the_click() {
        let app = crate::testing::window(1);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (state, _worker) = crate::testing::state_for(&app, crate::testing::folder(1));
        state.borrow_mut().window.batch = true;
        app.invoke_select(0);
        app.set_panel_tab("Develop".into());
        app.set_collapsed_mixer(false);
        app.set_bw_enabled(true);
        let on = app.get_mixer_enabled();
        click_header_switch(&app, "COLOR MIXER");
        assert_eq!(app.get_mixer_enabled(), on, "not switched");
        assert!(!app.get_collapsed_mixer(), "not folded");
        app.set_bw_enabled(false);
        click_header_switch(&app, "COLOR MIXER");
        assert_eq!(app.get_mixer_enabled(), !on, "live, switched");
        assert!(!app.get_collapsed_mixer(), "not folded");
    }

    /// A monochrome DNG and a color one of our own writing, opened
    /// through a real worker, each frame arriving with a white of its
    /// own, as from a paste or a sync, and the first mask's too.
    struct Mono {
        dir: std::path::PathBuf,
        app: App,
        state: Rc<Shared>,
        _worker: Rc<Worker>,
        heard: std::sync::mpsc::Receiver<Outcome>,
    }

    // Whites the panel's sliders hold exactly, so a save that reads
    // them back from the panel writes the same.
    const ARRIVED: WhiteBalance = WhiteBalance::Custom {
        temperature: 4000.0,
        tint: 0.0078125,
    };
    const MASK_ARRIVED: LocalWhite = LocalWhite::Absolute {
        temperature: 3200.0,
        tint: -0.00390625,
    };

    fn mono(what: &str, mask: bool) -> Mono {
        let dir = crate::testing::scratch_dir(what);
        let mono = dir.join("mono.dng");
        crate::testing::monochrome_dng(&mono, 96, 64);
        let color = dir.join("color.dng");
        crate::testing::linear_dng(&color, 96, 64, "Greycard", "Test Body");
        let app = crate::testing::window(2);
        app.window()
            .set_size(slint::LogicalSize::new(1500.0, 950.0));
        let (tx, heard) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let worker = Rc::new(Worker::new(move |o| {
            let _ = tx.lock().unwrap().send(o);
        }));
        let state = crate::testing::state_with(&app, vec![mono, color], worker.clone());
        // A batch state: no offer of models or lenses comes up over the
        // panel, nor is the user's settings file written.
        state.borrow_mut().window.batch = true;
        app.set_collapsed_wb(false);
        for line in &mut state.borrow_mut().sidecars {
            line.current.white_balance = ARRIVED;
            if mask {
                line.current.adjustments.push(greycard_edit::Adjustment {
                    id: 1,
                    name: "Lamp".into(),
                    look: greycard_edit::Look {
                        white_balance: MASK_ARRIVED,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            }
        }
        Mono {
            dir,
            app,
            state,
            _worker: worker,
            heard,
        }
    }

    impl Mono {
        /// Row `row` chosen and its open and develop taken in, as the
        /// window's event loop takes them.
        fn open(&self, row: i32) {
            self.app.invoke_select(row);
            self.land();
        }

        /// The open and develop of the frame chosen last taken in, as
        /// the window takes them: each open and develop handed over,
        /// which drops one of an older generation, until the current
        /// generation's develop.
        fn land(&self) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            loop {
                assert!(std::time::Instant::now() < deadline, "waiting for the open");
                let Ok(o) = self
                    .heard
                    .recv_timeout(std::time::Duration::from_millis(50))
                else {
                    continue;
                };
                let generation = self.state.borrow().develop.generation;
                let last = match &o {
                    Outcome::Opened { .. } => false,
                    Outcome::Developed { generation: g, .. } => *g == generation,
                    Outcome::Failed { message, .. } => panic!("{message}"),
                    _ => continue,
                };
                crate::panel::deliver::deliver(&self.app, o);
                if last {
                    break;
                }
            }
            self.settle();
        }

        fn settle(&self) {
            for _ in 0..2 {
                i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(
                    crate::panel::edit::SAVE_MS + 100,
                ));
                slint::platform::update_timers_and_animations();
            }
        }

        /// What a press could have moved: the edit, the open frame's
        /// sidecar and its history, the develop asked, the panel's
        /// whites, and the dropper.
        fn moved(&self) -> String {
            let st = self.state.borrow();
            let c = st.current.unwrap();
            let line = st.line(c).unwrap();
            format!(
                "{:#?}",
                (
                    c,
                    format!("{:?}", st.develop.edit),
                    format!("{:?}", line.current),
                    line.history.len(),
                    st.develop.generation,
                    (
                        self.app.get_as_shot(),
                        self.app.get_temperature(),
                        self.app.get_tint(),
                    ),
                    (
                        self.app.get_local_wb_on(),
                        self.app.get_local_wb_relative(),
                        self.app.get_local_temperature(),
                        self.app.get_local_tint(),
                    ),
                    self.app.get_picking().to_string(),
                )
            )
        }

        /// Whether the slider labeled `label` takes a hand.
        fn enabled(&self, label: &str) -> bool {
            let (at, _) = crate::testing::labeled(&self.app, label);
            assert!(at.x > 0.0, "{label:?} on screen");
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&self.app, label)
                .find_map(|e| e.accessible_enabled())
                .unwrap_or_else(|| panic!("no slider {label:?}"))
        }

        /// A drag along the slider labeled `label`, from its handle's
        /// side of the track a fifth of the way across.
        fn drag(&self, label: &str) {
            let (at, size) = crate::testing::labeled(&self.app, label);
            let (left, right) = (at.x + 68.0, at.x + size.width - 60.0);
            let x = left + 0.3 * (right - left);
            let y = at.y + size.height / 2.0;
            crate::testing::drag(&self.app, &[(x, y), (x + 10.0, y), (x + 60.0, y)]);
            self.settle();
        }

        /// A double-click on the slider labeled `label`, a reset.
        fn double_click(&self, label: &str) {
            self.settle();
            let (at, size) = crate::testing::labeled(&self.app, label);
            let (x, y) = (at.x + 30.0, at.y + size.height / 2.0);
            crate::testing::click(&self.app, x, y);
            crate::testing::click(&self.app, x, y);
            self.settle();
        }

        /// The whites alone: what the keys must leave, where a slider
        /// they do reach may move something else.
        fn whites(&self) -> String {
            let st = self.state.borrow();
            format!(
                "{:?}",
                (
                    st.current,
                    &st.develop.edit.white_balance,
                    st.develop
                        .edit
                        .adjustments
                        .iter()
                        .map(|a| a.look.white_balance)
                        .collect::<Vec<_>>(),
                    (
                        self.app.get_as_shot(),
                        self.app.get_temperature(),
                        self.app.get_tint(),
                    ),
                    (
                        self.app.get_local_wb_on(),
                        self.app.get_local_temperature(),
                        self.app.get_local_tint(),
                    ),
                )
            )
        }

        /// The sliders' keys: Exposure focused, comma walks up the
        /// column twice toward the white's sliders, and Up steps
        /// whichever slider has the keys. A disabled slider never takes
        /// them, so no white moves.
        fn keys(&self) {
            use slint::platform::Key;
            let (at, size) = crate::testing::labeled(&self.app, "Exposure");
            crate::testing::click(&self.app, at.x + 30.0, at.y + size.height / 2.0);
            crate::testing::press(&self.app, ",");
            crate::testing::press(&self.app, ",");
            crate::testing::press(&self.app, Key::UpArrow);
            crate::testing::press(&self.app, Key::Escape);
            self.settle();
        }

        /// A click on the button labeled `label` in the white
        /// balance's row of Neutral, and on its right edge (a menu's
        /// chevron) too.
        fn click_row(&self, label: &str) {
            let (neutral, _) = crate::testing::labeled(&self.app, "Neutral");
            let (at, size) =
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(&self.app, label)
                    .map(|e| (e.absolute_position(), e.size()))
                    .find(|(at, _)| (at.y - neutral.y).abs() < 4.0)
                    .unwrap_or_else(|| panic!("no {label:?} by Neutral"));
            let y = at.y + size.height / 2.0;
            crate::testing::click(&self.app, at.x + size.width / 2.0, y);
            crate::testing::click(&self.app, at.x + size.width + 12.0, y);
            self.settle();
        }

        /// A click on the switch at the right of the header titled
        /// `title`.
        fn click_header_switch(&self, title: &str) {
            click_header_switch(&self.app, title);
            self.settle();
        }

        /// The presets' six, as many as are on screen.
        fn presets_shown(&self) -> usize {
            WHITE_PRESETS
                .iter()
                .map(|(n, ..)| crate::testing::count_labeled(&self.app, n))
                .sum()
        }
    }

    impl Drop for Mono {
        fn drop(&mut self) {
            crate::testing::remove_dir_retry(&self.dir);
        }
    }

    /// On a monochrome frame the Develop tab's WHITE BALANCE rests
    /// whole: its sliders are disabled, and a drag, a double-click, the
    /// keys, As shot, Auto and its presets, Neutral, W, and the calls
    /// behind them change neither the edit nor the picture, which keeps
    /// the white it arrived with. The section still folds. A color frame
    /// opened after it is live again, and the monochrome one chosen
    /// again rests before its decode lands.
    #[test]
    fn a_monochrome_frames_white_balance_rests() {
        let m = mono("mono-white", false);
        let app = &m.app;
        // A color frame first, the dropper taken up on it: opening the
        // monochrome frame puts it down.
        m.open(1);
        assert!(!app.get_monochrome_open());
        assert!(!app.invoke_sheet_open(), "no sheet over the keys");
        crate::testing::press(app, "w");
        assert_eq!(app.get_picking(), "White");
        m.open(0);
        assert!(!app.invoke_sheet_open());
        assert!(app.get_monochrome_open());
        assert_eq!(app.get_picking(), "", "the dropper put down");
        assert_eq!(m.state.borrow().develop.edit.white_balance, ARRIVED);
        assert!(!app.get_as_shot());

        let before = m.moved();
        assert!(!m.enabled("Temperature") && !m.enabled("Tint"));
        assert!(!control_enabled(app, "Toggle::root", "As shot"));
        assert!(!control_enabled(app, "PanelButton::root", "Neutral"));
        m.drag("Temperature");
        m.drag("Tint");
        m.double_click("Temperature");
        m.double_click("Tint");
        crate::testing::click_switch(app, "As shot");
        m.settle();
        m.click_row("Auto");
        assert!(!app.get_menu_up(), "no presets' menu");
        m.click_row("Neutral");
        assert_eq!(app.get_picking(), "");
        crate::testing::press(app, "w");
        assert_eq!(app.get_picking(), "");
        // W from a tab without the dropper does not bring Develop.
        app.set_panel_tab("Masks".into());
        crate::testing::press(app, "w");
        assert_eq!(app.get_panel_tab(), "Masks");
        assert_eq!(app.get_picking(), "");
        app.set_panel_tab("Develop".into());
        app.set_wb_presets_shown(true);
        assert_eq!(m.presets_shown(), 0, "the menu asked for does not open");
        // A warm picture for Auto to read: no white from it, and no
        // word of a miss either.
        crate::panel::viewport::AUTO_WHITE_GRID
            .with(|g| *g.borrow_mut() = Some((vec![[0.3, 0.2, 0.1]; 1600], 40)));
        app.set_status("".into());
        app.invoke_wb_auto();
        assert_eq!(app.get_status(), "");
        crate::panel::viewport::AUTO_WHITE_GRID.with(|g| *g.borrow_mut() = None);
        app.invoke_wb_preset("Tungsten".into());
        app.invoke_pick_started("White".into());
        assert_eq!(app.get_picking(), "");
        // A press with the dropper somehow in hand reads nothing, nor
        // says it waits for the develop.
        app.set_status("".into());
        app.set_picking("White".into());
        app.invoke_pick_pressed(600.0, 300.0);
        assert_eq!(app.get_status(), "");
        app.set_picking("".into());
        // The calls behind As shot and a slider's reset, as a key or a
        // click that reached the control would make them: refused, the
        // edit's white shown again.
        app.set_as_shot(!app.get_as_shot());
        app.invoke_as_shot_toggled();
        m.settle();
        assert_eq!(m.moved(), before, "As shot");
        app.set_temperature(app.get_temperature() + 500.0);
        app.invoke_wb_slider_reset("tint".into());
        m.settle();
        assert_eq!(m.moved(), before, "a slider's reset");
        assert_eq!(m.state.borrow().develop.edit.white_balance, ARRIVED);
        let whites = m.whites();
        m.keys();
        assert_eq!(m.whites(), whites);

        // It folds and opens as any section does.
        let folded = app.get_collapsed_wb();
        let (at, size) = crate::testing::labeled(app, "WHITE BALANCE");
        crate::testing::click(app, at.x + size.width / 2.0, at.y + size.height / 2.0);
        assert_eq!(app.get_collapsed_wb(), !folded);
        crate::testing::click(app, at.x + size.width / 2.0, at.y + size.height / 2.0);
        assert_eq!(app.get_collapsed_wb(), folded);
        m.settle();

        // A color frame: live, its white the one it arrived with until
        // a drag moves it.
        m.open(1);
        assert!(!app.get_monochrome_open());
        assert!(m.enabled("Temperature") && m.enabled("Tint"));
        assert!(control_enabled(app, "Toggle::root", "As shot"));
        assert!(control_enabled(app, "PanelButton::root", "Neutral"));
        let before = m.moved();
        m.drag("Temperature");
        assert_ne!(m.moved(), before);
        assert_ne!(m.state.borrow().develop.edit.white_balance, ARRIVED);
        crate::testing::press(app, "w");
        assert_eq!(app.get_picking(), "White");

        // The monochrome frame again: known, so it rests from the
        // moment it is chosen, the dropper put down; its sidecar's
        // white is still its own.
        app.invoke_select(0);
        assert!(app.get_monochrome_open(), "known before the decode");
        m.settle();
        assert_eq!(app.get_picking(), "", "put down before the decode");
        m.land();
        assert!(app.get_monochrome_open());
        assert_eq!(m.state.borrow().develop.edit.white_balance, ARRIVED);
    }

    /// A mask's WHITE BALANCE on a monochrome frame rests too: its
    /// switch, Temperature, Tint, Follow global, and Neutral change
    /// neither the mask's white it arrived with nor the picture; on a
    /// color frame they are live.
    #[test]
    fn a_monochrome_frames_mask_white_balance_rests() {
        let m = mono("mono-mask-white", true);
        let app = &m.app;
        let to_mask = || {
            app.set_panel_tab("Masks".into());
            app.invoke_tab_changed("Masks".into());
            app.invoke_target_changed(1);
            assert_eq!(app.get_target(), 1);
            m.settle();
        };
        m.open(0);
        assert!(app.get_monochrome_open());
        to_mask();
        // As the panel holds it: what a save would write.
        let mask_white = || {
            let st = m.state.borrow();
            crate::panel::edit::read_edit(app, &st.develop.edit, st.develop.target).adjustments[0]
                .look
                .white_balance
        };
        assert_eq!(mask_white(), MASK_ARRIVED);
        assert!(app.get_local_wb_on());

        let before = m.moved();
        assert!(!m.enabled("Temperature") && !m.enabled("Tint"));
        assert!(!control_enabled(app, "Toggle::root", "Follow global"));
        assert!(!control_enabled(app, "PanelButton::root", "Neutral"));
        m.drag("Temperature");
        m.drag("Tint");
        m.double_click("Temperature");
        crate::testing::click_switch(app, "Follow global");
        m.settle();
        let (at, size) = crate::testing::labeled(app, "Neutral");
        crate::testing::click(app, at.x + size.width / 2.0, at.y + size.height / 2.0);
        crate::testing::press(app, "w");
        let folded = app.get_collapsed_wb();
        m.click_header_switch("\u{b7}  WHITE BALANCE");
        assert_eq!(
            app.get_collapsed_wb(),
            folded,
            "the dead switch folds nothing"
        );
        // The calls behind the switch, Follow global, and a double-click,
        // as a key or a click that reached the control would make them:
        // refused, the mask's white shown again.
        m.settle();
        assert_eq!(m.moved(), before);
        app.set_local_wb_relative(!app.get_local_wb_relative());
        app.invoke_local_wb_relative_toggled();
        m.settle();
        assert_eq!(m.moved(), before, "Follow global");
        app.set_local_wb_on(!app.get_local_wb_on());
        app.invoke_local_wb_toggled();
        m.settle();
        assert_eq!(m.moved(), before, "the switch");
        app.invoke_local_wb_reset();
        m.settle();
        assert_eq!(m.moved(), before, "the reset");
        assert_eq!(mask_white(), MASK_ARRIVED);
        let whites = m.whites();
        m.keys();
        assert_eq!(m.whites(), whites);

        // A color frame's mask: live.
        app.set_panel_tab("Develop".into());
        app.invoke_tab_changed("Develop".into());
        m.open(1);
        assert!(!app.get_monochrome_open());
        to_mask();
        assert!(m.enabled("Temperature") && m.enabled("Tint"));
        assert!(control_enabled(app, "Toggle::root", "Follow global"));
        assert!(control_enabled(app, "PanelButton::root", "Neutral"));
        let before = m.moved();
        m.drag("Temperature");
        assert_ne!(m.moved(), before);
        assert_ne!(mask_white(), MASK_ARRIVED);
    }

    /// How the open frame came to be known monochrome while a white
    /// control held the focus.
    enum Route {
        /// Left from a color frame to a monochrome one already known.
        Left,
        /// Home from a color frame to a monochrome one already known.
        Home,
        /// The monochrome frame's own decode landing, the control
        /// focused in the moment before it, when the frame was not yet
        /// known. A mask's controls go with the mask a frame change
        /// unchooses, so this is their only way.
        Decode,
    }

    /// A white control holding the focus when the open frame turns out
    /// monochrome takes its own keys and does nothing: focused, then
    /// the frame known monochrome by `route`, then `keys`. The
    /// monochrome frame's stored whites are the ones it arrived with,
    /// and nothing reached the window: no frame step, no zoom, no grid
    /// or cull.
    fn a_focused_control_takes_no_key(
        what: &str,
        mask: bool,
        route: Route,
        focus: impl Fn(&Mono),
        keys: &[slint::SharedString],
    ) {
        use slint::platform::Key;
        let m = mono(what, mask);
        let app = &m.app;
        match route {
            Route::Left | Route::Home => {
                m.open(0);
                m.open(1);
            }
            Route::Decode => app.invoke_select(0),
        }
        assert!(!app.get_monochrome_open());
        assert!(!app.invoke_sheet_open(), "no sheet over the keys");
        if mask {
            app.set_panel_tab("Masks".into());
            app.invoke_tab_changed("Masks".into());
            app.invoke_target_changed(1);
            m.settle();
        }
        // Twice, so the white is as it was, and the control keeps the
        // focus.
        focus(&m);
        focus(&m);
        let key = match route {
            Route::Left => Some(Key::LeftArrow),
            Route::Home => Some(Key::Home),
            Route::Decode => None,
        };
        if let Some(key) = key {
            crate::testing::press(app, key);
            m.settle();
        }
        m.land();
        assert_eq!(m.state.borrow().current, Some(0));
        assert!(app.get_monochrome_open());
        if mask {
            assert_eq!(app.get_target(), 1, "the mask still chosen");
        }
        let window = || {
            let st = m.state.borrow();
            (
                st.current,
                st.develop.zoom,
                app.get_grid_open(),
                st.culling.cull.is_some(),
            )
        };
        let (before, shown) = (m.moved(), window());
        for k in keys {
            crate::testing::press(app, k.clone());
        }
        m.settle();
        assert_eq!(m.moved(), before);
        assert_eq!(window(), shown, "no key reached the window");
        stored_arrived(&m);
    }

    /// Space and Return, a button's own keys.
    fn button_keys() -> Vec<slint::SharedString> {
        vec![" ".into(), slint::platform::Key::Return.into()]
    }

    /// The monochrome frame's stored whites, read off its sidecar.
    fn stored_arrived(m: &Mono) {
        let st = m.state.borrow();
        let line = st.line(0).unwrap();
        assert_eq!(line.current.white_balance, ARRIVED);
        if let Some(a) = line.current.adjustments.first() {
            assert_eq!(a.look.white_balance, MASK_ARRIVED);
        }
    }

    fn click_as_shot(m: &Mono) {
        crate::testing::click_switch(&m.app, "As shot");
        m.settle();
    }

    fn click_follow_global(m: &Mono) {
        crate::testing::click_switch(&m.app, "Follow global");
        m.settle();
    }

    #[test]
    fn a_focused_as_shot_takes_no_key_on_a_monochrome_frame() {
        // Left alone: As shot asks a develop, whose newer generation
        // the open landing after it then gives way to.
        a_focused_control_takes_no_key(
            "mono-focus-left",
            false,
            Route::Left,
            click_as_shot,
            &button_keys(),
        );
    }

    #[test]
    fn a_focused_mask_switch_takes_no_key_on_a_monochrome_frame() {
        a_focused_control_takes_no_key(
            "mono-focus-switch",
            true,
            Route::Decode,
            |m| m.click_header_switch("\u{b7}  WHITE BALANCE"),
            &button_keys(),
        );
    }

    #[test]
    fn a_focused_follow_global_takes_no_key_on_a_monochrome_frame() {
        a_focused_control_takes_no_key(
            "mono-focus-follow",
            true,
            Route::Decode,
            click_follow_global,
            &button_keys(),
        );
    }

    /// A focused Temperature, Home to the monochrome frame, and its
    /// step keys: no step, and no frame stepped by the arrows either.
    /// The slider's own guard alone keeps this white: the develop a
    /// step asks for reads the panel as any slider's does.
    #[test]
    fn a_focused_temperature_takes_no_step_on_a_monochrome_frame() {
        use slint::platform::Key;
        // More up than down, so steps taken would not cancel out.
        let keys: Vec<slint::SharedString> = [Key::UpArrow, Key::RightArrow, Key::LeftArrow]
            .into_iter()
            .map(Into::into)
            .chain(["+".into(), "=".into()])
            .collect();
        a_focused_control_takes_no_key(
            "mono-focus-temperature",
            false,
            Route::Home,
            |m| {
                // On the label: focused, the value left where it is.
                let (at, size) = crate::testing::labeled(&m.app, "Temperature");
                crate::testing::click(&m.app, at.x + 30.0, at.y + size.height / 2.0);
                m.settle();
            },
            &keys,
        );
    }

    /// The guides as they draw, for an eye to judge: a grading wheel
    /// and the tint's ring mid-drag, Shift's line and Ctrl's circle,
    /// at scales 1 and 2, on Charcoal and Light, drawn by Slint's
    /// software renderer. A drag in the middle cannot be had from
    /// outside the window, so the state is set here. Run with
    /// `GREYCARD_WHEEL_CAPTURES=DIR` and `--ignored`.
    #[test]
    #[ignore]
    fn capture_the_wheel_guides() {
        use crate::testing;
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        let Some(dir) = std::env::var_os("GREYCARD_WHEEL_CAPTURES") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        testing::drawing_backend();
        for theme in ["Charcoal", "Light"] {
            for scale in [1.0f32, 2.0] {
                for ring in [false, true] {
                    for held in [Key::Shift, Key::Control] {
                        let app = testing::window(1);
                        app.window()
                            .dispatch_event(WindowEvent::ScaleFactorChanged {
                                scale_factor: scale,
                            });
                        // The testing window takes a size as physical
                        // and logical both; the snapshot's buffer is the
                        // physical, the layout the logical.
                        app.window().set_size(slint::PhysicalSize::new(
                            (1200.0 * scale) as u32,
                            (3000.0 * scale) as u32,
                        ));
                        app.window().dispatch_event(WindowEvent::Resized {
                            size: slint::LogicalSize::new(1200.0, 3000.0),
                        });
                        app.global::<crate::Theme>().set_name(theme.into());
                        let (_state, _worker) = testing::state_for(&app, testing::folder(1));
                        app.invoke_select(0);
                        app.set_panel_tab("Develop".into());
                        // The sections' opening animations run out first,
                        // or the ring is found where it stood before them.
                        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(
                            1,
                        ));
                        // The wheel scrolled into view and where it is
                        // then; the crop, a margin round it (round the
                        // row of three for the grading).
                        let (c, crop) = if ring {
                            let e =
                                i_slint_backend_testing::ElementHandle::find_by_element_type_name(
                                    &app,
                                    "CurveEditor",
                                )
                                .find(|e| e.size().width == 176.0)
                                .expect("the tint ring is on screen");
                            let (at, size) = (e.absolute_position(), e.size());
                            (
                                (at.x + size.width / 2.0, at.y + size.height / 2.0),
                                (
                                    at.x - 24.0,
                                    at.y - 24.0,
                                    size.width + 48.0,
                                    size.height + 48.0,
                                ),
                            )
                        } else {
                            let _ = center(&app, "Shadows wheel");
                            i_slint_backend_testing::mock_elapsed_time(
                                std::time::Duration::from_secs(1),
                            );
                            let (a, sa) = testing::labeled(&app, "Shadows wheel");
                            let (b, sb) = testing::labeled(&app, "Highlights wheel");
                            (
                                (a.x + sa.width / 2.0, a.y + sa.height / 2.0),
                                (
                                    a.x - 12.0,
                                    a.y - 12.0,
                                    b.x + sb.width - a.x + 24.0,
                                    sa.height + 24.0,
                                ),
                            )
                        };
                        let (cx, cy) = c;
                        let k = if ring { 1.8 } else { 1.0 };
                        let at = |dx: f32, dy: f32| {
                            slint::LogicalPosition::new(cx + k * dx, cy + k * dy)
                        };
                        let to = |dx, dy| {
                            app.window().dispatch_event(WindowEvent::PointerMoved {
                                position: at(dx, dy),
                            })
                        };
                        let key = |down: bool| {
                            let text: slint::SharedString = held.into();
                            app.window().dispatch_event(if down {
                                WindowEvent::KeyPressed { text }
                            } else {
                                WindowEvent::KeyReleased { text }
                            });
                        };
                        to(25.0, 0.0);
                        app.window().dispatch_event(WindowEvent::PointerPressed {
                            position: at(25.0, 0.0),
                            button: PointerEventButton::Left,
                        });
                        to(20.0, -20.0);
                        key(true);
                        // On past the key, so the hold shows: Shift's
                        // pointer off its ray, Ctrl's round its circle.
                        if held == Key::Shift {
                            to(32.0, -6.0);
                        } else {
                            to(-26.0, 6.0);
                        }
                        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_secs(
                            1,
                        ));
                        assert_ne!(app.get_wheel_guide().kind, 0);
                        // The ring measured again: the panel has moved
                        // under it since the press (the picture's
                        // develop settling above), and the snapshot is
                        // of now.
                        let crop = if ring {
                            let e =
                                i_slint_backend_testing::ElementHandle::find_by_element_type_name(
                                    &app,
                                    "CurveEditor",
                                )
                                .find(|e| e.size().width == 176.0)
                                .expect("the tint ring is on screen");
                            let (at, size) = (e.absolute_position(), e.size());
                            (
                                at.x - 24.0,
                                at.y - 24.0,
                                size.width + 48.0,
                                size.height + 48.0,
                            )
                        } else {
                            crop
                        };
                        let shot = app.window().take_snapshot().expect("a drawing backend");
                        key(false);
                        app.window().dispatch_event(WindowEvent::PointerReleased {
                            position: at(0.0, 0.0),
                            button: PointerEventButton::Left,
                        });
                        let full = image::RgbaImage::from_raw(
                            shot.width(),
                            shot.height(),
                            shot.as_bytes().to_vec(),
                        )
                        .unwrap();
                        let (x, y, w, h) = crop;
                        let px = |v: f32| (v * scale).max(0.0).round() as u32;
                        let part =
                            image::imageops::crop_imm(&full, px(x), px(y), px(w), px(h)).to_image();
                        let name = format!(
                            "{}-x{}-{}-{}.png",
                            theme.to_lowercase(),
                            scale as u32,
                            if ring { "tint" } else { "wheels" },
                            if held == Key::Shift {
                                "shift-line"
                            } else {
                                "ctrl-circle"
                            },
                        );
                        part.save(dir.join(name)).unwrap();
                    }
                }
            }
        }
    }
}
