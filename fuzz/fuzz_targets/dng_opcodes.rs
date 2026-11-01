//! A DNG's `OpcodeList2`, read for its gain maps and each map applied
//! to a small buffer, one plane and three: never a panic, and samples
//! that start between 0 and 1 stay finite. The first four bytes place
//! the buffer in the maps' image; the rest is the list.
#![no_main]

use greycard_core::develop::gain_map::{MapPlacement, apply_gain_maps};
use greycard_core::opcodes::gain_maps;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((head, list)) = data.split_first_chunk::<4>() else {
        return;
    };
    let maps = gain_maps(list);
    let (width, height) = (7, 5);
    let at = MapPlacement {
        x: usize::from(head[0] % 8),
        y: usize::from(head[1] % 8),
        image_width: usize::from(head[2] % 32),
        image_height: usize::from(head[3] % 32),
    };
    for cpp in [1, 3] {
        let mut samples: Vec<f32> = (0..width * height * cpp)
            .map(|i| (i % 11) as f32 / 10.0)
            .collect();
        let held = apply_gain_maps(&mut samples, width, cpp, at, &maps);
        assert!(held <= samples.len() * maps.len());
        assert!(samples.iter().all(|s| s.is_finite()), "{samples:?}");
    }
});
