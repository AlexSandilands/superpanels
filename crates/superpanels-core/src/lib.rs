#![forbid(unsafe_code)]

//! Pure-logic core for the Superpanels wallpaper manager.
//!
//! See [SPEC.md] for the design and [PLAN.md] for the development plan.

/// The crate version, sourced from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
