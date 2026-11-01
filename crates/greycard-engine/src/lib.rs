//! The develop below the window: one list of stages for every develop
//! of an edit, derived from the edit and what the develop is for
//! (`stages`), the walk that runs it and keeps what it passes for the
//! next (`walk`), the one finish both ends read (`finish`), and the
//! worker that takes the window's jobs and runs them (`worker`), with
//! what it holds: the reduced develop, the thumbnail pool, the
//! previews, the edit's pictures, the models, and the export queue.
//!
//! It stands between the render crate and the window, and depends on
//! core, edit, edit-io, render, gpu, ai, lens, and the library alone:
//! never on the window, Slint, or anything that names a window's
//! state. The GPU is lent to it as an optional context by whoever
//! owns a device, never made here; without one, every stage runs on
//! the CPU.

pub mod ai;
pub mod camera;
pub mod edited_pictures;
pub mod finish;
pub mod lens;
pub mod made;
pub mod naming;
pub mod previews;
pub mod queue;
pub mod reduced;
pub mod sheet;
pub mod stages;
pub mod thumbpool;
pub mod version_key;
pub mod walk;
pub mod worker;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
