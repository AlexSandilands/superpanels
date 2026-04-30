//! Shared mutable state held across all daemon tasks.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use superpanels_core::config::{Config, ConfigError, LibraryConfig};
use superpanels_core::display::Monitor;
use superpanels_core::ipc::{RuntimeState, SlideshowSummary};
use superpanels_core::library::{LibraryEntry, load_index, scan_folder};
use superpanels_core::slideshow::{SlideshowPicker, load_state};
use superpanels_core::{detect, ipc};
use tracing::{debug, info, warn};

use crate::schedule::ScheduleChecker;

pub(crate) struct DaemonState {
    pub config: Config,
    /// Explicit override from `--config <PATH>`. `None` means use
    /// [`Config::default_path`]. Owned by the daemon so write-side IPC
    /// handlers don't accept a client-supplied destination.
    pub config_path: Option<PathBuf>,
    pub monitors: Vec<Monitor>,
    pub library: Vec<LibraryEntry>,
    pub active_profile: Option<String>,
    /// Picker for the currently active profile's slideshow; `None` when the
    /// active profile has no slideshow source.
    pub slideshow_picker: Option<SlideshowPicker>,
    pub last_apply_unix_secs: Option<u64>,
    pub schedule_checker: ScheduleChecker,
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

        let library = load_library_index(&config.library);

        Ok(Self {
            config,
            config_path: config_path.map(Path::to_path_buf),
            monitors,
            library,
            active_profile: None,
            slideshow_picker: None,
            last_apply_unix_secs: None,
            schedule_checker: ScheduleChecker::new(),
        })
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

    /// Rescan library roots and update in-memory index.
    pub(crate) fn rescan_library(&mut self) {
        let roots: Vec<PathBuf> = self.config.library.roots.clone();
        let recursive = self.config.library.recursive;
        let mut entries = Vec::new();
        for root in &roots {
            let batch = scan_folder(root, recursive, |_| {});
            info!(root = %root.display(), found = batch.len(), "library scan complete");
            entries.extend(batch);
        }
        self.library = entries;
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
            active_profile: None,
            slideshow_picker: None,
            last_apply_unix_secs: None,
            schedule_checker: ScheduleChecker::new(),
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

fn load_library_index(cfg: &LibraryConfig) -> Vec<LibraryEntry> {
    let Some(state_dir) = DaemonState::state_dir() else {
        return Vec::new();
    };
    let index_path = state_dir.join("library-index.json");
    match load_index(&index_path) {
        Ok(entries) => {
            debug!(path = %index_path.display(), count = entries.len(), "loaded library index");
            entries
        }
        Err(e) => {
            warn!(error = %e, "could not load library index; will rescan");
            let mut entries = Vec::new();
            for root in &cfg.roots {
                entries.extend(scan_folder(root, cfg.recursive, |_| {}));
            }
            entries
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same as above
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use superpanels_core::config::{
        BackendKind, ImageSet, Profile, ProfileBody, SlideshowConfig as SlideshowCfg,
        SlideshowSort, SlideshowStart, SpanProfile, SpanSource,
    };
    use superpanels_core::layout::{BezelConfig, FitMode};
    use superpanels_core::slideshow::{
        SlideshowConfig as PickerCfg, SlideshowPicker, SlideshowSort as PickerSort,
        SlideshowStart as PickerStart, persist_state,
    };
    use tempfile::tempdir;

    use super::*;

    fn slideshow_profile(name: &str) -> Profile {
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
                fit: FitMode::Fill,
                offset: [0, 0],
            }),
            bezels: BezelConfig {
                horizontal_mm: 0.0,
                vertical_mm: 0.0,
            },
            backend_override: Some(BackendKind::Custom),
            schedule: None,
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
}
