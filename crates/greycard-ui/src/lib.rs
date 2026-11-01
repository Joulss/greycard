//! The editor's modules that need no window: no Slint type, no wgpu,
//! and no `State`, so their tests run without one and a later engine
//! crate can reach them. The binary, `main.rs`, holds the window and
//! names these at the paths it always has.

pub mod archive;
pub mod carried;
pub mod collections;
pub mod delete;
pub mod display;
pub mod edit_in;
pub mod export_name;
pub mod files;
pub mod filter;
pub mod gpu_marker;
pub mod import;
pub mod instance;
pub mod log;
pub mod mounts;
pub mod naming;
pub mod outline;
pub mod preset_fit;
pub mod rejects;
pub mod scrolling;
pub mod selection;
pub mod tags;
pub mod wheel;
pub mod zoom;
