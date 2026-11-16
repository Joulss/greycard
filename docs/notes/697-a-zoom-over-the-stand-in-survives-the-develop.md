# §697. A zoom over the stand-in survives the develop (2026-11-15)

**The bug.** In Develop, a frame zoomed into while its camera picture (or the
edit's picture) stood in snapped back to Fit when its develop landed. Space,
Z, the wheel, a click to 1:1, and the zoom menu all did it.

**Cause.** `panel/deliver.rs` (the `Outcome::Developed` arm, line 718 on
master) fitted the view whenever the last frame's picture was still held:
`if st.develop.held.take().is_some() { st.develop.zoom = 0.0; }`. A frame
chosen from another one holds that one's picture until its own develop lands,
and the stand-in goes up in between, so the zoom chosen on the stand-in read as
the last frame's zoom and was dropped. Where nothing was held (the session's
first open), the zoom stayed but the place did not: the render
(`panel/startup.rs`, line 1310 on master) re-centered any frame whose size
changed, and the develop's frame is not the stand-in's size.

**Since when.** Always. The rule is in the first commit (3c74cbf7) and in
v0.11.0 unchanged (`deliver.rs:691`), as are the stand-in, `held`, and the zoom
keys. §688 and §692 (Space and Z to the focus point) and §585 (the edit
stand-in's size) only made the stand-in easier to zoom on; none of them caused it.

**The fix.**
- **Noting the view:** when the develop lands, `cull::stood_view` notes the
  view chosen on the open frame's own stand-in: the zoom, whether it is a named
  level, the center as fractions of the stand-in, the stand-in's width, and
  what it shows (the edit's frame, or the camera's picture, cut to the camera's
  frame or of the whole sensor). It is noted only for a zoomed view; a fit
  lands as a fit anyway. The develop is then not fitted.
- **One layout:** the render's layout is now one function,
  `viewport::frame_on_show`, which the render calls and the tests call
  directly; the render's closure itself runs in no test. It works
  out the frame, the export size for the sheet, and the centering, and it puts
  the noted view onto the develop's frame.
  - The edit's picture maps by fraction.
  - The camera's picture maps through the frame's turns and mirror to the
    source, then through the edit's geometry, so a crop, a straighten, or a
    keystone lands on the same place.
- **Named and free zooms:** a named level (Space, Z, a click to 1:1, the zoom
  menu, all through `zoom_to`) stays that level of the develop. Someone who
  pressed Space to check focus on a small embedded JPEG gets 1:1 of the
  develop, not about 30%. A free zoom (the wheel, a pinch, the + and - keys, marked by
  `Develop::zoom_free`) keeps its scale on screen. A middle click's way back
  from 200% carries the flag (`ZoomBack::free`).
- **Command line:** the command line's zoom about the middle (`zoom_stands`)
  stands into the develop as before.
- **Where the note ends:** the noted view cannot outlive its frame.
  `browser::current_moved` (every change of the current frame) and Cull's
  select and enter clear it. The layout also takes it only onto the develop it
  was noted for, and only when that develop is the open frame's own
  (`own_develop`).

**Unchanged.**
- A frame changed while zoomed opens fitted under its stand-in and its develop
  (`placeholder_arrived` fits it, so nothing is noted).
- A develop that lands while another frame's picture still stands in is fitted.
- Cull's loupe is untouched: no develop replaces its picture.

**Changed beyond the bug.** Leaving Cull for Develop, the loupe's picture
stands in (`Wait::held`), and a zoom chosen on it now stays too.

**Tests** (`panel/cull.rs`, through `frame_on_show`):
- `a_zoom_key_pressed_over_the_stand_in_stays_when_the_develop_lands`: Space on
  the focus point at 1:1, and Z at 3:1 with a pan on a JPEG half the frame's
  size. It fails on master with the zoom at 0.
- `a_small_stand_in_keeps_a_named_level_and_a_wheels_scale`: a stand-in a
  quarter of the frame's width.
- `a_wheel_or_click_zoom_over_the_stand_in_stays_through_a_crop`
- `a_frame_chosen_before_the_draw_takes_no_zoom_from_the_last`
- `a_zoom_on_the_loupes_picture_after_cull_stays_into_develop`
- `a_zoom_taken_back_to_fit_over_the_stand_in_lands_fitted`
- `frames_arrowed_through_while_zoomed_open_fitted`
- `the_stand_ins_place_goes_through_every_turn_and_mirror_to_a_crop`
- In `panel/viewport.rs`, the property test
  `a_stand_ins_point_lands_on_the_same_source_point` (2000 random geometries).

**Known limits.**
- A camera JPEG of another aspect than the raw (16:9 or 1:1 in camera, uncut)
  maps by its fractions and may land slightly off.
- A free zoom's scale is matched on the frame's width only, so under a
  keystone or a straighten it is close rather than exact.
