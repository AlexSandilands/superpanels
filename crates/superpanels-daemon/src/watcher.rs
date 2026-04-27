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
    do_rescan_with_state_dir(state, DaemonState::state_dir().as_deref());
}

fn do_rescan_with_state_dir(state: &Arc<Mutex<DaemonState>>, state_dir: Option<&std::path::Path>) {
    // This runs in a spawn_blocking context — blocking is fine.
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let mut guard = state.lock().await;
        guard.rescan_library();
        info!(
            count = guard.library.len(),
            "library rescanned after FS event"
        );
        if let Some(dir) = state_dir {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use std::path::Path;

    use superpanels_core::config::{Config, LibraryConfig};
    use tempfile::tempdir;

    use super::*;

    fn write_dummy_image(path: &Path) {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img).save(path).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rescan_picks_up_added_and_removed_files() {
        // Arrange — library root with 3 images. The state_dir is overridden via
        // the test-only `do_rescan_with_state_dir` helper so this test never
        // touches XDG_STATE_HOME.
        let lib_dir = tempdir().unwrap();
        let state_dir = tempdir().unwrap();

        let a = lib_dir.path().join("a.png");
        let b = lib_dir.path().join("b.png");
        let c = lib_dir.path().join("c.png");
        write_dummy_image(&a);
        write_dummy_image(&b);
        write_dummy_image(&c);

        let config = Config {
            library: LibraryConfig {
                roots: vec![lib_dir.path().to_path_buf()],
                recursive: false,
                thumbnail_size: 320,
                auto_scan: true,
            },
            ..Default::default()
        };
        let state = Arc::new(Mutex::new(DaemonState::for_tests(config)));

        // Act 1 — initial rescan picks up all three files.
        let s1 = Arc::clone(&state);
        let sd1 = state_dir.path().to_path_buf();
        tokio::task::spawn_blocking(move || do_rescan_with_state_dir(&s1, Some(&sd1)))
            .await
            .unwrap();
        let count_after_first = state.lock().await.library.len();
        assert_eq!(count_after_first, 3);

        // Mutate — remove one, add one.
        std::fs::remove_file(&c).unwrap();
        let d = lib_dir.path().join("d.png");
        write_dummy_image(&d);

        // Act 2 — rescan reflects the mutation.
        let s2 = Arc::clone(&state);
        let sd2 = state_dir.path().to_path_buf();
        tokio::task::spawn_blocking(move || do_rescan_with_state_dir(&s2, Some(&sd2)))
            .await
            .unwrap();
        let count_after_second = state.lock().await.library.len();
        assert_eq!(count_after_second, 3);
        let paths: Vec<PathBuf> = state
            .lock()
            .await
            .library
            .iter()
            .map(|e| e.path.clone())
            .collect();
        assert!(paths.contains(&d), "expected d.png in rescanned index");
        assert!(
            !paths.contains(&c),
            "expected c.png to be gone after removal"
        );

        // The persisted library-index.json should exist under our temp state dir.
        assert!(state_dir.path().join("library-index.json").exists());
    }
}
