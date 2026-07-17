//! Process-wide app state held in Tauri's manager. Tracks the runtime snapshot
//! we surface to the tray (active profile + last filename) and a cached config
//! path so the in-process fallback uses the same file the daemon would.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub(crate) struct AppState {
    /// Most recent snapshot of `current_state` from the daemon (or in-process).
    pub(crate) last_runtime: Mutex<Option<RuntimeSnapshot>>,
    /// Optional override for the config path; `None` means use the XDG default.
    pub(crate) config_path: Mutex<Option<PathBuf>>,
    /// Signal long-running background workers (currently only `tray::spawn_poller`)
    /// to exit cleanly. Set on `RunEvent::ExitRequested` so the poller doesn't
    /// outlive the Tauri runtime and dial into a torn-down daemon during shutdown.
    pub(crate) shutdown: AtomicBool,
    /// Set when tray "Settings…" rebuilds a torn-down window: the fresh page has
    /// no event listeners yet, so it drains this flag on boot (via
    /// `take_pending_open_settings`) instead of receiving `tray://open-settings`.
    pub(crate) pending_open_settings: AtomicBool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) active_profile: Option<String>,
    pub(crate) current_filename: Option<String>,
    pub(crate) paused: bool,
}

impl AppState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn config_path(&self) -> Option<PathBuf> {
        self.config_path.lock().ok().and_then(|g| g.clone())
    }

    pub(crate) fn snapshot(&self) -> RuntimeSnapshot {
        self.last_runtime
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default()
    }

    pub(crate) fn set_snapshot(&self, snap: RuntimeSnapshot) {
        if let Ok(mut g) = self.last_runtime.lock() {
            *g = Some(snap);
        }
    }

    pub(crate) fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    pub(crate) fn shutting_down(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    pub(crate) fn set_pending_open_settings(&self) {
        self.pending_open_settings.store(true, Ordering::SeqCst);
    }

    /// Read and clear the pending-settings flag. The boot handshake calls this
    /// exactly once per fresh page load, so a rebuild-for-settings opens the
    /// panel and any stale flag can't reopen it on the next launch.
    pub(crate) fn take_pending_open_settings(&self) -> bool {
        self.pending_open_settings.swap(false, Ordering::SeqCst)
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
    fn shutdown_flag_is_off_by_default_and_latches_on_request() {
        let s = AppState::new();
        assert!(!s.shutting_down());
        s.request_shutdown();
        assert!(s.shutting_down());
        // Idempotent — repeat requests don't unset.
        s.request_shutdown();
        assert!(s.shutting_down());
    }

    #[test]
    fn pending_open_settings_latches_then_drains_once() {
        let s = AppState::new();
        assert!(!s.take_pending_open_settings());
        s.set_pending_open_settings();
        // First drain sees it; a second must not — else settings would reopen
        // on the next fresh page load.
        assert!(s.take_pending_open_settings());
        assert!(!s.take_pending_open_settings());
    }

    #[test]
    fn pending_open_settings_consumed_by_live_event_then_boot_drain_is_false() {
        // Tray "Settings…" always stages the flag AND emits. On a live window
        // the event's `onOpenSettings` handler drains it; a later window rebuild
        // must then see a *cleared* flag on its boot handshake, or Settings
        // would spuriously reopen. Models: set -> event drain -> boot drain.
        let s = AppState::new();
        s.set_pending_open_settings();
        assert!(
            s.take_pending_open_settings(),
            "live event consumes the flag"
        );
        assert!(
            !s.take_pending_open_settings(),
            "next rebuilt window's boot handshake must not reopen settings"
        );
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
