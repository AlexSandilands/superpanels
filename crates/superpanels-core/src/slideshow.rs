//! Slideshow picker (`SPEC.md` §9.2, PLAN.md §2.3).
//!
//! Pure logic: given a candidate pool and a [`SlideshowConfig`], the
//! [`SlideshowPicker`] decides which image comes next, suppresses the last
//! `recent_history_size` choices, and persists its [`SlideshowState`] so a
//! daemon restart resumes mid-rotation. Timing is *not* part of the
//! picker's job — that lives in the daemon (Phase 2.5) so the picker stays
//! synchronous and trivial to test.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rand::SeedableRng;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// User-facing slideshow tunables (`SPEC.md` §9.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideshowConfig {
    /// Wall-clock interval between automatic advances. Driven by the
    /// daemon — the picker doesn't read it.
    pub interval: Duration,
    /// Selection order.
    pub sort: SlideshowSort,
    /// Suppress the last *N* shown images so a small folder doesn't repeat
    /// immediately. Default of `10` is set by callers.
    pub recent_history_size: usize,
    /// What to do on daemon start (`Resume` keeps the prior history,
    /// `NewRandom` reshuffles, `First` jumps to index 0).
    pub on_start: SlideshowStart,
    /// Pause the timer while the user is interacting with the GUI.
    pub pause_when_active: bool,
    /// On `apply`, if a chosen file vanished between scan and apply, skip
    /// it instead of erroring.
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

/// Order in which the picker walks the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlideshowSort {
    /// Random selection respecting recent-history suppression.
    Shuffle,
    /// Sorted by path, ascending.
    Alphabetical,
    /// Sorted by directory mtime, oldest first. (The daemon supplies a
    /// pre-sorted pool — the picker does not stat files.)
    DateAsc,
    /// Sorted by directory mtime, newest first.
    DateDesc,
    /// Sorted by `last_shown` ascending — least-recently-shown first.
    LastShownAsc,
}

/// Behaviour when the daemon (re-)starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlideshowStart {
    /// Pick up exactly where we left off — keep the persisted history.
    Resume,
    /// Discard history and pick a new random starting point.
    NewRandom,
    /// Jump to index 0 (first item in the configured sort order).
    First,
}

/// Persistable slideshow state. Lives at
/// `$XDG_STATE_HOME/superpanels/state.json` per `SPEC.md` §9.2.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SlideshowState {
    /// Index of the most recently chosen entry within the (sorted) pool.
    /// `None` before the first pick.
    pub current_index: Option<usize>,
    /// Recently-shown paths, newest first. Bounded by
    /// `recent_history_size`.
    pub history: VecDeque<PathBuf>,
    /// User-toggled pause flag.
    pub paused: bool,
}

/// Errors raised by [`SlideshowPicker::next`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlideshowError {
    /// The candidate pool is empty.
    #[error("slideshow pool is empty")]
    EmptyPool,
    /// Every entry was either in the recent-history window or was skipped
    /// because the file vanished. The caller should widen the pool, shrink
    /// `recent_history_size`, or wait for the next scan.
    #[error("no eligible image: every candidate was in recent history or unavailable")]
    NoEligibleEntry,
}

/// Picker over a static pool of paths.
///
/// Holds a [`SlideshowState`] internally so callers can iterate without
/// threading state through every call. State is loaded with
/// [`Self::with_state`] and the latest copy can be exfiltrated via
/// [`Self::state`] for persistence.
pub struct SlideshowPicker {
    config: SlideshowConfig,
    state: SlideshowState,
    rng: rand::rngs::StdRng,
}

impl SlideshowPicker {
    /// New picker with empty state and an OS-seeded RNG.
    pub fn new(config: SlideshowConfig) -> Self {
        Self {
            config,
            state: SlideshowState::default(),
            rng: rand::rngs::StdRng::from_os_rng(),
        }
    }

    /// New picker initialised from a previously persisted state. Honours
    /// `config.on_start` — `NewRandom` and `First` reset history before
    /// the first pick.
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

    /// Replace the RNG with a deterministic seeded source — used by tests
    /// and benches that want reproducible Shuffle behaviour.
    #[cfg(test)]
    pub(crate) fn with_seeded_rng(mut self, seed: u64) -> Self {
        self.rng = rand::rngs::StdRng::seed_from_u64(seed);
        self
    }

    /// Snapshot the current state for persistence.
    pub fn state(&self) -> &SlideshowState {
        &self.state
    }

    /// Mutable view of the state (for the daemon to flip `paused`).
    pub fn state_mut(&mut self) -> &mut SlideshowState {
        &mut self.state
    }

    /// Compute the next path to display, mutating internal history.
    ///
    /// `pool` is the caller-managed candidate set (typically the
    /// [`crate::library::LibraryEntry`] paths after filters apply). The
    /// picker sorts the slice copy according to `config.sort`, picks the
    /// next one, and records it in `state.history`. Recent-history
    /// suppression is best-effort: if every entry is recent, the picker
    /// returns [`SlideshowError::NoEligibleEntry`] — the daemon decides
    /// whether to widen the pool or shrink the history window.
    pub fn next(&mut self, pool: &[PathBuf]) -> Result<PathBuf, SlideshowError> {
        if pool.is_empty() {
            return Err(SlideshowError::EmptyPool);
        }
        let ordered = sorted_pool(pool, self.config.sort);
        let history: std::collections::HashSet<PathBuf> =
            self.state.history.iter().cloned().collect();

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
        let last = self
            .state
            .current_index
            .and_then(|i| ordered.get(i).cloned());
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
        while self.state.history.len() > self.config.recent_history_size {
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

/// Persist `state` to `path` as JSON. The file is rewritten via a
/// `<path>.tmp` then rename so a crash never leaves a half-written file.
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

/// Read state from `path`. Returns `Default::default()` when the file is
/// absent — fresh install has no state until the first apply.
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

/// I/O failure during state persistence.
#[derive(Debug, Error)]
#[error("slideshow state I/O at {path}: {source}")]
pub struct SlideshowIoError {
    /// Path that failed.
    pub path: PathBuf,
    /// Underlying error.
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
