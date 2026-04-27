//! Folder-driven library index (`SPEC.md` §7.1–7.2). JSON-only in Phase 2;
//! `SQLite` lands in Phase 4b.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::SystemTime;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub path: PathBuf,
    /// `(width, height)` from `image::image_dimensions` — header read, not
    /// a full decode.
    pub resolution: (u32, u32),
    pub aspect_ratio: f32,
    pub file_size: u64,
    pub modified: SystemTime,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favourite: bool,
    #[serde(default)]
    pub last_shown: Option<SystemTime>,
    #[serde(default)]
    pub show_count: u32,
}

const SUPPORTED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "tiff"];

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("library index at {path} is corrupt: {source}")]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("library index I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("watcher setup failed: {0}")]
    Watch(#[from] notify::Error),
}

/// Walk `root` and return a [`LibraryEntry`] per supported image. Uses
/// header-only image reads via `rayon`, so 100k files scan in seconds.
/// Unsupported extensions and unreadable files are skipped (logged at
/// `debug!`) — a corrupt JPEG shouldn't crash the scan. `progress` is
/// called from worker threads, hence `Send + Sync`.
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

/// Persist `entries` as JSON. Writes via `<path>.tmp` then renames, so a
/// crash mid-write never leaves a half-written index.
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

/// Returns an empty `Vec` when `path` doesn't exist (fresh install).
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

/// Forwards raw FS events onto a channel. Debounce / coalesce belongs to
/// the daemon — keeping it out of here lets tests drive the channel
/// directly without spinning up inotify.
pub struct FolderWatcher {
    watcher: RecommendedWatcher,
    roots: Vec<PathBuf>,
}

impl FolderWatcher {
    /// Each root is watched recursively.
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

    pub fn watch(&mut self, path: &Path) -> Result<(), LibraryError> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        self.roots.push(path.to_path_buf());
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<(), LibraryError> {
        self.watcher.unwatch(path)?;
        self.roots.retain(|r| r != path);
        Ok(())
    }

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
