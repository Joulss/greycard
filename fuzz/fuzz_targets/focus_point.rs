//! The camera's focus point read from a raw's head: a CR3's box walk to
//! its maker note and Canon's AFInfo2 record; a TIFF raw's IFDs to a
//! CR2's, NEF's, ARW's, RW2's (through its embedded JPEG), or ORF's
//! note; a RAF's header to its JPEG's note and its own metadata. Never
//! a panic, and a point found lies on the picture: its center within
//! the frame, its size no larger than it, every number finite.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(Some((point, _))) = greycard_core::decode::focus_point_bytes(data) {
        let [x, y, w, h] = point.area;
        assert!(point.area.iter().all(|v| v.is_finite()));
        // Within a rounding of the f64 the record is judged in.
        let on = -1e-4..=1.0 + 1e-4;
        assert!(w > 0.0 && h > 0.0 && on.contains(&w) && on.contains(&h));
        assert!(on.contains(&(x + w / 2.0)) && on.contains(&(y + h / 2.0)));
    }
});
