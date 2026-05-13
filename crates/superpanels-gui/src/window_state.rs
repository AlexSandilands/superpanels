//! Persist window position and size to `state.json`.
//!
//! Tauri-side state lives next to the user's runtime files at
//! `$XDG_STATE_HOME/superpanels/window.json` (or `~/.local/state/...`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{App, Manager, PhysicalPosition, PhysicalSize, Window};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct WindowState {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) x: Option<i32>,
    pub(crate) y: Option<i32>,
    pub(crate) maximized: bool,
}

const FILE_NAME: &str = "window.json";

pub(crate) fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|h| h.join(".local").join("state"))
        })?;
    Some(base.join("superpanels"))
}

pub(crate) fn state_path() -> Option<PathBuf> {
    state_dir().map(|d| d.join(FILE_NAME))
}

pub(crate) fn load_from(path: &Path) -> WindowState {
    let Ok(bytes) = fs::read(path) else {
        return WindowState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub(crate) fn save_to(path: &Path, state: &WindowState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, body)
}

/// Apply a saved window state (size, position, maximized) to the main window.
pub(crate) fn restore(app: &App) {
    let Some(path) = state_path() else { return };
    let state = load_from(&path);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let (Some(w), Some(h)) = (state.width, state.height) {
        let _ = window.set_size(PhysicalSize::new(w, h));
    }
    if let (Some(x), Some(y)) = (state.x, state.y) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    if state.maximized {
        let _ = window.maximize();
    }
}

/// Snapshot the window's current geometry into `state.json`. Called on close.
pub(crate) fn persist(window: &Window) -> std::io::Result<()> {
    let Some(path) = state_path() else {
        return Ok(());
    };
    let size = window.inner_size().ok();
    let pos = window.outer_position().ok();
    let maximized = window.is_maximized().unwrap_or(false);
    let state = WindowState {
        width: size.map(|s| s.width),
        height: size.map(|s| s.height),
        x: pos.map(|p| p.x),
        y: pos.map(|p| p.y),
        maximized,
    };
    save_to(&path, &state)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let s = WindowState {
            width: Some(1280),
            height: Some(800),
            x: Some(100),
            y: Some(50),
            maximized: true,
        };
        save_to(&path, &s).unwrap();
        let back = load_from(&path);
        assert_eq!(back.width, s.width);
        assert_eq!(back.height, s.height);
        assert_eq!(back.x, s.x);
        assert_eq!(back.y, s.y);
        assert_eq!(back.maximized, s.maximized);
    }

    #[test]
    fn load_from_missing_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let s = load_from(&path);
        assert!(s.width.is_none());
        assert!(s.height.is_none());
        assert!(!s.maximized);
    }

    #[test]
    fn load_from_garbage_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"not json").unwrap();
        let s = load_from(&path);
        assert!(s.width.is_none());
    }
}
