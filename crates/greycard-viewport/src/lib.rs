//! The viewport: the developed frame drawn on the window's device
//! through `viewport.wgsl`, held by its tests to the render crate's
//! CPU finish; the scopes' binning, which `scope.wgsl` does on the
//! device and [`scope`] is the reference for; the monitor's display
//! table; and the device's loss as the viewport's waits see it.
//!
//! No Slint type: what the device gives back is a plain picture
//! ([`Rgba8`]), and the window makes Slint's of it.

pub mod display;
pub mod lost;
pub mod peaking;
mod pixels;
pub mod render;
pub mod scope;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use pixels::Rgba8;
