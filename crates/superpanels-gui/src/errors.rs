//! `IpcError` — the single shape returned to the frontend across all
//! `#[tauri::command]`s (`SPEC.md` §12.4). Concrete typed errors from
//! `superpanels-core` flatten into this enum so the JS side gets a stable
//! tagged union it can switch on.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "kind", content = "message")]
pub enum IpcError {
    /// The daemon was reachable but it returned an error string.
    Daemon(String),
    /// Configuration load / save / validation failed.
    Config(String),
    /// Display detection failed.
    Detect(String),
    /// `compute_crop_specs` rejected the layout — typically missing physical mm.
    Layout(String),
    /// The wallpaper backend rejected the apply.
    Backend(String),
    /// Image load / decode / save failed.
    Image(String),
    /// Library scan / index / FS error.
    Library(String),
    /// Frontend supplied an invalid argument that the Rust side caught early.
    InvalidArgument(String),
    /// Catch-all for anything we couldn't classify.
    Internal(String),
}

impl IpcError {
    pub fn invalid<M: Into<String>>(m: M) -> Self {
        Self::InvalidArgument(m.into())
    }

    pub fn internal<M: Into<String>>(m: M) -> Self {
        Self::Internal(m.into())
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daemon(m) => write!(f, "daemon: {m}"),
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Detect(m) => write!(f, "detect: {m}"),
            Self::Layout(m) => write!(f, "layout: {m}"),
            Self::Backend(m) => write!(f, "backend: {m}"),
            Self::Image(m) => write!(f, "image: {m}"),
            Self::Library(m) => write!(f, "library: {m}"),
            Self::InvalidArgument(m) => write!(f, "invalid argument: {m}"),
            Self::Internal(m) => write!(f, "internal: {m}"),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<superpanels_core::ConfigError> for IpcError {
    fn from(e: superpanels_core::ConfigError) -> Self {
        Self::Config(e.to_string())
    }
}

impl From<superpanels_core::DetectError> for IpcError {
    fn from(e: superpanels_core::DetectError) -> Self {
        Self::Detect(e.to_string())
    }
}

impl From<superpanels_core::LayoutError> for IpcError {
    fn from(e: superpanels_core::LayoutError) -> Self {
        Self::Layout(e.to_string())
    }
}

impl From<superpanels_core::BackendError> for IpcError {
    fn from(e: superpanels_core::BackendError) -> Self {
        Self::Backend(e.to_string())
    }
}

impl From<superpanels_core::LibraryError> for IpcError {
    fn from(e: superpanels_core::LibraryError) -> Self {
        Self::Library(e.to_string())
    }
}

impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on serialiser bugs
mod tests {
    use super::*;

    #[test]
    fn invalid_argument_serialises_with_kind_and_message() {
        let err = IpcError::invalid("bad thing");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"InvalidArgument","message":"bad thing"}"#);
    }

    #[test]
    fn config_error_round_trips_via_from() {
        let e: IpcError = superpanels_core::ConfigError::NoConfigDir.into();
        assert!(matches!(e, IpcError::Config(_)));
    }
}
