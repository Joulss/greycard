//! Pictures the scopes' tests here share with the window's tests of
//! their drawing. Built for this crate's tests and, behind the
//! `testing` feature, for the window's.

/// A picture whose value rises with the column, grey.
pub fn ramp(w: usize, h: usize) -> Vec<[f32; 3]> {
    (0..w * h)
        .map(|i| {
            let v = (i % w) as f32 / (w - 1) as f32;
            [v; 3]
        })
        .collect()
}

/// A picture of one color.
pub fn flat(w: usize, h: usize, c: [f32; 3]) -> Vec<[f32; 3]> {
    vec![c; w * h]
}

/// A picture with something in every scope: hue across, value
/// down.
pub fn colors(w: usize, h: usize) -> Vec<[f32; 3]> {
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let t = x * std::f32::consts::TAU;
            [
                y * (0.5 + 0.5 * t.cos()),
                y * (0.5 + 0.5 * (t + 2.1).cos()),
                y * (0.5 + 0.5 * (t + 4.2).cos()),
            ]
        })
        .collect()
}
