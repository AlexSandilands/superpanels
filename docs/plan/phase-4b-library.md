# Phase 4b — Library + SQLite

**Goal.** Library grid with thumbnails, tags, favourites. SQLite replaces the Phase-2 JSON index. Drag images *from* the grid *onto* canvas monitors.

**Definition of done.**
- [ ] Library grid renders 1,000 thumbnails smoothly; filtering by tag and aspect ratio works.
- [ ] Tags and favourites persist across daemon restarts via SQLite.
- [ ] Migration from the Phase-2 `library-index.json` runs once on first launch and removes the old file.
- [ ] One clean screenshot of the library grid.

## 4b.1 Library grid
- [ ] `LibraryGrid` virtualised list (e.g. `svelte-virtual-list`); renders only visible rows.
- [ ] `library_thumbnail` IPC returns WebP bytes; cached client-side via `URL.createObjectURL`.
- [ ] Filters: tag chips, aspect ratio dropdown, min-resolution input.
- [ ] Sort: Date added, Date modified, Resolution, Last shown.
- [ ] Right-click context menu: Apply now, Set for monitor…, Tag…, Favourite, Reveal in file manager, Delete from library.
- [ ] Search box filtering on filename + tag.
- [ ] Drag-and-drop image *out* of the grid onto a monitor in the canvas → assigns image to that monitor (PerMonitor body).

## 4b.2 Library backing — SQLite
- [ ] Replace the Phase-2 in-memory + `library-index.json` index with SQLite per SPEC §14.5.
- [ ] One-shot migration from `library-index.json` on first run of the new version; removes the old file once committed.
- [ ] Schema migrations via `PRAGMA user_version`.
- [ ] Tag operations idempotent; case-insensitive matching.

**Risks for this phase.**
- Thumbnail generation for a large library is the GUI's first-impression cost. Move to a background queue with visible progress, never block the grid.
