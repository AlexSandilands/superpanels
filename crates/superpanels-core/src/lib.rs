#![forbid(unsafe_code)]

//! Pure-logic core for the Superpanels wallpaper manager.

pub mod backends;
pub mod config;
pub mod display;
pub mod image;
pub mod layout;
pub mod library;
pub mod slideshow;

pub use backends::{
    AppliedReport, BackendError, CustomBackend, FehBackend, GnomeBackend, HyprlandBackend,
    KdeBackend, MockBackend, SwayBackend, UnavailableBackend, WallpaperBackend, detect_backend,
};
pub use config::{
    BackendConfig, BackendKind, Config, ConfigError, GeneralConfig, LibraryConfig, MonitorConfig,
    Profile,
};
pub use display::{
    Availability, DetectError, DisplayDetector, Monitor, MonitorRef, Rotation, detect,
};
pub use layout::{BezelConfig, CropSpec, FitMode, LayoutError, Rect, compute_crop_specs};
pub use library::{
    FolderWatcher, LibraryEntry, LibraryError, load_index, persist_index, scan_folder,
};
pub use slideshow::{
    SlideshowConfig, SlideshowError, SlideshowPicker, SlideshowSort, SlideshowStart, SlideshowState,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
