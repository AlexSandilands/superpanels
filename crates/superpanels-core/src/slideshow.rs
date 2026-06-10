//! Slideshow picker. Pure synchronous logic — timing lives
//! in the daemon so the picker stays trivial to test.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rand::SeedableRng;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideshowConfig {
    /// Driven by the daemon; the picker doesn't read it.
    pub interval: Duration,
    pub sort: SlideshowSort,
    /// Suppress the last *N* shown images.
    pub recent_history_size: usize,
    pub on_start: SlideshowStart,
    /// Pause the timer while the user is interacting with the GUI.
    pub pause_when_active: bool,
    /// Skip vanished files at apply time instead of erroring.
    pub skip_on_unavailable: bool,
}

impl Default for SlideshowConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30 * 60),
            sort: SlideshowSort::Shuffle,
            recent_history_size: 10,
            on_start: SlideshowStart::Resume,
            pause_when_active: false,
            skip_on_unavailable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlideshowSort {
    Shuffle,
    Alphabetical,
    /// Pre-sorted by the daemon (it has the `LibraryEntry` mtime); the
    /// picker doesn't stat files.
    DateAsc,
    DateDesc,
    /// Least-recently-shown first.
    LastShownAsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlideshowStart {
    Resume,
    NewRandom,
    First,
}

/// Persisted at `$XDG_STATE_HOME/superpanels/state.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SlideshowState {
    pub current_index: Option<usize>,
    /// Newest first; bounded by `recent_history_size`.
    pub history: VecDeque<PathBuf>,
    pub paused: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlideshowError {
    #[error("slideshow pool is empty")]
    EmptyPool,
    /// Every candidate was in recent history or unavailable; caller should
    /// widen the pool, shrink history, or wait for the next scan.
    #[error("no eligible image: every candidate was in recent history or unavailable")]
    NoEligibleEntry,
}

pub struct SlideshowPicker {
    config: SlideshowConfig,
    state: SlideshowState,
    rng: rand::rngs::StdRng,
}

impl SlideshowPicker {
    pub fn new(config: SlideshowConfig) -> Self {
        Self {
            config,
            state: SlideshowState::default(),
            rng: rand::rngs::StdRng::from_os_rng(),
        }
    }

    /// `NewRandom` and `First` reset history before the first pick.
    pub fn with_state(config: SlideshowConfig, state: SlideshowState) -> Self {
        let state = match config.on_start {
            SlideshowStart::Resume => state,
            SlideshowStart::NewRandom | SlideshowStart::First => SlideshowState {
                current_index: None,
                history: VecDeque::new(),
                paused: state.paused,
            },
        };
        Self {
            config,
            state,
            rng: rand::rngs::StdRng::from_os_rng(),
        }
    }

    /// Deterministic RNG for tests / benches.
    #[cfg(test)]
    pub(crate) fn with_seeded_rng(mut self, seed: u64) -> Self {
        self.rng = rand::rngs::StdRng::seed_from_u64(seed);
        self
    }

    pub fn state(&self) -> &SlideshowState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut SlideshowState {
        &mut self.state
    }

    /// Swap the config on a live picker, keeping history and pause state —
    /// unlike [`Self::with_state`], `on_start` is *not* re-applied. History is
    /// re-trimmed when the new window is smaller.
    pub fn update_config(&mut self, config: SlideshowConfig) {
        self.config = config;
        while self.state.history.len() > self.history_capacity() {
            self.state.history.pop_back();
        }
    }

    /// History always retains at least the current image — it feeds
    /// current-path reporting and `step_back` — even when the
    /// repeat-suppression window (`recent_history_size`) is 0.
    fn history_capacity(&self) -> usize {
        self.config.recent_history_size.max(1)
    }

    /// Step back to the previously shown image: drops the newest history
    /// entry and returns the new front, which is now the current image.
    /// `None` when there is nothing to go back to.
    pub fn step_back(&mut self) -> Option<PathBuf> {
        if self.state.history.len() < 2 {
            return None;
        }
        self.state.history.pop_front();
        // The pool position of the restored image is unknown without the
        // pool; the next advance re-derives it from history.
        self.state.current_index = None;
        self.state.history.front().cloned()
    }

    /// Pick the next path, recording it in `state.history`. If every entry
    /// is in recent history, returns [`SlideshowError::NoEligibleEntry`] —
    /// the daemon decides whether to widen the pool or shrink history.
    pub fn next(&mut self, pool: &[PathBuf]) -> Result<PathBuf, SlideshowError> {
        if pool.is_empty() {
            return Err(SlideshowError::EmptyPool);
        }
        let ordered = sorted_pool(pool, self.config.sort);
        // Only the configured window suppresses repeats; history may hold one
        // extra entry (the current image) when the window is 0.
        let history: std::collections::HashSet<PathBuf> = self
            .state
            .history
            .iter()
            .take(self.config.recent_history_size)
            .cloned()
            .collect();

        let choice = match self.config.sort {
            SlideshowSort::Shuffle => self.pick_shuffle(&ordered, &history),
            _ => self.pick_sequential(&ordered, &history),
        };

        let path = choice.ok_or(SlideshowError::NoEligibleEntry)?;
        self.record(&path, &ordered);
        Ok(path)
    }

    fn pick_shuffle(
        &mut self,
        ordered: &[PathBuf],
        history: &std::collections::HashSet<PathBuf>,
    ) -> Option<PathBuf> {
        let eligible: Vec<&PathBuf> = ordered
            .iter()
            .filter(|p| !history.contains(*p))
            .filter(|p| Self::path_available(self.config.skip_on_unavailable, p))
            .collect();
        if eligible.is_empty() {
            let fallback: Vec<&PathBuf> = ordered
                .iter()
                .filter(|p| Self::path_available(self.config.skip_on_unavailable, p))
                .collect();
            return fallback.choose(&mut self.rng).map(|p| (*p).clone());
        }
        eligible.choose(&mut self.rng).map(|p| (*p).clone())
    }

    fn pick_sequential(
        &self,
        ordered: &[PathBuf],
        history: &std::collections::HashSet<PathBuf>,
    ) -> Option<PathBuf> {
        // History front is the current image and survives `step_back`;
        // `current_index` is the fallback for state persisted before history
        // tracked the current entry.
        let last = self.state.history.front().cloned().or_else(|| {
            self.state
                .current_index
                .and_then(|i| ordered.get(i).cloned())
        });
        let start = match last {
            Some(prev) => ordered.iter().position(|p| p == &prev).map_or(0, |i| i + 1),
            None => 0,
        };
        for offset in 0..ordered.len() {
            let idx = (start + offset) % ordered.len();
            let candidate = &ordered[idx];
            if history.contains(candidate) {
                continue;
            }
            if !Self::path_available(self.config.skip_on_unavailable, candidate) {
                continue;
            }
            return Some(candidate.clone());
        }
        // Every entry is in history. Walk again allowing history but still
        // honouring availability, so we keep advancing rather than stalling.
        for offset in 0..ordered.len() {
            let idx = (start + offset) % ordered.len();
            let candidate = &ordered[idx];
            if Self::path_available(self.config.skip_on_unavailable, candidate) {
                return Some(candidate.clone());
            }
        }
        None
    }

    fn path_available(skip_on_unavailable: bool, path: &Path) -> bool {
        if skip_on_unavailable {
            path.exists()
        } else {
            true
        }
    }

    fn record(&mut self, chosen: &Path, ordered: &[PathBuf]) {
        if let Some(idx) = ordered.iter().position(|p| p == chosen) {
            self.state.current_index = Some(idx);
        }
        self.state.history.push_front(chosen.to_path_buf());
        while self.state.history.len() > self.history_capacity() {
            self.state.history.pop_back();
        }
    }
}

fn sorted_pool(pool: &[PathBuf], sort: SlideshowSort) -> Vec<PathBuf> {
    let mut copy = pool.to_vec();
    match sort {
        SlideshowSort::Alphabetical => copy.sort(),
        // The daemon supplies the pool already ordered for date / last_shown
        // sorts (it has the LibraryEntry metadata); the picker just walks it.
        // For Shuffle, ordering is irrelevant — the choose-from-rng step
        // does the work.
        SlideshowSort::DateAsc
        | SlideshowSort::DateDesc
        | SlideshowSort::LastShownAsc
        | SlideshowSort::Shuffle => {}
    }
    copy
}

/// Writes via `<path>.tmp` then renames, so a crash never leaves a
/// half-written file.
pub fn persist_state(state: &SlideshowState, path: &Path) -> Result<(), SlideshowIoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| SlideshowIoError {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_vec_pretty(state).map_err(|source| SlideshowIoError {
        path: path.to_path_buf(),
        source: std::io::Error::other(source),
    })?;
    std::fs::write(&tmp, &json).map_err(|source| SlideshowIoError {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| SlideshowIoError {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Returns `Default::default()` when `path` doesn't exist.
pub fn load_state(path: &Path) -> Result<SlideshowState, SlideshowIoError> {
    if !path.exists() {
        return Ok(SlideshowState::default());
    }
    let bytes = std::fs::read(path).map_err(|source| SlideshowIoError {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SlideshowIoError {
        path: path.to_path_buf(),
        source: std::io::Error::other(source),
    })
}

#[derive(Debug, Error)]
#[error("slideshow state I/O at {path}: {source}")]
pub struct SlideshowIoError {
    pub path: PathBuf,
    #[source]
    pub source: std::io::Error,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected picker errors
mod tests {
    use super::*;

    fn pool(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn config_with(sort: SlideshowSort, history_size: usize) -> SlideshowConfig {
        SlideshowConfig {
            interval: Duration::from_secs(60),
            sort,
            recent_history_size: history_size,
            on_start: SlideshowStart::Resume,
            pause_when_active: false,
            skip_on_unavailable: false,
        }
    }

    #[test]
    fn alphabetical_sort_returns_in_order() {
        // Arrange
        let images = pool(&["c.png", "a.png", "b.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 0));

        // Act + Assert
        assert_eq!(picker.next(&images).unwrap(), PathBuf::from("a.png"));
        assert_eq!(picker.next(&images).unwrap(), PathBuf::from("b.png"));
        assert_eq!(picker.next(&images).unwrap(), PathBuf::from("c.png"));
        assert_eq!(picker.next(&images).unwrap(), PathBuf::from("a.png")); // wraps
    }

    #[test]
    fn history_suppresses_recent_images_rotation() {
        // Arrange — recent_history_size 2 means the last two picks must
        // not appear in the next two picks.
        let images = pool(&["a.png", "b.png", "c.png", "d.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 2));

        // Act
        let a = picker.next(&images).unwrap();
        let b = picker.next(&images).unwrap();
        let c = picker.next(&images).unwrap();

        // Assert — history is [b, a] when picking c, so c must not be a or b.
        assert_eq!(a, PathBuf::from("a.png"));
        assert_eq!(b, PathBuf::from("b.png"));
        assert_ne!(c, a);
        assert_ne!(c, b);
    }

    #[test]
    fn shuffle_with_seeded_rng_is_deterministic() {
        let images = pool(&["a.png", "b.png", "c.png", "d.png", "e.png"]);
        let mut p1 =
            SlideshowPicker::new(config_with(SlideshowSort::Shuffle, 2)).with_seeded_rng(42);
        let mut p2 =
            SlideshowPicker::new(config_with(SlideshowSort::Shuffle, 2)).with_seeded_rng(42);

        let s1: Vec<_> = (0..5).map(|_| p1.next(&images).unwrap()).collect();
        let s2: Vec<_> = (0..5).map(|_| p2.next(&images).unwrap()).collect();
        assert_eq!(s1, s2);
    }

    #[test]
    fn skip_unavailable_skips_missing_files() {
        // Arrange — only one of the three paths actually exists.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.png");
        std::fs::write(&real, b"fake").unwrap();
        let images = pool(&["does-not-exist-1.png", "does-not-exist-2.png"])
            .into_iter()
            .chain(std::iter::once(real.clone()))
            .collect::<Vec<_>>();
        let mut config = config_with(SlideshowSort::Alphabetical, 0);
        config.skip_on_unavailable = true;
        let mut picker = SlideshowPicker::new(config);

        // Act
        let first = picker.next(&images).unwrap();
        let second = picker.next(&images).unwrap();

        // Assert — picker should always land on the real file, never on a
        // missing one.
        assert_eq!(first, real);
        assert_eq!(second, real);
    }

    #[test]
    fn empty_pool_returns_empty_pool_error() {
        let mut picker = SlideshowPicker::new(SlideshowConfig::default());
        let err = picker.next(&[]).unwrap_err();
        assert_eq!(err, SlideshowError::EmptyPool);
    }

    #[test]
    fn resume_on_start_restores_history() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let images = pool(&["a.png", "b.png", "c.png"]);
        let mut config = config_with(SlideshowSort::Alphabetical, 2);
        config.on_start = SlideshowStart::Resume;
        let mut picker = SlideshowPicker::new(config.clone());
        picker.next(&images).unwrap(); // a
        picker.next(&images).unwrap(); // b
        persist_state(picker.state(), &state_path).unwrap();

        // Act
        let restored = load_state(&state_path).unwrap();
        let resumed = SlideshowPicker::with_state(config, restored.clone());

        // Assert — history is intact, and the next pick respects it.
        assert_eq!(restored.history.len(), 2);
        assert_eq!(restored.history[0], PathBuf::from("b.png"));
        assert_eq!(restored.history[1], PathBuf::from("a.png"));
        let mut resumed = resumed;
        let next = resumed.next(&images).unwrap();
        assert_eq!(next, PathBuf::from("c.png"));
    }

    #[test]
    fn new_random_on_start_clears_history() {
        let mut config = config_with(SlideshowSort::Shuffle, 5);
        config.on_start = SlideshowStart::NewRandom;
        let prior = SlideshowState {
            current_index: Some(2),
            history: VecDeque::from(vec![PathBuf::from("x.png"), PathBuf::from("y.png")]),
            paused: false,
        };
        let picker = SlideshowPicker::with_state(config, prior);
        assert!(picker.state().history.is_empty());
        assert_eq!(picker.state().current_index, None);
    }

    #[test]
    fn history_window_is_bounded_by_config() {
        let images = pool(&["a.png", "b.png", "c.png", "d.png", "e.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 2));
        for _ in 0..5 {
            picker.next(&images).unwrap();
        }
        assert!(picker.state().history.len() <= 2);
    }

    #[test]
    fn update_config_keeps_state_and_trims_history() {
        // Arrange — build up 3 history entries with a window of 5.
        let images = pool(&["a.png", "b.png", "c.png", "d.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 5));
        for _ in 0..3 {
            picker.next(&images).unwrap();
        }
        picker.state_mut().paused = true;

        // Act — shrink the window to 1 via a live config swap.
        let mut new_cfg = config_with(SlideshowSort::Shuffle, 1);
        new_cfg.on_start = SlideshowStart::NewRandom; // must NOT clear history
        picker.update_config(new_cfg);

        // Assert — history trimmed to the new window, pause preserved.
        assert_eq!(picker.state().history.len(), 1);
        assert!(picker.state().paused);
    }

    #[test]
    fn zero_history_window_still_tracks_current_image() {
        let images = pool(&["a.png", "b.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 0));

        let first = picker.next(&images).unwrap();

        // The window suppresses nothing, but the current image stays visible.
        assert_eq!(picker.state().history.front(), Some(&first));
        assert_eq!(picker.state().history.len(), 1);
    }

    #[test]
    fn step_back_restores_previous_image_as_current() {
        let images = pool(&["a.png", "b.png", "c.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 5));
        let first = picker.next(&images).unwrap();
        let _second = picker.next(&images).unwrap();

        let back = picker.step_back();

        assert_eq!(back, Some(first.clone()));
        assert_eq!(picker.state().history.front(), Some(&first));
        // Only one entry left — nothing further to go back to.
        assert!(picker.step_back().is_none());
    }

    #[test]
    fn sequential_resume_continues_from_step_back_target() {
        let images = pool(&["a.png", "b.png", "c.png"]);
        let mut picker = SlideshowPicker::new(config_with(SlideshowSort::Alphabetical, 2));
        let _a = picker.next(&images).unwrap();
        let b = picker.next(&images).unwrap();
        picker.step_back();

        // Next after stepping back to `a` is `b` again, not `c`.
        let next = picker.next(&images).unwrap();
        assert_eq!(next, b);
    }

    #[test]
    fn persist_and_load_round_trips_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut state = SlideshowState::default();
        state.history.push_front(PathBuf::from("a.png"));
        state.current_index = Some(0);
        state.paused = true;

        persist_state(&state, &path).unwrap();
        let loaded = load_state(&path).unwrap();
        assert_eq!(loaded, state);
    }

    #[test]
    fn load_state_returns_default_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let state = load_state(&dir.path().join("missing.json")).unwrap();
        assert_eq!(state, SlideshowState::default());
    }
}
