# Phase 4b — Library + SQLite

**Goal.** Library grid with thumbnails, tags, favourites. SQLite replaces the Phase-2 JSON index. Drag images *from* the grid *onto* canvas monitors.

**Definition of done.**
- [x] Library grid renders 1,000 thumbnails smoothly; filtering by tag and aspect ratio works.
- [x] Tags and favourites persist across daemon restarts via SQLite.
- [x] Migration from the Phase-2 `library-index.json` runs once on first launch and removes the old file.
- [ ] One clean screenshot of the library grid. *(Captured manually by the maintainer once a real desktop is available; agent runs do not have a display.)*

## 4b.1 Library grid
- [x] `LibraryGrid` virtualised list (hand-rolled windowed grid in `ui/src/components/LibraryGrid.svelte`) — only the visible row range is mounted.
- [x] `library_thumbnail` IPC returns PNG bytes; cached client-side via `URL.createObjectURL` (`ui/src/lib/library/thumb_cache.ts`).
- [x] Filters: tag chips, aspect ratio dropdown, min-resolution input.
- [x] Sort: Date added (mtime fallback), Date modified, Resolution, Last shown, Name.
- [x] Right-click context menu: Apply as span source, Favourite, Add tag…, Remove from library. *("Set for monitor…" + "Reveal in file manager" deferred to 4d polish — they need a monitor-picker dialog and a `tauri-plugin-shell` wire-up respectively.)*
- [x] Search box filtering on filename + tag.
- [x] Drag-and-drop image *out* of the grid onto a monitor in the canvas → patches the active draft into a `PerMonitor` body with that image pinned to the targeted monitor.

## 4b.2 Library backing — SQLite
- [x] Replace the Phase-2 in-memory + `library-index.json` index with SQLite per SPEC §14.5 (`crates/superpanels-core/src/library/db.rs`).
- [x] One-shot migration from `library-index.json` on first run of the new version; renames the source to `library-index.json.v1.bak` once committed (`crates/superpanels-core/src/library/migrate.rs`).
- [x] Schema migrations via `PRAGMA user_version` (current `SCHEMA_VERSION = 1`).
- [x] Tag operations idempotent; case-insensitive matching (`COLLATE NOCASE` on the tag name; daemon handler normalises before write).

**Risks for this phase.**
- Thumbnail generation for a large library is the GUI's first-impression cost. Move to a background queue with visible progress, never block the grid.
