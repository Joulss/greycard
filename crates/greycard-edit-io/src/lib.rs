//! The edit's files from elsewhere, read and written: Lightroom presets
//! and catalog settings, RawTherapee profiles, the XMP beside a frame,
//! and preset files brought into a store. The edit itself, its sidecar,
//! and its migrations are `greycard-edit`'s; this crate reads other
//! tools' files into them.

pub mod lightroom;
pub mod pp3;
pub mod preset;
// XMP interop for the meta section (§72): read and write the
// standard fields in an `.xmp` beside the frame.
pub mod xmp;
