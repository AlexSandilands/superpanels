# Phase 3 — Tauri shell & tray

**Goal.** `superpanels gui` opens a window. Tray icon works. Profile switching from the tray is functional. UI is functional but not yet polished.

**Definition of done.**
- [x] `superpanels gui` opens a window with a working monitor canvas (rectangles only — no image yet) and a profile list.
- [x] System tray shows the icon with a working profile-switch menu.
- [x] All Tauri IPC commands from SPEC §12.4 are wired through to the daemon's IPC.
- [x] Closing the GUI window does not stop the daemon. Quit from tray menu stops both.
- [x] Auto-start opt-in writes `~/.config/autostart/superpanels.desktop`.

## 3.1 Tauri scaffold
- [x] `superpanels-gui` crate; `tauri.conf.json` configured for Linux build.
- [x] CSP locked down to local resources only.
- [x] Allowlist of Tauri APIs: only what we actually use (filesystem in library roots, IPC, tray, dialog/open).
- [x] Single-window app; window state (size, position) persists to state.json.
- [x] Build wired into the workspace; `cargo run -p superpanels-gui` opens the window.

## 3.2 Tauri command bindings
- [x] Every command in SPEC §12.4 declared with `#[tauri::command]`.
- [x] Each command is a 3-line wrapper: parse args → call core → return `Result`.
- [x] Commands try the daemon first; fall back to in-process if no daemon.
- [x] `serde_json` types shared between Rust and Svelte via `ts-rs` (compile-time TS type generation).

## 3.3 Svelte 5 frontend skeleton
- [x] Vite + Svelte 5 + TypeScript; SvelteKit not used (overkill).
- [x] Tailwind CSS + a small custom `theme.css` for tokens.
- [x] Stores: `profile.ts`, `monitors.ts`, `library.ts`, `toast.ts`.
- [x] Layout: left rail (Profiles / Library / Settings tabs), main area.

## 3.4 Monitor canvas — basic
- [x] Canvas component renders rectangles to scale based on `detect_monitors`.
- [x] No image yet — just outlines and bezel bars on a flat background. Proves the geometry pipeline.
- [x] Resize-aware: re-renders on window resize.

## 3.5 Profile list
- [x] Reads from `list_profiles`.
- [x] Click-to-apply via `apply_profile`.
- [x] Active profile indicator (re-fetched from `current_state`).

## 3.6 System tray
- [x] SVG icon (light/dark variants).
- [x] Left click: show/hide window.
- [x] Right click menu: profile list with active tick, separator, Next/Prev/Pause, Settings, Quit.
- [x] Tray menu state synchronised to daemon state via a periodic 1-second poll (Tauri tray APIs don't observe; polling is fine and cheap).
- [x] Tooltip showing active profile + current filename.

## 3.7 Autostart
- [x] Settings toggle "Start at login" writes/removes `~/.config/autostart/superpanels.desktop`.
- [x] First-run modal asks; choice persisted.

## 3.8 Notifications
- [x] `notify-rust`-backed; off by default, opt-in via settings.
- [x] Surfaces apply errors regardless of opt-in (errors-only mode).

**Risks for this phase.**
- Tauri tray support on Wayland is uneven. KDE has good tray support; GNOME requires AppIndicator extensions; Sway needs `waybar` or similar with StatusNotifierItem support. Document this clearly; the GUI degrades gracefully if no tray host is present (no icon, but window still works).
- WebKitGTK on KDE may not pick up the right theme. Provide `--gtk-theme-from-kde` env var or honour `kdeglobals`.
