//! A Lightroom preset from before 7.3, a `.lrtemplate`, through the
//! strict table reader and the mapper: the preset it makes, and the
//! edit that preset makes, each write as JSON and read back equal.
#![no_main]

use greycard_edit_io::lightroom;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(imported) = lightroom::read_template(text, "fuzz") {
        greycard_fuzz::preset_round_trips(&imported.preset, "lrtemplate");
        greycard_fuzz::applied_round_trips(&imported.preset, "lrtemplate applied");
    }
});
