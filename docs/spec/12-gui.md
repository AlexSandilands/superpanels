# 12. GUI — Tauri + Svelte

## 12.1 Window layout

The main window is a three-panel layout, with a tabbed left rail (Profiles / Library / Settings).

```
┌────────────────────────────────────────────────────────────────────┐
│ Superpanels                                              [—][▢][×] │
├──────────┬─────────────────────────────────────────────────────────┤
│ Profile  │  ┌──────────────────────────────────────────────────┐   │
│ Library  │  │             Monitor preview canvas               │   │
│ Settings │  │                                                  │   │
│ ─────    │  │     ┌────────┐  ██  ┌────────┐                   │   │
│ ▶ home   │  │     │        │  ██  │        │                   │   │
│   work   │  │     │        │  ██  │        │                   │   │
│   movie  │  │     └────────┘  ██  └────────┘                   │   │
│          │  │           drag image to reposition ↕             │   │
│ [+ New]  │  └──────────────────────────────────────────────────┘   │
│          │  ┌──────────────────────────────────────────────────┐   │
│          │  │ Image:      [/home/alex/walls/pano.jpg] [Browse] │   │
│          │  │ Bezel H:    [───●─────────] 8 mm                 │   │
│          │  │ Bezel V:    [─●───────────] 5 mm                 │   │
│          │  │ Fit:        [Fill ▾]                             │   │
│          │  │ Slideshow:  [Off ▾]                              │   │
│          │  │                                  [Apply]         │   │
│          │  └──────────────────────────────────────────────────┘   │
└──────────┴─────────────────────────────────────────────────────────┘
```

## 12.2 Library view

Selectable from the left rail. Grid of thumbnails (320 px), filterable by tag, aspect ratio, and resolution. Right-click on a thumbnail: "Apply now" / "Set for monitor…" / "Add tag" / "Toggle favourite" / "Reveal in file manager". Click to focus; double-click to apply.

## 12.3 Monitor preview canvas

The canvas is the heart of the UI. Phase 4c uses a free-positioning model:
the image floats freely across the canvas; monitors are crop windows that
hover over it. Seven-layer compositing:

1. **Background:** canvas-wide slate fill.
2. **Image rectangle:** drawn at `(offset, image_size_px)` across the whole canvas, no layout-bbox clip. When `image_size_px` is `None`, the rectangle is derived from `FitMode` (legacy behaviour).
3. **Off-monitor dim:** translucent black over everything outside the monitor rects. Toggleable (the `D` key flips it).
4. **Per-monitor backing:** opaque black fill at each monitor's rect — what the user actually sees on the panel when the source rectangle doesn't cover it (the letterbox pixels).
5. **Image inside monitors:** the image redrawn at full alpha, clipped to the union of the monitor rects.
6. **Bezel bars:** solid dark rectangles between monitors, sized proportionally to the configured physical gap.
7. **Monitor chrome:** thin coloured outlines, labels, hover glow, and corner resize handles around the image rectangle (when the user has pinned an explicit transform).

`SpanProfile.image_size_px: Option<[u32; 2]>` toggles between the two modes.
`None` defers to `FitMode`; `Some([w, h])` is the GUI's free transform — the
canvas, the Rust crop algorithm, and the apply pipeline all agree the image
rectangle on the canvas is `(offset.x, offset.y, w, h)` regardless of fit.

**Accuracy:**
- Monitors rendered at correct relative *physical* sizes in mm. A 27" 4K and a 24" 1080p look visibly different in the canvas.
- Correct relative positions: a monitor sitting physically lower renders lower.
- Portrait monitors rendered as rotated rectangles.
- Bezel bars sized proportionally to the configured mm gap.

**Interactivity:**
- Drag inside the image rectangle to **pan**; the offset is committed on pointer-up.
- Drag a corner handle to **resize**; aspect-locked by default. Hit priority is corner handles → image body → monitor rect.
- "Cover all monitors" snaps offset + `image_size_px` to the smallest scale that covers the union of monitor rects (useful for portrait + landscape pairs that share no common axis).
- Bezel sliders update in real time — gap bars grow/shrink, image shifts to compensate.
- Hover a monitor: glow highlight + tooltip showing pixel range and physical-size info.
- Click a monitor: side popout showing the exact crop as it will appear on that screen.
- Pinch/scroll on the canvas: zoom (within 0.5×–2.0×) for inspection, doesn't change the applied result.

**Apply animation:** a fast (< 400 ms) flash per monitor confirming the apply.

**No full-res image processing on interaction.** The Rust side computes crop coordinates (pure arithmetic, microseconds). The canvas draws rectangles using the *thumbnail* of the image, scaled. The full-res pipeline only runs on Apply.

## 12.4 IPC commands (Tauri, mirrored 1:1 in the daemon's IPC)

```rust
#[tauri::command] fn detect_monitors() -> Result<Vec<Monitor>, IpcError>;
#[tauri::command] fn list_profiles() -> Result<Vec<Profile>, IpcError>;
#[tauri::command] fn apply_profile(name: String) -> Result<AppliedReport, IpcError>;
#[tauri::command] fn save_profile(profile: Profile) -> Result<(), IpcError>;
#[tauri::command] fn delete_profile(name: String) -> Result<(), IpcError>;
#[tauri::command] fn preview_crop(
    image: String,
    offset_px: (i32, i32),
    bezels: BezelConfig,
    fit: FitMode,
) -> Result<Vec<CropSpec>, IpcError>;
#[tauri::command] fn library_list(filter: LibraryFilter) -> Result<Vec<LibraryEntry>, IpcError>;
#[tauri::command] fn library_thumbnail(path: String) -> Result<Vec<u8>, IpcError>; // PNG/WebP bytes
#[tauri::command] fn library_tag(path: String, tag: String, on: bool) -> Result<(), IpcError>;
#[tauri::command] fn slideshow_next() -> Result<AppliedReport, IpcError>;
#[tauri::command] fn slideshow_prev() -> Result<AppliedReport, IpcError>;
#[tauri::command] fn slideshow_pause(paused: bool) -> Result<(), IpcError>;
#[tauri::command] fn get_config() -> Result<Config, IpcError>;
#[tauri::command] fn save_config(config: Config) -> Result<(), IpcError>;
#[tauri::command] fn redetect() -> Result<Vec<Monitor>, IpcError>;
#[tauri::command] fn current_state() -> Result<RuntimeState, IpcError>;  // active profile, current source, slideshow position
#[tauri::command] fn set_autostart(enabled: bool) -> Result<(), IpcError>; // writes/removes the user XDG autostart .desktop entry
#[tauri::command] fn get_autostart() -> Result<bool, IpcError>;            // reads the current autostart state
```

The GUI also exposes a local-only `source_thumbnail(path: String)` command for selected/dropped source preview bytes. It is intentionally not mirrored in daemon IPC because it represents a webview-local, user-mediated file choice rather than library state. `set_autostart`/`get_autostart` are similarly GUI-local — they touch the user's XDG autostart directory directly.

`IpcError` is a thin enum that flattens the typed errors from `superpanels-core` (`DetectError`, `BackendError`, `LayoutError`, `ConfigError`) into a single shape suitable for serialisation to the frontend. `Profile`, `MonitorPlacement`, `FitMode`, `CropSpec`, and friends are the types from §3.

## 12.4.1 Profile manager

The profile manager opens as a modal overlay in the main window, shaped like `LibraryModal` — left rail (search + list), main detail pane on a `Backdrop`. Same-window overlay rather than a dedicated Tauri webview keeps the capability surface small (no `core:webview:allow-create-webview-window`) and lets the manager share the main window's profile/canvas state without IPC round-trips. Reachable from:

- The tray menu's "Open profile manager…" item.
- The top-nav profile-manager icon button.
- The tray-pill profile dropdown's "Open profile manager…" footer.

List view per profile: thumbnail, name, colour swatch, last-applied recency, validity badge, "Authored for: {topology}" chip when current topology differs.

Per-profile actions: Apply, Rename (inline), Edit colour swatch (palette popover), Edit description, Open referenced file/folder in the OS file manager, Duplicate, Export (TOML bundle), Delete (confirm dialog).

Top-level actions: New profile (disabled — directs the user to the canvas's "Save as new" button), Import bundle, Empty-state CTA.

Disabled-profile rows are greyed-out, list every applicable disable reason inline, and offer a "Repair" button that triggers the topology-repair flow (§9.1.1).

## 12.4.2 Tray selector

The tray pill in the title bar is purely a **switcher**. Creation actions live elsewhere (the manager window for blank/duplicate/import; the top-nav "Save as new" button for capture-current).

- Outside click closes the dropdown.
- Long names truncate with ellipsis + native tooltip.
- Sort: pinned/active profile first, then by `last_applied_at` desc.
- Surfaces the active schedule rule when present ("Auto: switching to dark at 18:00").
- Footer items: "Open profile manager…" and "Pause schedules" toggle.
- Empty state: clear "No profiles yet — open the profile manager" CTA.

## 12.4.3 Top-nav button cluster

The top-nav row exposes the four canvas-authoring actions, in order: **Apply**, **Save**, **Save as new**, **Revert**. All four are always visible. Their enable rules and visual states track §9.1.2.

- **Apply** — `Enter` shortcut. Pushes the current canvas to the desktop (`apply_canvas` IPC). Disabled while the draft has no usable image source or while a save is in flight.
- **Save** — `Ctrl+S` shortcut. Commits the current canvas state into the active profile's TOML (`save_profile`). Disabled when there is no active profile. Rendered with the default white tint when the canvas matches the persisted state and with `--accent` when dirty so the user can tell at a glance that there are unsaved edits.
- **Save as new** — `Ctrl+Shift+S` shortcut. Opens a dialog with name (required, validated for uniqueness), colour swatch (curated 12-swatch palette), and an optional description. Confirm creates a new profile capturing the current canvas state (image source, transform, `monitor_state`, `topology` from live OS) and switches to it as active. Disabled when the canvas has no image (tooltip: "no image on canvas").
- **Revert** — re-pulls the active profile's persisted state into the canvas (overrides + draft + image transform). Disabled when the canvas is clean OR when there is no active profile.

A confirm-discard modal (`ConfirmDiscardModal`) interposes whenever the user initiates a profile switch (tray pill, profile-manager Apply, top-nav profile pill) while the canvas is dirty, and on `WindowEvent::CloseRequested` while the canvas is dirty. Cancel keeps the canvas; Confirm drops the edits and proceeds. Schedule-driven switches do not trigger this modal — see §9.3.3.

## 12.4.4 Settings → Schedules

Populates the previously-empty Schedules tab.

- Rule list. Each row: enabled toggle, trigger summary, target profile, "next fires at HH:MM" hint, edit/delete.
- Add-rule form: trigger type (daily / cron), parameters, target profile dropdown.
- **Conflict prevention:** save is blocked when a new/edited rule would fire at the same minute as another enabled rule.
- Master "pause all schedules" toggle (mirrored in tray).

## 12.5 Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Enter` | Apply current settings |
| `Ctrl+N` | New profile |
| `Ctrl+S` | Save current profile |
| `Ctrl+Shift+S` | Save profile as… |
| `Ctrl+1/2/3` | Switch to top three profiles |
| `Space` | Pause/resume slideshow |
| `→` / `←` | Slideshow next / prev |
| `R` | Reset image transform (offset + image_size_px) |
| `D` | Toggle off-monitor dim |
| `F5` | Re-detect monitors |
| `Ctrl+,` | Open settings |
| `Ctrl+L` | Focus library search |
| `Esc` | Close modal / return to canvas |

## 12.6 Empty/first-run state

- First run with no monitors detected: canvas shows a placeholder with "Couldn't detect monitors — try `superpanels detect --debug`" and a button to open the manual override dialog.
- **First run with monitors detected but no physical-size config:** the canvas is rendered (so the user sees their layout) but each monitor is annotated with "physical size unknown — click to configure" and the Apply button is disabled with a tooltip explaining why. Clicking a monitor opens a small modal with two input modes:
  - *Diagonal + aspect ratio* (e.g. `27"`, `16:9`, landscape) — auto-computes physical mm. The default for "common monitor sizes" — a 27" 16:9 panel comes out to ≈ 597 × 336 mm.
  - *Direct mm entry* — width and height in mm.
  The chosen values are written to `[[monitor]]` in the config (§14.1), keyed on `stable_id` (or `name` if no stable ID).
- First run with no profile: canvas shows a single example monitor outline + "Drop an image here or pick one from the library" prompt.
- First run with no library roots: a friendly onboarding modal asks "Where are your wallpapers?" and registers a root.

## 12.7 Toasts & error surfacing

Backend errors surface as a non-blocking toast at bottom-right with the error message and a "Copy details" button. The full error (including subprocess stderr) is available in the toast's expand-disclosure and in the log file.

## 12.8 Theming

- Default dark theme, inspired by KDE Breeze Dark.
- Auto light/dark via `prefers-color-scheme`, override in settings.
- Accent colour follows `kdeglobals` `[General] AccentColor` when on KDE; otherwise a sensible blue. User can override.
