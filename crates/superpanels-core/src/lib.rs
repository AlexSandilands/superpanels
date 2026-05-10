#![forbid(unsafe_code)]

//! Pure-logic core for the Superpanels wallpaper manager.

pub mod backends;
pub mod config;
pub mod display;
pub mod image;
pub mod ipc;
pub mod layout;
pub mod library;
pub mod schedule;
pub mod slideshow;
pub mod validity;

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
pub use layout::{
    CropSpec, FitMode, ImageRectMm, LayoutError, Rect, compute_crop_specs, cover_image_rect_mm,
    synthesise_placements,
};
pub use library::{
    DEFAULT_LIBRARY_PAGE, DbError, FolderWatcher, LibraryDb, LibraryEntry, LibraryError,
    LibraryFilter, MAX_LIBRARY_PAGE, MigrationError, SCHEMA_VERSION, apply_library_filter,
    load_index, migrate_json_to_sqlite, persist_index, scan_folder,
};
pub use schedule::{MonitorPlacement, Schedule, ScheduleError, TopologyFingerprint, Trigger};
pub use slideshow::{
    SlideshowConfig, SlideshowError, SlideshowPicker, SlideshowSort, SlideshowStart, SlideshowState,
};
pub use validity::{DisableReason, ProfileValidity};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
