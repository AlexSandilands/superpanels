#![forbid(unsafe_code)]

//! Pure-logic core for the Superpanels wallpaper manager.
//!
//! See [SPEC.md] for the design and [PLAN.md] for the development plan.

pub mod config;
pub mod display;
pub mod image;
pub mod layout;

pub use config::{
    BackendConfig, BackendKind, Config, ConfigError, GeneralConfig, LibraryConfig, MonitorConfig,
    Profile,
};
pub use display::{
    Availability, DetectError, DisplayDetector, Monitor, MonitorRef, Rotation, detect,
};
pub use layout::{BezelConfig, CropSpec, FitMode, LayoutError, Rect, compute_crop_specs};

/// The crate version, sourced from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
