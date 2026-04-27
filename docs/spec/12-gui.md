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

The canvas is the heart of the UI. Five-layer compositing:

1. **Base layer:** wallpaper image, scaled to the canvas size and positioned per the live offset.
2. **Mask layer:** dark semi-transparent overlay covering the entire canvas.
3. **Cut-out:** `destination-out` composite punches a hole through the mask for each monitor rectangle. The monitors appear as lit windows revealing the image beneath; everything outside is dimmed.
4. **Bezel bars:** solid dark rectangles between monitors, sized proportionally to the configured physical gap.
5. **Outlines & labels:** thin light borders, monitor name labels, and on-hover tooltips.

**Accuracy:**
- Monitors rendered at correct relative *physical* sizes in mm. A 27" 4K and a 24" 1080p look visibly different in the canvas.
- Correct relative positions: a monitor sitting physically lower renders lower.
- Portrait monitors rendered as rotated rectangles.
- Bezel bars sized proportionally to the configured mm gap.

**Interactivity:**
- Drag the image to reposition; on each drag tick (16 ms), an IPC `preview_crop` call returns updated `Vec<CropSpec>` and the canvas redraws.
- Bezel sliders update in real time — gap bars grow/shrink, image shifts to compensate.
- Hover a monitor: glow highlight + tooltip showing pixel range and physical-size info.
- Click a monitor: side popout showing the exact crop as it will appear on that screen.
- `R` resets the offset to centred.
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
```

`IpcError` is a thin enum that flattens the typed errors from `superpanels-core` (`DetectError`, `BackendError`, `LayoutError`, `ConfigError`) into a single shape suitable for serialisation to the frontend. `Profile`, `BezelConfig`, `FitMode`, `CropSpec`, and friends are the types from §3 (after the rework — see §3.4 for `Profile`'s shape).

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
| `R` | Reset image offset |
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
