# Configuration & state

Reference for the on-disk files Superpanels reads and writes. Code: [`crates/superpanels-core/src/config.rs`](../../crates/superpanels-core/src/config.rs) (and `config/*.rs`); state lives in the library DB and small JSON files under `$XDG_STATE_HOME`.

## Files at a glance

| File | Path | Owner |
|---|---|---|
| Config | `$XDG_CONFIG_HOME/superpanels/config.toml` | user-edited or written by GUI |
| Library DB | `$XDG_DATA_HOME/superpanels/library.db` | daemon (SQLite, schema-versioned) |
| Slideshow state | `$XDG_STATE_HOME/superpanels/slideshow-state.json` | daemon |
| Resume state | `$XDG_STATE_HOME/superpanels/resume-state.json` | daemon (read by GUI as fallback) |
| Window geometry | `$XDG_STATE_HOME/superpanels/window.json` | GUI on window close |
| Tray icon style | `$XDG_STATE_HOME/superpanels/tray.json` | GUI (Settings → Appearance) |
| Autostart desktop | `$XDG_CONFIG_HOME/autostart/superpanels.desktop` | GUI when autostart is on |
| App-menu desktop | `$XDG_DATA_HOME/applications/superpanels-gui.desktop` | GUI on every launch (taskbar icon on Wayland) |
| App icons | `$XDG_DATA_HOME/icons/hicolor/<size>/apps/superpanels-gui.png` | GUI on every launch |

Tilde expansion is supported in path-valued fields (`library.roots`, etc.).

## `config.toml`

Parsed with `serde` + `toml`. Every section has defaults — a missing or empty file is valid.

```toml
[general]
default_profile = "home"          # profile to apply on daemon start when state has no active profile
autostart       = true            # write the XDG autostart .desktop entry
notifications   = "errors"        # off | errors | all
theme           = "auto"          # auto | light | dark

[backend]
prefer          = "auto"          # auto | kde | gnome | sway | hyprland | feh | custom
custom_command  = ""              # required when prefer = "custom"; supports {image_N} / {monitor_N}

[library]
roots           = ["~/Pictures/walls"]
recursive       = true
thumbnail_size  = 512             # px on the long edge
auto_scan       = true            # rescan on FS change

# Per-monitor physical sizes. Detection returns pixels; this gives us mm.
# Match by stable_id when the detector supplied one; fall back to name for
# compositors that don't expose one. At least one of stable_id / name is
# required. The GUI's first-run flow writes these blocks for you.
[[monitor]]
stable_id   = "f7f0f124-9e9b-4ef0-91a7-426d58091760"  # KDE per-output UUID
name        = "DP-1"
physical_mm = [597.0, 336.0]                          # 27" 16:9 landscape

[[monitor]]
name        = "HDMI-A-1"
physical_mm = [527.0, 296.0]                          # 24" 16:9

[[profile]]
name     = "home"
topology = "<opaque hex hash>"     # captured at authoring time

# A Standard profile is a stack of free-positioned image layers (bottom-to-top).
# A single image is just a one-layer Standard — there is no separate single
# mode. Each layer carries its own canvas rectangle in mm.
[profile.body]
type = "standard"

[[profile.body.layers]]
path          = "~/walls/pano.jpg"
image_rect_mm = { x_mm = 0.0, y_mm = 0.0, w_mm = 1202.0, h_mm = 336.0 }

[profile.monitor_state."f7f0f124-..."]
x_mm = 0.0
y_mm = 0.0

[profile.monitor_state."HDMI-A-1"]
x_mm = 605.0
y_mm = 0.0

[[profile]]
name     = "work"
topology = "<opaque hex hash>"

[profile.body]
type = "slideshow"
# The profile-level canvas rectangle: the baseline placement for the live
# image, and (under `uniform_layout`) the rect applied to every untuned image.
image_rect_mm = { x_mm = 0.0, y_mm = 0.0, w_mm = 1124.0, h_mm = 336.0 }

[profile.body.source]
# Optional: apply the profile-level layout (monitor_state + image_rect_mm)
# to every image instead of cover-fitting each untuned image at its own
# aspect. Suits sets authored at one fixed resolution (the GUI's
# "apply to all" button sets this, warning when the set mixes aspects).
# Default false; omitted when false.
uniform_layout = false

# Mixed source list: any number of live folders and hand-picked images.
# Folders are re-scanned on each pool resolve, so new files join the rotation.
[[profile.body.source.images.sources]]
type      = "folder"
path      = "~/walls/work"
recursive = false

[[profile.body.source.images.sources]]
type = "image"
path = "~/walls/specials/skyline.png"

[profile.body.source.config]
interval_secs       = 1800
sort                = "shuffle"
recent_history_size = 10
on_start            = "resume"

# Optional per-image canvas overrides: a sparse map keyed by the image's
# absolute path, holding the same placement + image-rect snapshot a profile
# persists at top level. Resolved daemon-side at apply time, so a hand-tuned
# image keeps its layout with the GUI closed. Renaming or moving the file
# drops the tweak (the key no longer matches). Images WITHOUT an override
# keep the profile's monitor placements but are cover-fit at their own
# aspect ratio (unless `uniform_layout` is set) — then the profile-level
# image_rect_mm applies to every untuned image.
[profile.body.source.overrides."/home/me/walls/specials/skyline.png"]
image_rect_mm = { x_mm = 0.0, y_mm = -40.0, w_mm = 1900.0, h_mm = 720.0 }

[profile.body.source.overrides."/home/me/walls/specials/skyline.png".monitor_state."f7f0f124-..."]
x_mm = 0.0
y_mm = 0.0

[[schedule]]
display_name = "Day mode"
profile      = "home"
enabled      = true

[schedule.trigger]
type   = "daily"
hour   = 8
minute = 0

schedules_paused = false
```

Enums use `serde`'s tagged representation (`#[serde(tag = "type", rename_all = "snake_case")]`). This is the source of truth for the on-disk format; the Rust types in `superpanels-core` are the source of truth for the runtime model.

`ImageSet` also deserializes the pre-1.0 single-variant forms — `images = { type = "folder", path, recursive }` and `images = { type = "playlist", paths }` — lifting them into the `sources` list; serialization always emits the current form.

### Profiles

A profile is **the mode the user is in**, not a one-shot apply request. It bundles the canvas state — image transform, per-monitor placements — captured under a specific monitor topology.

- `monitor_state: HashMap<String, MonitorPlacement>` — physical mm placements keyed by `stable_id` (or `name` fallback). Gaps between monitors fall out of these placements; there is no separate bezel field.
- `topology: TopologyFingerprint` — opaque hash over the connected `stable_id`s + rotations the profile was authored against. Compared by equality at apply time; mismatch disables the profile until the user re-authors via the **topology-repair flow**.
- `body: ProfileBody` — one of three flat variants: `Standard { layers }`, `Slideshow { source, image_rect_mm }`, or `PerMonitor { assignments, fit }`. The flat split keeps "per-monitor + slideshow" unrepresentable.
- `Standard { layers: Vec<StandardLayer> }` — one or more free-positioned images on the canvas at once. **A single image is just a one-layer Standard** — there is no separate single-image mode. Each `StandardLayer` is `{ path, image_rect_mm }` in canvas mm-space; `layers` is bottom-to-top stacking order. At apply, every monitor alpha-composites the layers that overlap it (top over bottom), uncovered regions render black — so one image can slice across two monitors while another fills a third. Shares the profile-level `monitor_state` (placements/gaps). See `docs/reference/layout-math.md` § Standard.
- `Slideshow { source, image_rect_mm }` — `source` holds the image set, timing `config`, sparse per-image `overrides`, and the `uniform_layout` flag; `image_rect_mm` is the profile-level canvas rectangle. A slideshow source may carry sparse per-image `overrides` (placements + image rect keyed by absolute path); the daemon's slideshow-apply choke point swaps them in when that image comes up, and the GUI canvas follows live. Untuned slideshow images use the profile's placements with a per-image cover-fit rect (aspect preserved, sliced across the placed desktop plane), unless the slideshow sets `uniform_layout` — then the profile-level `image_rect_mm` applies to every untuned image.

A profile is **disabled** when any of:

- Topology mismatch (connected set or rotation differs from fingerprint).
- A Standard layer's image is missing, or the Standard profile has no layers (`standard_empty`).
- Slideshow image set has no sources yet (`slideshow_empty` — the GUI offers the "add images" flow instead of repair).
- Slideshow image set has no usable source (every folder missing/empty and every picked image missing — one healthy source keeps the profile enabled).
- Referenced `MonitorRef` in a `PerMonitor` body is not connected.
- Required `physical_size_mm` missing for any expected monitor.

Disabled profiles show greyed-out with their disable reasons; they don't auto-apply when a schedule fires.

### Schedules

Top-level concept driving profile switches by clock, separate from slideshow timing.

- `Trigger::Daily { hour, minute }` or `Trigger::Cron { expr }`.
- Conflict prevention: two enabled rules that fire at the same minute on a representative day are unrepresentable in saved state; save is blocked.
- Boot catch-up: on daemon start, the most recent past fire-time today is applied if the active profile differs.
- Preemption: schedule fires preempt manual choice. The escape is the master `schedules_paused` toggle (mirrored in the tray menu).

## Validation

Config is validated at load time **and** before every save (`Config::save_to`). Load rejects an invalid file wholesale, so the save-side check exists to keep any IPC write path from persisting a config that would brick the next start. Invalid configs **do not crash** — they return an error with the exact field path (`profile[1].monitor_state.DP-1: …`) and the previous wallpaper remains on the desktop.

Bounded invariants enforced in `superpanels-core::ipc::validate` (so daemon and in-process IPC share one source of truth):

| Input | Bound |
|---|---|
| `physical_mm` components | finite, `> 0`, `≤ 10_000` mm |
| `stable_id`, `name` | non-empty, `≤ 256` chars, no control chars |
| `Profile.name` | non-empty, `≤ 64` chars post-trim |
| `Profile.body::Slideshow.source.images.sources` | `≤ 10_000` entries |
| `Profile.body::Slideshow.source.overrides` | `≤ 1_000` entries; placements + rect finite |
| `Config.profiles` | `≤ 256` entries |
| `Config.monitors` | `≤ 64` entries |
| `offset`/`image_size_px` components | finite, `|v| ≤ 1_000_000` |
| `tag` (`library_tag`) | non-empty after trim, `≤ 64` chars |
| `library_list.limit` | capped at `MAX_LIBRARY_PAGE = 1000` |

Bound choices target human plausibility, not perf. Hitting any cap means the input is malicious or malformed.

## Hot reload

On `SIGHUP` the daemon reloads config. The CLI loads config fresh on each invocation. The GUI's "Save" button writes the file and triggers reload via IPC.

## Library DB

SQLite at `$XDG_DATA_HOME/superpanels/library.db`. Schema versioned via `PRAGMA user_version`; migrations are pure-Rust, idempotent, applied on startup. Tables: `entries`, `tags`, `entry_tags`, `roots`. See [`crates/superpanels-core/src/library.rs`](../../crates/superpanels-core/src/library.rs).

## State files

`resume-state.json` (`superpanels-core/src/resume.rs`) records the active profile, last-apply backend, and last-apply timestamp; the daemon rewrites it on every successful apply and restores it at startup — `general.default_profile`, when set, overrides the resumed profile. The GUI's in-process `current_state` fallback reads the same file so a daemon-less launch still lands on the last profile. `slideshow-state.json` records slideshow position/history. **Never** persist per-monitor temp file paths — those are wiped at the start of each apply, so persisting them would always be stale. If the daemon needs to repaint after re-detection, it re-runs the pipeline from the source.

`window.json` (window geometry) and `tray.json` (tray icon style: `white` or `blue`) are GUI-only and read at launch, before the webview loads — which is why the tray style lives here rather than in the frontend's localStorage. Missing or malformed files fall back to defaults.

## Migration

Each persistent file carries a `version` field. On load, if the version is older than the binary expects, a migration step runs and a backup is left at `<file>.v<N>.bak`. If the version is newer (downgrade), the binary refuses to write and prints a clear error.
