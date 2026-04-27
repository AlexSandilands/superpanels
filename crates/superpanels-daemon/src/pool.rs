//! Slideshow pool resolution. Lifts `scan_folder` calls *out* of the
//! daemon state lock so a Rayon-blocking walk cannot stall every other
//! IPC handler.
//!
//! The contract is: callers serve the pool from `DaemonState.library`
//! when the cached index already covers the requested folder; otherwise
//! they hand the resolution to `scan_blocking` *with the lock dropped*
//! and update the cache afterward.

use std::path::{Path, PathBuf};

use superpanels_core::config::ImageSet;
use superpanels_core::library::{LibraryEntry, scan_folder};

/// Try to satisfy `images` from `cached_library` without touching disk.
///
/// Returns `Some(paths)` when:
/// - it is a `Playlist`, or
/// - it is a `Folder` and `cached_library` has at least one entry under
///   that folder (respecting `recursive`).
///
/// Returns `None` when a fresh scan is required.
pub(crate) fn pool_from_cache(
    images: &ImageSet,
    cached_library: &[LibraryEntry],
) -> Option<Vec<PathBuf>> {
    match images {
        ImageSet::Playlist { paths } => Some(paths.clone()),
        ImageSet::Folder { path, recursive } => {
            let hits: Vec<PathBuf> = cached_library
                .iter()
                .filter(|e| under_folder(&e.path, path, *recursive))
                .map(|e| e.path.clone())
                .collect();
            if hits.is_empty() { None } else { Some(hits) }
        }
    }
}

/// Synchronous fallback used inside `tokio::task::spawn_blocking`.
/// Walks the disk; safe to call without holding any daemon lock.
pub(crate) fn scan_blocking(images: &ImageSet) -> Vec<PathBuf> {
    match images {
        ImageSet::Folder { path, recursive } => scan_folder(path, *recursive, |_| {})
            .into_iter()
            .map(|e| e.path)
            .collect(),
        ImageSet::Playlist { paths } => paths.clone(),
    }
}

fn under_folder(entry: &Path, folder: &Path, recursive: bool) -> bool {
    let Ok(rel) = entry.strip_prefix(folder) else {
        return false;
    };
    if recursive {
        return true;
    }
    // Non-recursive: only direct children — relative path must have exactly one component.
    rel.components().count() == 1
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn entry(path: &str) -> LibraryEntry {
        LibraryEntry {
            path: PathBuf::from(path),
            resolution: (1, 1),
            aspect_ratio: 1.0,
            file_size: 0,
            modified: SystemTime::UNIX_EPOCH,
            tags: Vec::new(),
            favourite: false,
            last_shown: None,
            show_count: 0,
        }
    }

    #[test]
    fn pool_from_cache_serves_playlist_directly() {
        let playlist = ImageSet::Playlist {
            paths: vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")],
        };
        let pool = pool_from_cache(&playlist, &[]).unwrap();
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn pool_from_cache_returns_none_when_folder_has_no_cached_entries() {
        let images = ImageSet::Folder {
            path: PathBuf::from("/walls"),
            recursive: true,
        };
        assert!(pool_from_cache(&images, &[entry("/other/x.png")]).is_none());
    }

    #[test]
    fn pool_from_cache_filters_by_folder_when_recursive() {
        let images = ImageSet::Folder {
            path: PathBuf::from("/walls"),
            recursive: true,
        };
        let lib = vec![
            entry("/walls/a.png"),
            entry("/walls/sub/b.png"),
            entry("/elsewhere/c.png"),
        ];
        let pool = pool_from_cache(&images, &lib).unwrap();
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn pool_from_cache_excludes_subdirs_when_not_recursive() {
        let images = ImageSet::Folder {
            path: PathBuf::from("/walls"),
            recursive: false,
        };
        let lib = vec![entry("/walls/a.png"), entry("/walls/sub/b.png")];
        let pool = pool_from_cache(&images, &lib).unwrap();
        assert_eq!(pool, vec![PathBuf::from("/walls/a.png")]);
    }
}
