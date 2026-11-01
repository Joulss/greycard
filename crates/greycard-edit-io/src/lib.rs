//! The edit's files from elsewhere, read and written: Lightroom presets
//! and catalog settings, RawTherapee profiles, the XMP beside a frame,
//! and preset files brought into a store; and the user's camera profile
//! and look directories, listed, read, and handed to a develop as
//! [`Stores`]. The edit itself, its sidecar, and its migrations are
//! `greycard-edit`'s; this crate reads files into them.

pub mod key;
pub mod lightroom;
pub mod looks;
pub mod on_disk;
pub mod pp3;
pub mod preset;
pub mod profiles;
mod stores;
// XMP interop for the meta section (§72): read and write the
// standard fields in an `.xmp` beside the frame.
pub mod xmp;

pub use stores::Stores;
