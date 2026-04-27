//! Folder-driven library index (`SPEC.md` §7.1–7.2, PLAN.md §2.4).
//!
//! Stores a flat in-memory list of [`LibraryEntry`]s built by walking one or
//! more roots in parallel via `rayon`. Tags / favourites / show counts live
//! on each entry and are persisted to a single JSON file
//! (`$XDG_STATE_HOME/superpanels/library-index.json`); a `notify`-backed
//! [`FolderWatcher`] forwards FS events for the daemon to handle in Phase
//! 2.5. `SQLite` arrives in Phase 4b — Phase 2 deliberately stays JSON-only
//! per PLAN.md §2.4.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::SystemTime;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

/// Cached metadata for a single image found by [`scan_folder`].
///
/// Mirrors `SPEC.md` §7.1. `tags`, `favourite`, `last_shown` and
/// `show_count` start empty / false / `None` / `0` and are mutated by the
/// GUI / daemon as the user interacts with the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Absolute path to the image file.
    pub path: PathBuf,
    /// Pixel resolution `(width, height)` read via `image::image_dimensions`
    /// — does not decode the full image.
    pub resolution: (u32, u32),
    /// Aspect ratio `width / height`, cached for filter convenience.
    pub aspect_ratio: f32,
    /// File size in bytes from `std::fs::metadata`.
    pub file_size: u64,
    /// Last-modified time from the filesystem.
    pub modified: SystemTime,
    /// User-applied free-text tags (`SPEC.md` §7.3).
    #[serde(default)]
    pub tags: Vec<String>,
    /// User-applied favourite flag (the special boolean tag).
    #[serde(default)]
    pub favourite: bool,
    /// Most recent time this entry was shown by the slideshow, or `None` if
    /// it has never been shown.
    #[serde(default)]
    pub last_shown: Option<SystemTime>,
    /// Cumulative number of times the entry has been shown.
    #[serde(default)]
    pub show_count: u32,
}

const SUPPORTED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "tiff"];

/// Errors raised by the library index API.
#[derive(Debug, Error)]
pub enum LibraryError {
    /// The persisted index file existed but contained malformed JSON.
    #[error("library index at {path} is corrupt: {source}")]
    Corrupt {
        /// Path that failed to parse.
        path: PathBuf,
        /// Underlying serde error.
        #[source]
        source: serde_json::Error,
    },
    /// I/O failure while reading or writing the index.
    #[error("library index I/O at {path}: {source}")]
    Io {
        /// Path involved in the failed I/O.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// FS-watch setup failed (missing inotify, permissions, …).
    #[error("watcher setup failed: {0}")]
    Watch(#[from] notify::Error),
}

/// Walk `root` (recursively when `recursive` is `true`) and return a
/// [`LibraryEntry`] for every supported image found.
///
/// Reads `image::image_dimensions` (header only — does not decode pixels)
/// so 100k images scan in seconds. The closure `progress` is invoked
/// periodically with the count of entries processed so far for GUI
/// progress reporting; it must be `Send + Sync` because `rayon` calls it
/// from worker threads.
///
/// Files with unsupported extensions are skipped silently (logged at
/// `debug!`). Image header reads that fail are also skipped — a corrupt
/// JPEG shouldn't crash the scan.
pub fn scan_folder(
    root: &Path,
    recursive: bool,
    progress: impl Fn(usize) + Send + Sync,
) -> Vec<LibraryEntry> {
    let candidates = collect_candidates(root, recursive);
    let counter = std::sync::atomic::AtomicUsize::new(0);

    candidates
        .par_iter()
        .filter_map(|path| {
            let entry = build_entry(path);
            let n = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            progress(n);
            entry
        })
        .collect()
}

fn collect_candidates(root: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, recursive, &mut out);
    out
}

fn walk(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if recursive {
                walk(&path, recursive, out);
            }
            continue;
        }
        if file_type.is_file() && has_supported_extension(&path) {
            out.push(path);
        }
    }
}

fn has_supported_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext_lower = ext.to_ascii_lowercase();
    SUPPORTED_EXTS.iter().any(|s| *s == ext_lower)
}

fn build_entry(path: &Path) -> Option<LibraryEntry> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "skipping entry: stat failed");
            return None;
        }
    };
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let dimensions = match image::image_dimensions(path) {
        Ok(d) => d,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "skipping entry: dimension read failed");
            return None;
        }
    };
    let (w, h) = dimensions;
    if h == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)] // reason: typical image dims fit f32 mantissa exactly
    let aspect_ratio = (w as f32) / (h as f32);
    Some(LibraryEntry {
        path: path.to_path_buf(),
        resolution: dimensions,
        aspect_ratio,
        file_size: metadata.len(),
        modified,
        tags: Vec::new(),
        favourite: false,
        last_shown: None,
        show_count: 0,
    })
}

/// Persist `entries` to `path` as JSON. The file is rewritten atomically:
/// we serialise to `<path>.tmp` first then rename, so a crashing scan
/// never leaves a half-written index in place.
pub fn persist_index(entries: &[LibraryEntry], path: &Path) -> Result<(), LibraryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| LibraryError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_vec_pretty(entries).map_err(|source| LibraryError::Corrupt {
        path: path.to_path_buf(),
        source,
    })?;
    std::fs::write(&tmp, &json).map_err(|source| LibraryError::Io {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| LibraryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Read the persisted index back from `path`. Returns an empty `Vec` when
/// the file is absent — a fresh install has no index until the first scan.
pub fn load_index(path: &Path) -> Result<Vec<LibraryEntry>, LibraryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(path).map_err(|source| LibraryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let entries: Vec<LibraryEntry> =
        serde_json::from_slice(&bytes).map_err(|e| LibraryError::Corrupt {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(entries)
}

/// Thin wrapper around [`notify::RecommendedWatcher`] that forwards every
/// raw FS event onto a channel.
///
/// The watcher does *not* update the index itself — the daemon (Phase 2.5)
/// owns the debounce / coalesce logic. Splitting it this way keeps the
/// core pure: tests can drive the channel directly without spinning up
/// inotify.
pub struct FolderWatcher {
    watcher: RecommendedWatcher,
    roots: Vec<PathBuf>,
}

impl FolderWatcher {
    /// Create a watcher subscribed to every path in `roots`. Each root is
    /// watched recursively. Events are sent on `tx`; the receiver lives in
    /// the caller (typically the daemon's event loop).
    pub fn new(roots: &[PathBuf], tx: Sender<notify::Event>) -> Result<Self, LibraryError> {
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            })?;
        for root in roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
        }
        Ok(Self {
            watcher,
            roots: roots.to_vec(),
        })
    }

    /// Add `path` to the watch set; recursive.
    pub fn watch(&mut self, path: &Path) -> Result<(), LibraryError> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        self.roots.push(path.to_path_buf());
        Ok(())
    }

    /// Remove `path` from the watch set. No-op if it wasn't being watched.
    pub fn unwatch(&mut self, path: &Path) -> Result<(), LibraryError> {
        self.watcher.unwatch(path)?;
        self.roots.retain(|r| r != path);
        Ok(())
    }

    /// Currently-watched roots (read-only snapshot).
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on FS / image errors
mod tests {
    use super::*;

    use image::{ImageBuffer, Rgba};

    fn write_png(path: &Path, w: u32, h: u32) {
        let buf = ImageBuffer::<Rgba<u8>, _>::from_pixel(w, h, Rgba([255, 0, 0, 255]));
        buf.save(path).unwrap();
    }

    #[test]
    fn scan_folder_finds_images_recursively() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("a.png"), 16, 9);
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        write_png(&nested.join("b.png"), 32, 18);

        // Act
        let entries = scan_folder(dir.path(), true, |_| {});

        // Assert
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries
            .iter()
            .filter_map(|e| e.path.file_name().and_then(|s| s.to_str()))
            .collect();
        assert!(names.contains(&"a.png"));
        assert!(names.contains(&"b.png"));
    }

    #[test]
    fn scan_folder_skips_non_images() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("a.png"), 16, 9);
        std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("readme.md"), b"# title").unwrap();

        // Act
        let entries = scan_folder(dir.path(), false, |_| {});

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].resolution, (16, 9));
    }

    #[test]
    fn scan_folder_non_recursive_ignores_subdirectories() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("top.png"), 16, 9);
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        write_png(&nested.join("inner.png"), 32, 18);

        // Act
        let entries = scan_folder(dir.path(), false, |_| {});

        // Assert
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn scan_folder_invokes_progress_callback_once_per_entry() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            write_png(&dir.path().join(format!("{i}.png")), 8, 8);
        }
        let counter = std::sync::atomic::AtomicUsize::new(0);

        // Act
        let entries = scan_folder(dir.path(), false, |_| {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        // Assert
        assert_eq!(entries.len(), 5);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 5);
    }

    #[test]
    fn persist_and_load_round_trips() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("a.png"), 16, 9);
        let mut entries = scan_folder(dir.path(), false, |_| {});
        entries[0].tags.push("nature".to_owned());
        entries[0].favourite = true;
        let index_path = dir.path().join("library-index.json");

        // Act
        persist_index(&entries, &index_path).unwrap();
        let loaded = load_index(&index_path).unwrap();

        // Assert
        assert_eq!(loaded, entries);
    }

    #[test]
    fn load_index_returns_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let entries = load_index(&dir.path().join("missing.json")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn load_index_errors_on_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let err = load_index(&path).unwrap_err();
        assert!(matches!(err, LibraryError::Corrupt { .. }));
    }
}
