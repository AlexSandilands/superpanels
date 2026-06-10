//! Slideshow pool resolution. Lifts `scan_folder` calls *out* of the
//! daemon state lock so a Rayon-blocking walk cannot stall every other
//! IPC handler.
//!
//! [`resolve_pool`] is the single entry point: it serves what it can from
//! `DaemonState.library`, hands only the uncovered folders to
//! `scan_blocking` *with the lock dropped*, and records the pool size for
//! the GUI counter. The persisted library cache is owned by the FS watcher
//! and explicit rescans, so ad-hoc slideshow folders never pollute it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use superpanels_core::config::{ImageSet, ImageSource};
use superpanels_core::library::{LibraryEntry, scan_folder};
use tokio::sync::Mutex;
use tracing::error;

use crate::state::DaemonState;

/// Outcome of a cache-only resolution attempt.
pub(crate) struct CachePool {
    /// Paths served from image sources and cache-covered folders, deduped.
    pub hits: Vec<PathBuf>,
    /// Folder sources with zero cached entries — either outside the library
    /// roots or genuinely empty; only these need a disk walk.
    pub uncovered: Vec<ImageSource>,
}

/// Resolve `images` to the full pool, returning `None` when it is empty.
/// Folders the cache covers cost no I/O; the rest are scanned in
/// `spawn_blocking`. Updates `DaemonState::slideshow_pool_len` on success.
pub(crate) async fn resolve_pool(
    state: &Arc<Mutex<DaemonState>>,
    images: &ImageSet,
) -> Option<Vec<PathBuf>> {
    let CachePool { hits, uncovered } = {
        let guard = state.lock().await;
        pool_from_cache(images, &guard.library)
    };

    let pool = if uncovered.is_empty() {
        hits
    } else {
        let scan_set = ImageSet { sources: uncovered };
        let scanned = match tokio::task::spawn_blocking(move || scan_blocking(&scan_set)).await {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "pool resolver task panicked");
                return None;
            }
        };
        dedupe(hits.into_iter().chain(scanned).collect())
    };

    if pool.is_empty() {
        return None;
    }
    state.lock().await.slideshow_pool_len = Some(pool.len());
    Some(pool)
}

/// Serve as much of `images` as possible from `cached_library` without
/// touching disk. Image sources always hit; folder sources with no cached
/// entries are reported back as `uncovered` rather than failing the whole
/// resolution, so one un-indexed folder doesn't force a rescan of the rest.
pub(crate) fn pool_from_cache(images: &ImageSet, cached_library: &[LibraryEntry]) -> CachePool {
    let mut hits: Vec<PathBuf> = Vec::new();
    let mut uncovered: Vec<ImageSource> = Vec::new();
    for source in &images.sources {
        match source {
            ImageSource::Image { path } => hits.push(path.clone()),
            ImageSource::Folder { path, recursive } => {
                let before = hits.len();
                hits.extend(
                    cached_library
                        .iter()
                        .filter(|e| under_folder(&e.path, path, *recursive))
                        .map(|e| e.path.clone()),
                );
                if hits.len() == before {
                    uncovered.push(source.clone());
                }
            }
        }
    }
    CachePool {
        hits: dedupe(hits),
        uncovered,
    }
}

/// Synchronous fallback used inside `tokio::task::spawn_blocking`.
/// Walks the disk; safe to call without holding any daemon lock.
pub(crate) fn scan_blocking(images: &ImageSet) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for source in &images.sources {
        match source {
            ImageSource::Folder { path, recursive } => out.extend(
                scan_folder(path, *recursive, |_| {})
                    .into_iter()
                    .map(|e| e.path),
            ),
            ImageSource::Image { path } => out.push(path.clone()),
        }
    }
    dedupe(out)
}

/// A hand-picked image may also live inside a folder source; keep the first
/// occurrence so the picker never sees the same path twice.
fn dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    if paths.len() < 2 {
        return paths;
    }
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(paths.len());
    paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
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
    fn pool_from_cache_serves_image_sources_directly() {
        let set = ImageSet::from_images(vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")]);
        let res = pool_from_cache(&set, &[]);
        assert_eq!(res.hits.len(), 2);
        assert!(res.uncovered.is_empty());
    }

    #[test]
    fn pool_from_cache_reports_folder_with_no_cached_entries_as_uncovered() {
        let images = ImageSet::from_folder(PathBuf::from("/walls"), true);
        let res = pool_from_cache(&images, &[entry("/other/x.png")]);
        assert!(res.hits.is_empty());
        assert_eq!(res.uncovered.len(), 1);
    }

    #[test]
    fn pool_from_cache_filters_by_folder_when_recursive() {
        let images = ImageSet::from_folder(PathBuf::from("/walls"), true);
        let lib = vec![
            entry("/walls/a.png"),
            entry("/walls/sub/b.png"),
            entry("/elsewhere/c.png"),
        ];
        let res = pool_from_cache(&images, &lib);
        assert_eq!(res.hits.len(), 2);
        assert!(res.uncovered.is_empty());
    }

    #[test]
    fn pool_from_cache_excludes_subdirs_when_not_recursive() {
        let images = ImageSet::from_folder(PathBuf::from("/walls"), false);
        let lib = vec![entry("/walls/a.png"), entry("/walls/sub/b.png")];
        let res = pool_from_cache(&images, &lib);
        assert_eq!(res.hits, vec![PathBuf::from("/walls/a.png")]);
    }

    #[test]
    fn pool_from_cache_unions_folders_and_images_without_duplicates() {
        let images = ImageSet {
            sources: vec![
                ImageSource::Folder {
                    path: PathBuf::from("/walls"),
                    recursive: false,
                },
                ImageSource::Image {
                    path: PathBuf::from("/walls/a.png"), // duplicate of a folder hit
                },
                ImageSource::Image {
                    path: PathBuf::from("/pick/c.png"),
                },
            ],
        };
        let lib = vec![entry("/walls/a.png"), entry("/walls/b.png")];
        let res = pool_from_cache(&images, &lib);
        assert_eq!(
            res.hits,
            vec![
                PathBuf::from("/walls/a.png"),
                PathBuf::from("/walls/b.png"),
                PathBuf::from("/pick/c.png"),
            ]
        );
    }

    #[test]
    fn pool_from_cache_keeps_covered_hits_when_one_folder_is_uncovered() {
        // One un-indexed folder must not discard the cached half of the set.
        let images = ImageSet {
            sources: vec![
                ImageSource::Folder {
                    path: PathBuf::from("/walls"),
                    recursive: true,
                },
                ImageSource::Folder {
                    path: PathBuf::from("/not-indexed"),
                    recursive: true,
                },
            ],
        };
        let res = pool_from_cache(&images, &[entry("/walls/a.png")]);
        assert_eq!(res.hits, vec![PathBuf::from("/walls/a.png")]);
        assert_eq!(
            res.uncovered,
            vec![ImageSource::Folder {
                path: PathBuf::from("/not-indexed"),
                recursive: true,
            }]
        );
    }

    #[test]
    fn scan_blocking_includes_picked_images_alongside_folder_scan() {
        let dir = tempfile::tempdir().unwrap();
        let in_folder = dir.path().join("a.png");
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save(&in_folder)
            .unwrap();
        let images = ImageSet {
            sources: vec![
                ImageSource::Folder {
                    path: dir.path().to_path_buf(),
                    recursive: false,
                },
                ImageSource::Image {
                    path: PathBuf::from("/pick/c.png"),
                },
            ],
        };
        let pool = scan_blocking(&images);
        assert!(pool.contains(&in_folder));
        assert!(pool.contains(&PathBuf::from("/pick/c.png")));
    }
}
