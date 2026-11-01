//! The develop below the window: one list of stages for every develop
//! of an edit, derived from the edit and what the develop is for
//! (`stages`), the walk that runs it and keeps what it passes for the
//! next (`walk`), and the one finish both ends read (`finish`).
//!
//! It stands between the render crate and the window, and depends on
//! core, edit, render, gpu, ai, and lens alone: never on the window,
//! Slint, or anything that names a window's state. The GPU is lent to
//! it as an optional context by whoever owns a device, never made
//! here; without one, every stage runs on the CPU.

pub mod finish;
pub mod stages;
pub mod walk;
