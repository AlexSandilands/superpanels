//! In-process LRU for `cmd_library_thumbnail` outputs.
//!
//! Keyed on `(canonicalised_path, mtime)` so a write to the source file
//! invalidates the cache the next time the daemon stats it. Bounded by both
//! [`MAX_ENTRIES`] and [`MAX_BYTES`] so a sustained slideshow over a
//! many-thousand-entry library can't drift the resident set indefinitely.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Maximum number of cached thumbnails. Sized to comfortably cover the
/// slideshow look-ahead (next/prev) plus a typical library-grid first-page
/// prewarm without forcing eviction during normal use.
const MAX_ENTRIES: usize = 64;

/// Maximum total cached bytes (encoded PNG). 64 entries × ~256 KiB each ≈
/// 16 MiB; the byte cap is the hard backstop in case a single high-resolution
/// thumbnail exceeds the per-entry budget.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Key {
    path: PathBuf,
    mtime: SystemTime,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedThumbnail {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

#[derive(Debug, Default)]
pub(crate) struct ThumbnailCache {
    /// Front = most-recently-used. `VecDeque` is fine at `MAX_ENTRIES = 64`;
    /// linear scans cost O(64) which is dwarfed by the decode + resize a hit
    /// avoids.
    entries: VecDeque<(Key, CachedThumbnail)>,
    bytes_in_use: usize,
}

impl ThumbnailCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Look up a cached thumbnail and promote it to MRU on hit.
    pub(crate) fn get(&mut self, path: &Path, mtime: SystemTime) -> Option<CachedThumbnail> {
        let pos = self
            .entries
            .iter()
            .position(|(k, _)| k.path == path && k.mtime == mtime)?;
        // `remove` returns the entry; reinsert at the front to mark as MRU.
        let entry = self.entries.remove(pos)?;
        let cloned = entry.1.clone();
        self.entries.push_front(entry);
        Some(cloned)
    }

    /// Insert (or overwrite) `path`'s cached thumbnail. Evicts LRU entries
    /// until both [`MAX_ENTRIES`] and [`MAX_BYTES`] are satisfied. A single
    /// thumbnail larger than [`MAX_BYTES`] is silently dropped — caching it
    /// would force-evict everything else.
    // reason: clippy flags `mime` vs `mtime`, but `mime` is the established
    // parameter name in this crate's render path (`render_thumbnail` → `(bytes,
    // mime)`). Keeping the names consistent across handler ↔ cache aids review.
    #[allow(clippy::similar_names)]
    pub(crate) fn put(
        &mut self,
        path: PathBuf,
        mtime: SystemTime,
        bytes: Vec<u8>,
        mime: &'static str,
    ) {
        if bytes.len() > MAX_BYTES {
            return;
        }
        let key = Key { path, mtime };
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            if let Some((_, removed)) = self.entries.remove(pos) {
                self.bytes_in_use = self.bytes_in_use.saturating_sub(removed.bytes.len());
            }
        }
        self.bytes_in_use = self.bytes_in_use.saturating_add(bytes.len());
        self.entries
            .push_front((key, CachedThumbnail { bytes, mime }));
        while self.entries.len() > MAX_ENTRIES || self.bytes_in_use > MAX_BYTES {
            let Some((_, removed)) = self.entries.pop_back() else {
                break;
            };
            self.bytes_in_use = self.bytes_in_use.saturating_sub(removed.bytes.len());
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn bytes_in_use(&self) -> usize {
        self.bytes_in_use
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness bugs
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(format!("/walls/{name}.png"))
    }

    fn epoch() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    #[test]
    fn miss_then_hit_returns_cached_bytes() {
        let mut c = ThumbnailCache::new();
        assert!(c.get(&fixture("a"), epoch()).is_none());
        c.put(fixture("a"), epoch(), vec![1, 2, 3], "image/png");
        let hit = c.get(&fixture("a"), epoch()).unwrap();
        assert_eq!(hit.bytes, vec![1, 2, 3]);
        assert_eq!(hit.mime, "image/png");
    }

    #[test]
    fn mtime_change_invalidates_cache() {
        // The point of keying on mtime: a write to the source file must look
        // like a miss the next time the daemon stats it, even if the path is
        // identical.
        let mut c = ThumbnailCache::new();
        c.put(fixture("a"), epoch(), vec![1, 2, 3], "image/png");
        let later = epoch() + Duration::from_secs(60);
        assert!(c.get(&fixture("a"), later).is_none());
        // The original (path, old-mtime) entry is still cached — useful if
        // the daemon races a concurrent write and the older mtime wins.
        assert!(c.get(&fixture("a"), epoch()).is_some());
    }

    #[test]
    fn entries_are_evicted_oldest_first_above_count_cap() {
        let mut c = ThumbnailCache::new();
        // Fill past MAX_ENTRIES with tiny payloads so the byte cap doesn't fire.
        for i in 0..MAX_ENTRIES + 5 {
            c.put(
                fixture(&format!("e{i}")),
                epoch(),
                vec![0u8; 8],
                "image/png",
            );
        }
        assert_eq!(c.len(), MAX_ENTRIES);
        // The first 5 inserts are evicted because the next 64 became MRU.
        for i in 0..5 {
            assert!(
                c.get(&fixture(&format!("e{i}")), epoch()).is_none(),
                "expected e{i} to have been evicted"
            );
        }
    }

    #[test]
    fn byte_cap_evicts_until_under_limit() {
        let mut c = ThumbnailCache::new();
        // Each entry is ~5 MiB; three of them blow past the 16 MiB cap.
        let big = vec![0u8; 5 * 1024 * 1024];
        c.put(fixture("a"), epoch(), big.clone(), "image/png");
        c.put(fixture("b"), epoch(), big.clone(), "image/png");
        c.put(fixture("c"), epoch(), big.clone(), "image/png");
        c.put(fixture("d"), epoch(), big, "image/png");
        assert!(c.bytes_in_use() <= MAX_BYTES);
        // Oldest insertions evicted first; "d" must remain.
        assert!(c.get(&fixture("d"), epoch()).is_some());
        assert!(c.get(&fixture("a"), epoch()).is_none());
    }

    #[test]
    fn put_overwrites_existing_entry_without_double_counting_bytes() {
        let mut c = ThumbnailCache::new();
        c.put(fixture("a"), epoch(), vec![0u8; 1024], "image/png");
        let after_first = c.bytes_in_use();
        c.put(fixture("a"), epoch(), vec![0u8; 2048], "image/png");
        // bytes_in_use must reflect ONLY the replacement, not the sum of both.
        assert_eq!(c.bytes_in_use(), after_first - 1024 + 2048);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn payload_larger_than_cap_is_dropped_without_evicting_others() {
        let mut c = ThumbnailCache::new();
        c.put(fixture("a"), epoch(), vec![0u8; 1024], "image/png");
        c.put(
            fixture("huge"),
            epoch(),
            vec![0u8; MAX_BYTES + 1],
            "image/png",
        );
        // The oversized payload was rejected outright; the prior entry stays.
        assert!(c.get(&fixture("huge"), epoch()).is_none());
        assert!(c.get(&fixture("a"), epoch()).is_some());
    }

    #[test]
    fn get_promotes_to_mru() {
        let mut c = ThumbnailCache::new();
        for i in 0..MAX_ENTRIES {
            c.put(
                fixture(&format!("e{i}")),
                epoch(),
                vec![0u8; 8],
                "image/png",
            );
        }
        // Touch e0 so the next eviction prefers e1 over e0.
        let _ = c.get(&fixture("e0"), epoch());
        c.put(fixture("new"), epoch(), vec![0u8; 8], "image/png");
        assert!(
            c.get(&fixture("e0"), epoch()).is_some(),
            "e0 should still be cached after promotion"
        );
        assert!(
            c.get(&fixture("e1"), epoch()).is_none(),
            "e1 should have been evicted"
        );
    }
}
