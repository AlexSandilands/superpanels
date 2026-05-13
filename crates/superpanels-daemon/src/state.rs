//! Shared mutable state held across all daemon tasks.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use superpanels_core::config::{Config, ConfigError, LibraryConfig};
use superpanels_core::display::Monitor;
use superpanels_core::ipc::{RuntimeState, SlideshowSummary};
use superpanels_core::library::{
    FolderWatcher, LibraryDb, LibraryEntry, migrate_json_to_sqlite, scan_folder,
};
use superpanels_core::slideshow::{SlideshowPicker, load_state};
use superpanels_core::{detect, ipc};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

use crate::schedule::ScheduleChecker;
use crate::thumbnail_cache::ThumbnailCache;

pub(crate) struct DaemonState {
    pub config: Config,
    /// Explicit override from `--config <PATH>`. `None` means use
    /// [`Config::default_path`]. Owned by the daemon so write-side IPC
    /// handlers don't accept a client-supplied destination.
    pub config_path: Option<PathBuf>,
    pub monitors: Vec<Monitor>,
    /// In-memory cache of library entries hydrated from [`library_db`]. Reads
    /// (filtering, slideshow pool selection) hit this directly so the daemon
    /// avoids a round trip to `SQLite` on every IPC call. Writes go through the
    /// DB and then refresh this vector.
    pub library: Vec<LibraryEntry>,
    /// ``SQLite`` library index (`SPEC §14.5`). `None` when the DB couldn't be
    /// opened — the daemon still serves library reads from the cached vector.
    pub library_db: Option<LibraryDb>,
    pub active_profile: Option<String>,
    /// Picker for the currently active profile's slideshow; `None` when the
    /// active profile has no slideshow source.
    pub slideshow_picker: Option<SlideshowPicker>,
    pub last_apply_unix_secs: Option<u64>,
    pub schedule_checker: ScheduleChecker,
    /// Inotify-backed FS watcher over [`LibraryConfig::roots`]. Rebuilt by
    /// [`Self::refresh_watcher`] whenever the configured roots change so a
    /// freshly-added folder starts auto-rescanning without a daemon restart.
    pub watcher: Option<FolderWatcher>,
    /// Sender wired into the watcher task; required to (re)build [`watcher`].
    /// Set once at daemon startup; `None` only inside test fixtures.
    pub watcher_tx: Option<UnboundedSender<notify::Event>>,
    /// LRU cache of `cmd_library_thumbnail` outputs, keyed on
    /// (canonicalised path, mtime). See [`ThumbnailCache`] for the bounds.
    pub thumbnail_cache: ThumbnailCache,
    /// Broadcast sender fired when the OS pushes a display-config change
    /// (KDE kscreen kded `configChanged`). Subscribers receive `()` ticks
    /// and are expected to pull a fresh snapshot via `current_state` IPC —
    /// the channel deliberately doesn't carry monitor data so a slow reader
    /// can't observe a stale list. No live subscribers today; the daemon→GUI
    /// relay is a tracked follow-up (see `docs/followups.md`).
    pub monitors_tx: Option<broadcast::Sender<()>>,
}

impl DaemonState {
    /// Load config, detect monitors, load library index from disk.
    pub(crate) fn load(config_path: Option<&std::path::Path>) -> Result<Self> {
        let config = match config_path {
            Some(p) => Config::load_from(p).context("loading config")?,
            None => Config::load_or_default().context("loading config")?,
        };

        let mut monitors = detect(None).unwrap_or_else(|e| {
            warn!(error = %e, "monitor detection failed; starting with empty layout");
            Vec::new()
        });
        config.merge_into_monitors(&mut monitors);

        let (library_db, library) = load_library_db_and_entries(&config.library);

        Ok(Self {
            config,
            config_path: config_path.map(Path::to_path_buf),
            monitors,
            library,
            library_db,
            active_profile: None,
            slideshow_picker: None,
            last_apply_unix_secs: None,
            schedule_checker: ScheduleChecker::new(),
            watcher: None,
            watcher_tx: None,
            thumbnail_cache: ThumbnailCache::new(),
            monitors_tx: None,
        })
    }

    /// Tear down any existing FS watcher and rebuild one over the current
    /// `config.library.roots`. Called once at startup after `watcher_tx` is
    /// wired in and again from `save_config` whenever the root list diffs, so
    /// auto-rescan picks up freshly-added folders without a daemon restart.
    pub(crate) fn refresh_watcher(&mut self) {
        let Some(tx) = self.watcher_tx.clone() else {
            return;
        };
        let roots = self.config.library.roots.clone();
        // Drop the old watcher first so its forwarder thread exits before the
        // new one starts pushing events on the same `tx`.
        self.watcher = None;
        if roots.is_empty() {
            return;
        }
        match crate::watcher::make_watcher(&roots, tx) {
            Ok(w) => {
                debug!(roots = roots.len(), "FS watcher refreshed");
                self.watcher = Some(w);
            }
            Err(e) => warn!(error = %e, "FS watcher refresh failed; auto-rescan disabled"),
        }
    }

    /// Path the daemon writes config to. Honours an explicit `--config`
    /// override, otherwise resolves the XDG default.
    pub(crate) fn config_save_path(&self) -> Result<PathBuf, ConfigError> {
        match &self.config_path {
            Some(p) => Ok(p.clone()),
            None => Config::default_path(),
        }
    }

    /// Snapshot suitable for the `current_state` IPC response.
    pub(crate) fn to_runtime_state(&self) -> RuntimeState {
        let slideshow = self.slideshow_picker.as_ref().map(|p| {
            let s = p.state();
            SlideshowSummary {
                current_index: s.current_index,
                history_len: s.history.len(),
                paused: s.paused,
            }
        });
        RuntimeState {
            version: ipc::PROTOCOL_VERSION,
            active_profile: self.active_profile.clone(),
            slideshow,
            last_apply_unix_secs: self.last_apply_unix_secs,
        }
    }

    /// Restore the slideshow picker for `profile_name` from persisted state.
    pub(crate) fn restore_slideshow(&mut self, profile_name: &str, state_path: &std::path::Path) {
        use superpanels_core::config::{ProfileBody, SpanSource};

        let profile = self
            .config
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .cloned();

        let Some(profile) = profile else {
            self.slideshow_picker = None;
            return;
        };

        let slideshow_cfg = match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Slideshow { config, .. } => Some(config.clone()),
                SpanSource::Single { .. } => None,
            },
            ProfileBody::PerMonitor(_) => None,
        };

        let Some(cfg) = slideshow_cfg else {
            self.slideshow_picker = None;
            return;
        };

        let saved_state = load_state(state_path).unwrap_or_default();
        let picker_cfg = crate::apply::profile_to_picker_config(&cfg);
        self.slideshow_picker = Some(SlideshowPicker::with_state(picker_cfg, saved_state));
        debug!(profile = profile_name, "restored slideshow picker");
    }

    /// Rescan library roots, persist to `SQLite`, and refresh the in-memory cache.
    /// On any DB failure the daemon falls back to the freshly scanned entries
    /// (with empty per-image metadata) so reads still work.
    pub(crate) fn rescan_library(&mut self) {
        let roots: Vec<PathBuf> = self.config.library.roots.clone();
        let recursive = self.config.library.recursive;
        let mut entries = Vec::new();
        for root in &roots {
            let batch = scan_folder(root, recursive, |_| {});
            info!(root = %root.display(), found = batch.len(), "library scan complete");
            entries.extend(batch);
        }
        self.library = match self.library_db.as_mut() {
            Some(db) => match db.replace_entries_preserving_metadata(&entries) {
                Ok(()) => db.list_entries().unwrap_or_else(|e| {
                    warn!(error = %e, "library DB read after rescan failed; using bare scan");
                    entries
                }),
                Err(e) => {
                    warn!(error = %e, "library DB write during rescan failed; using bare scan");
                    entries
                }
            },
            None => entries,
        };
    }

    /// Unix seconds for the current time.
    pub(crate) fn now_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    /// Interval for the active profile's slideshow; `None` when there is none.
    pub(crate) fn active_slideshow_interval(&self) -> Option<Duration> {
        use superpanels_core::config::{ProfileBody, SpanSource};

        let name = self.active_profile.as_deref()?;
        let profile = self.config.profiles.iter().find(|p| p.name == name)?;
        match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Slideshow { config, .. } => Some(config.interval),
                SpanSource::Single { .. } => None,
            },
            ProfileBody::PerMonitor(_) => None,
        }
    }

    /// State-dir path: `$XDG_STATE_HOME/superpanels/` (or `~/.local/state/superpanels/`).
    pub(crate) fn state_dir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
            let p = PathBuf::from(dir);
            if !p.as_os_str().is_empty() {
                return Some(p.join("superpanels"));
            }
        }
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("superpanels"),
        )
    }
}

#[cfg(test)]
impl DaemonState {
    /// Test constructor: build a state directly from in-memory pieces, bypassing
    /// XDG config / state-dir lookup. Production code should use [`Self::load`].
    pub(crate) fn for_tests(config: Config) -> Self {
        Self {
            config,
            config_path: None,
            monitors: Vec::new(),
            library: Vec::new(),
            library_db: None,
            active_profile: None,
            slideshow_picker: None,
            last_apply_unix_secs: None,
            schedule_checker: ScheduleChecker::new(),
            watcher: None,
            watcher_tx: None,
            thumbnail_cache: ThumbnailCache::new(),
            monitors_tx: None,
        }
    }

    /// Like [`Self::for_tests`] but with an explicit on-disk config target,
    /// used by tests for write-side handlers (`save_profile`, `delete_profile`,
    /// `save_config`).
    pub(crate) fn for_tests_with_path(config: Config, path: PathBuf) -> Self {
        Self {
            config_path: Some(path),
            ..Self::for_tests(config)
        }
    }
}

/// Open the library DB, run the JSON→`SQLite` migration if a legacy index file
/// is present, and return the hydrated in-memory cache. On any failure the
/// daemon degrades gracefully to a one-shot folder scan so reads still work
/// — the library is non-critical state.
fn load_library_db_and_entries(cfg: &LibraryConfig) -> (Option<LibraryDb>, Vec<LibraryEntry>) {
    let Some(state_dir) = DaemonState::state_dir() else {
        warn!("no state dir; library DB disabled");
        return (None, Vec::new());
    };
    if let Err(e) = std::fs::create_dir_all(&state_dir) {
        warn!(error = %e, "could not create state dir; library DB disabled");
        return (None, Vec::new());
    }
    let data_dir = library_data_dir().unwrap_or_else(|| state_dir.clone());
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        warn!(error = %e, "could not create library data dir");
    }
    let db_path = data_dir.join("library.db");

    let mut db = match LibraryDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            warn!(error = %e, "library DB unavailable; falling back to in-memory only");
            return (None, fresh_scan(cfg));
        }
    };

    // One-shot migration from the Phase-2 JSON index.
    let json_path = state_dir.join("library-index.json");
    if let Err(e) = migrate_json_to_sqlite(&json_path, &mut db) {
        warn!(error = %e, "JSON→`SQLite` migration failed; existing DB preserved");
    }

    let entries = match db.list_entries() {
        Ok(rows) if !rows.is_empty() => {
            debug!(count = rows.len(), "library DB hydrated");
            rows
        }
        Ok(_) => {
            // Empty DB — first run on this binary. Run a synchronous scan so
            // the library isn't blank until the FS watcher fires.
            let scanned = fresh_scan(cfg);
            if let Err(e) = db.replace_entries_preserving_metadata(&scanned) {
                warn!(error = %e, "library DB seed write failed; serving scan from memory");
            }
            db.list_entries().unwrap_or(scanned)
        }
        Err(e) => {
            warn!(error = %e, "library DB read failed; running fresh scan");
            fresh_scan(cfg)
        }
    };
    (Some(db), entries)
}

fn fresh_scan(cfg: &LibraryConfig) -> Vec<LibraryEntry> {
    let mut entries = Vec::new();
    for root in &cfg.roots {
        entries.extend(scan_folder(root, cfg.recursive, |_| {}));
    }
    entries
}

/// `$XDG_DATA_HOME/superpanels/` (or `~/.local/share/superpanels/`). Library
/// DB lives here per `SPEC §14.5`; falls back to the state dir when neither
/// XDG var is set.
fn library_data_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return Some(p.join("superpanels"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("superpanels"),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same as above
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use std::collections::HashMap;
    use superpanels_core::TopologyFingerprint;
    use superpanels_core::config::{
        BackendKind, ImageSet, Profile, ProfileBody, SlideshowConfig as SlideshowCfg,
        SlideshowSort, SlideshowStart, SpanProfile, SpanSource,
    };
    use superpanels_core::layout::ImageRectMm;
    use superpanels_core::slideshow::{
        SlideshowConfig as PickerCfg, SlideshowPicker, SlideshowSort as PickerSort,
        SlideshowStart as PickerStart, persist_state,
    };
    use tempfile::tempdir;

    use super::*;

    fn slideshow_profile(name: &str) -> Profile {
        let now = superpanels_core::config::now_timestamp();
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Slideshow {
                    images: ImageSet::Folder {
                        path: PathBuf::from("/walls"),
                        recursive: false,
                    },
                    config: SlideshowCfg {
                        interval: Duration::from_secs(60),
                        sort: SlideshowSort::Alphabetical,
                        recent_history_size: 10,
                        on_start: SlideshowStart::Resume,
                        pause_when_active: false,
                        skip_on_unavailable: true,
                    },
                },
                image_rect_mm: ImageRectMm::default(),
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: Some(BackendKind::Custom),
        }
    }

    #[test]
    fn restore_slideshow_round_trips_history_via_disk() {
        // Arrange — persist a picker state with non-empty history, then load
        // it back through DaemonState::restore_slideshow.
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("slideshow-state.json");

        let mut picker = SlideshowPicker::new(PickerCfg {
            interval: Duration::from_secs(60),
            sort: PickerSort::Alphabetical,
            recent_history_size: 10,
            on_start: PickerStart::Resume,
            pause_when_active: false,
            skip_on_unavailable: true,
        });
        picker
            .state_mut()
            .history
            .push_front(PathBuf::from("/walls/a.png"));
        picker
            .state_mut()
            .history
            .push_front(PathBuf::from("/walls/b.png"));
        picker.state_mut().current_index = Some(1);
        persist_state(picker.state(), &state_path).unwrap();

        let mut config = Config::default();
        config.profiles.push(slideshow_profile("p"));
        let mut state = DaemonState::for_tests(config);

        // Act
        state.restore_slideshow("p", &state_path);

        // Assert
        let restored = state.slideshow_picker.expect("picker was not restored");
        let s = restored.state();
        assert_eq!(s.history.len(), 2);
        assert_eq!(
            s.history.front().cloned(),
            Some(PathBuf::from("/walls/b.png"))
        );
        assert_eq!(s.current_index, Some(1));
    }

    #[test]
    fn refresh_watcher_is_noop_without_tx() {
        // for_tests leaves watcher_tx as None — refresh must do nothing rather
        // than panic so the rest of the suite can use the test fixture freely.
        let mut state = DaemonState::for_tests(Config::default());
        state.refresh_watcher();
        assert!(state.watcher.is_none());
    }

    #[tokio::test]
    async fn refresh_watcher_builds_when_tx_set_and_drops_when_roots_empty() {
        use superpanels_core::config::LibraryConfig;
        use tokio::sync::mpsc::unbounded_channel;

        // Arrange — give state a tx and a real (empty) root dir to watch.
        let lib_dir = tempdir().unwrap();
        let config = Config {
            library: LibraryConfig {
                roots: vec![lib_dir.path().to_path_buf()],
                recursive: false,
                thumbnail_size: 320,
                auto_scan: true,
            },
            ..Default::default()
        };
        let mut state = DaemonState::for_tests(config);
        let (tx, _rx) = unbounded_channel::<notify::Event>();
        state.watcher_tx = Some(tx);

        // Act 1 — refresh builds a watcher over the configured root.
        state.refresh_watcher();
        assert!(
            state.watcher.is_some(),
            "expected watcher to be built when roots are configured and tx is set"
        );

        // Act 2 — clearing roots and refreshing tears the watcher down.
        state.config.library.roots.clear();
        state.refresh_watcher();
        assert!(
            state.watcher.is_none(),
            "expected watcher to be torn down when roots are emptied"
        );
    }
}
