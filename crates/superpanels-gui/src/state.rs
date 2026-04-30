//! Process-wide app state held in Tauri's manager. Tracks the runtime snapshot
//! we surface to the tray (active profile + last filename) and a cached config
//! path so the in-process fallback uses the same file the daemon would.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct AppState {
    /// Most recent snapshot of `current_state` from the daemon (or in-process).
    pub last_runtime: Mutex<Option<RuntimeSnapshot>>,
    /// Optional override for the config path; `None` means use the XDG default.
    pub config_path: Mutex<Option<PathBuf>>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSnapshot {
    pub active_profile: Option<String>,
    pub current_filename: Option<String>,
    pub paused: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        self.config_path.lock().ok().and_then(|g| g.clone())
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.last_runtime
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn set_snapshot(&self, snap: RuntimeSnapshot) {
        if let Ok(mut g) = self.last_runtime.lock() {
            *g = Some(snap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_default_until_set() {
        let s = AppState::new();
        let snap = s.snapshot();
        assert!(snap.active_profile.is_none());
        assert!(!snap.paused);
    }

    #[test]
    fn set_snapshot_round_trips() {
        let s = AppState::new();
        s.set_snapshot(RuntimeSnapshot {
            active_profile: Some("home".into()),
            current_filename: Some("a.png".into()),
            paused: true,
        });
        let got = s.snapshot();
        assert_eq!(got.active_profile.as_deref(), Some("home"));
        assert_eq!(got.current_filename.as_deref(), Some("a.png"));
        assert!(got.paused);
    }
}
