# Phase 3 — Tauri shell & tray

**Goal.** `superpanels gui` opens a window. Tray icon works. Profile switching from the tray is functional. UI is functional but not yet polished.

**Definition of done.**
- [ ] `superpanels gui` opens a window with a working monitor canvas (rectangles only — no image yet) and a profile list.
- [ ] System tray shows the icon with a working profile-switch menu.
- [ ] All Tauri IPC commands from SPEC §12.4 are wired through to the daemon's IPC.
- [ ] Closing the GUI window does not stop the daemon. Quit from tray menu stops both.
- [ ] Auto-start opt-in writes `~/.config/autostart/superpanels.desktop`.

## 3.1 Tauri scaffold
- [ ] `superpanels-gui` crate; `tauri.conf.json` configured for Linux build.
- [ ] CSP locked down to local resources only.
- [ ] Allowlist of Tauri APIs: only what we actually use (filesystem in library roots, IPC, tray, dialog/open).
- [ ] Single-window app; window state (size, position) persists to state.json.
- [ ] Build wired into the workspace; `cargo run -p superpanels-gui` opens the window.

## 3.2 Tauri command bindings
- [ ] Every command in SPEC §12.4 declared with `#[tauri::command]`.
- [ ] Each command is a 3-line wrapper: parse args → call core → return `Result`.
- [ ] Commands try the daemon first; fall back to in-process if no daemon.
- [ ] `serde_json` types shared between Rust and Svelte via `ts-rs` (compile-time TS type generation).

## 3.3 Svelte 5 frontend skeleton
- [ ] Vite + Svelte 5 + TypeScript; SvelteKit not used (overkill).
- [ ] Tailwind CSS + a small custom `theme.css` for tokens.
- [ ] Stores: `profile.ts`, `monitors.ts`, `library.ts`, `toast.ts`.
- [ ] Layout: left rail (Profiles / Library / Settings tabs), main area.

## 3.4 Monitor canvas — basic
- [ ] Canvas component renders rectangles to scale based on `detect_monitors`.
- [ ] No image yet — just outlines and bezel bars on a flat background. Proves the geometry pipeline.
- [ ] Resize-aware: re-renders on window resize.

## 3.5 Profile list
- [ ] Reads from `list_profiles`.
- [ ] Click-to-apply via `apply_profile`.
- [ ] Active profile indicator (re-fetched from `current_state`).

## 3.6 System tray
- [ ] SVG icon (light/dark variants).
- [ ] Left click: show/hide window.
- [ ] Right click menu: profile list with active tick, separator, Next/Prev/Pause, Settings, Quit.
- [ ] Tray menu state synchronised to daemon state via a periodic 1-second poll (Tauri tray APIs don't observe; polling is fine and cheap).
- [ ] Tooltip showing active profile + current filename.

## 3.7 Autostart
- [ ] Settings toggle "Start at login" writes/removes `~/.config/autostart/superpanels.desktop`.
- [ ] First-run modal asks; choice persisted.

## 3.8 Notifications
- [ ] `notify-rust`-backed; off by default, opt-in via settings.
- [ ] Surfaces apply errors regardless of opt-in (errors-only mode).

**Risks for this phase.**
- Tauri tray support on Wayland is uneven. KDE has good tray support; GNOME requires AppIndicator extensions; Sway needs `waybar` or similar with StatusNotifierItem support. Document this clearly; the GUI degrades gracefully if no tray host is present (no icon, but window still works).
- WebKitGTK on KDE may not pick up the right theme. Provide `--gtk-theme-from-kde` env var or honour `kdeglobals`.
