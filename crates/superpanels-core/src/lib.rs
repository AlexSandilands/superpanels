#![forbid(unsafe_code)]

//! Pure-logic core for the Superpanels wallpaper manager.
//!
//! See [SPEC.md] for the design and [PLAN.md] for the development plan.

pub mod display;
pub mod layout;

pub use display::{Monitor, MonitorRef, Rotation};
pub use layout::{BezelConfig, CropSpec, FitMode, Rect};

/// The crate version, sourced from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
