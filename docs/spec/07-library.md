# 7. Wallpaper sources & library

A "wallpaper management app" needs a real notion of a *library*, not just a single file path. The library is the user's set of available images; profiles and slideshows draw from it.

## 7.1 Library model

```rust
struct LibraryEntry {
    path: PathBuf,
    resolution: (u32, u32),     // cached after first scan
    aspect_ratio: f32,
    file_size: u64,
    modified: SystemTime,
    tags: Vec<String>,          // user-applied
    favourite: bool,
    last_shown: Option<SystemTime>,
    show_count: u32,
}
```

The library is a flat index over one or more *roots* (folders) configured by the user. The app tracks per-image metadata (favourite, tags, last-shown) in a sidecar SQLite database at `$XDG_DATA_HOME/superpanels/library.db` so it isn't lost on rescan.

## 7.2 Folder scanning

- Scan recursively (configurable per root).
- File-type filter: `jpg`, `jpeg`, `png`, `webp`, `avif`, `bmp`, `tiff`, `heic` (the last two via optional features). Decode failures are logged and skipped.
- The daemon watches roots with `notify` (inotify on Linux) and updates the index incrementally.
- Initial scan uses `rayon` for parallel decode of metadata (resolution, etc.). For 5,000 images on an SSD: target < 10 s.
- Thumbnails: generated lazily on first GUI request, stored under `$XDG_CACHE_HOME/superpanels/thumbs/{sha256(path)}.webp` at 320 px on the long edge. Thumbnail cache is bounded at 500 MB; oldest-not-shown evicted first.

## 7.3 Tags & favourites

- Tags are user-applied free-text strings (e.g. `nature`, `dark`, `pano`).
- Favourites are a special boolean tag with first-class UI treatment.
- Tags and favourites are stored in `library.db`, never in the image file or filename.
- Tags can be filtered on in slideshow `ImageFilters` (e.g. "rotate through favourites tagged `pano`").

## 7.4 Smart selection

For a slideshow over a folder, the picker can be configured to prefer images that suit the monitor layout:

- `aspect_ratios = "wide"` filters to images whose aspect ratio is within ±10% of the canvas aspect — i.e. images that will span well without heavy crop.
- `min_resolution` rejects anything smaller than the canvas pixel area.
- `recent_history_size = N` (default 10) suppresses the last N images shown so a 12-image folder doesn't repeat for a while.
- `tags = ["foo"]` includes only matching images.
- `favourites_only = true` is shorthand for filtering on the favourite flag.

## 7.5 Drag-and-drop

In the GUI:
- Dropping an image file onto the main window adds it to the active profile as a Single source.
- Dropping a folder adds it as a library root and activates a Folder source.
- Dropping onto a specific monitor in the canvas creates a `PerMonitor`-body profile with that file pinned to that monitor.
