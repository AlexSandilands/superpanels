//! FS watcher task: debounces inotify events and triggers library rescans.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use superpanels_core::library::{FolderWatcher, LibraryError, persist_index};
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, info, warn};

use crate::state::DaemonState;

/// Debounce window: wait this long after the last FS event before rescanning.
const DEBOUNCE_MS: u64 = 2_000;

/// Drives library rescans from FS-watcher events.
///
/// `event_rx` receives raw [`notify::Event`] items forwarded by the watcher
/// callbacks. The task debounces them with a 2-second quiet window to avoid
/// re-scanning on every file write during a bulk copy.
pub(crate) async fn run_watcher(
    state: Arc<Mutex<DaemonState>>,
    mut event_rx: UnboundedReceiver<notify::Event>,
) {
    loop {
        // Wait for the first event.
        if event_rx.recv().await.is_none() {
            return; // channel closed; daemon is shutting down
        }

        // Drain further events for the debounce window.
        let deadline = tokio::time::Instant::now() + Duration::from_millis(DEBOUNCE_MS);
        loop {
            match tokio::time::timeout_at(deadline, event_rx.recv()).await {
                Ok(Some(_)) => {}   // more events; keep draining
                Ok(None) => return, // channel closed
                Err(_) => break,    // timeout reached; quiet period over
            }
        }

        debug!("FS quiet period over; triggering library rescan");
        let state_clone = Arc::clone(&state);
        tokio::task::spawn_blocking(move || {
            do_rescan(&state_clone);
        });
    }
}

fn do_rescan(state: &Arc<Mutex<DaemonState>>) {
    // This runs in a spawn_blocking context — blocking is fine.
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let mut guard = state.lock().await;
        guard.rescan_library();
        info!(
            count = guard.library.len(),
            "library rescanned after FS event"
        );
        if let Some(dir) = DaemonState::state_dir() {
            let index_path = dir.join("library-index.json");
            if let Err(e) = persist_index(&guard.library, &index_path) {
                warn!(error = %e, "failed to persist library index after rescan");
            }
        }
    });
}

/// Build a [`FolderWatcher`] that forwards events onto `tx`.
pub(crate) fn make_watcher(
    roots: &[PathBuf],
    tx: tokio::sync::mpsc::UnboundedSender<notify::Event>,
) -> Result<FolderWatcher, LibraryError> {
    // `notify` requires a `std::sync::mpsc::Sender`; we bridge to tokio.
    let (std_tx, std_rx) = std::sync::mpsc::channel::<notify::Event>();
    let watcher = FolderWatcher::new(roots, std_tx)?;

    // Forward thread: transfers events from the std channel to the tokio channel.
    std::thread::spawn(move || {
        while let Ok(event) = std_rx.recv() {
            if tx.send(event).is_err() {
                break; // receiver dropped
            }
        }
    });

    Ok(watcher)
}
