# 14. Configuration & state

## 14.1 Config file

Location: `$XDG_CONFIG_HOME/superpanels/config.toml` (default: `~/.config/superpanels/config.toml`).

Parsed with `serde` + `toml`. All fields have sane defaults so a minimal config (or no config) is valid.

```toml
[general]
default_profile  = "home"
autostart        = true              # write desktop file on first run
notifications    = "errors"          # off | errors | all
theme            = "auto"            # auto | light | dark

[backend]
prefer           = "auto"            # auto | kde | gnome | sway | hyprland | feh | custom
custom_command   = ""                # only when prefer = "custom"; supports {image_N}, {monitor_N}

[library]
roots            = ["~/Pictures/walls"]
recursive        = true
thumbnail_size   = 320
auto_scan        = true              # rescan on FS change

# Per-monitor physical sizes. The detector gives us pixels; this gives us
# millimetres. Match by stable_id when the detector supplied one (KDE per-output
# UUID etc.); fall back to name for compositors that don't expose a stable ID.
# At least one of `stable_id` / `name` must be set. The GUI's first-run flow
# writes these blocks for you.
[[monitor]]
stable_id     = "f7f0f124-9e9b-4ef0-91a7-426d58091760"  # KDE UUID
name          = "DP-1"                                  # informational; match falls back to this
physical_mm   = [597, 336]                              # 27" 16:9 landscape

[[monitor]]
name          = "HDMI-A-1"
physical_mm   = [527, 296]                              # 24" 16:9

[[profile]]
name = "home"
bezels = { horizontal_mm = 8.0, vertical_mm = 5.0 }

[profile.body]
type   = "span"
fit    = "fill"
offset = [0, 0]

[profile.body.source]
type = "single"
path = "~/walls/pano.jpg"

[[profile]]
name = "work"
bezels = { horizontal_mm = 8.0, vertical_mm = 5.0 }

[profile.body]
type = "span"
fit  = "fill"

[profile.body.source]
type = "slideshow"

[profile.body.source.images]
type      = "folder"
path      = "~/walls/work"
recursive = false
filters   = { aspect_ratios = "wide" }

[profile.body.source.config]
interval_secs       = 1800
sort                = "shuffle"
recent_history_size = 10
on_start            = "resume"
```

Enums use `serde`'s tagged representation (`#[serde(tag = "type", rename_all = "snake_case")]`) so the TOML stays readable and round-trip-stable. This is the source of truth for the on-disk format; the Rust types in §3 are the source of truth for the runtime model.

## 14.2 Validation

Config is validated at load time. Invalid configs *do not crash*; they return an error with the exact field path (`profile[1].slideshow.interval_secs: must be > 0`) and the previous wallpaper remains.

## 14.3 Hot reload

On `SIGHUP` the daemon reloads config. The CLI does not need this — it loads config fresh on each invocation. The GUI's "Save" button writes the file and triggers reload via IPC.

## 14.4 Runtime state

Location: `$XDG_STATE_HOME/superpanels/state.json` (default: `~/.local/state/superpanels/state.json`).

```json
{
  "active_profile": "home",
  "current_source": { "kind": "single", "path": "~/walls/pano.jpg" },
  "slideshow": {
    "profile": "home",
    "current_index": 17,
    "history": ["walls/a.jpg", "walls/b.jpg"],
    "paused": false
  },
  "last_apply": "2026-04-26T15:42:00Z",
  "version": 1
}
```

`current_source` records the *source* (a serialised `SpanSource` or `PerMonitorProfile.assignments`), never the per-monitor temp file paths — those are wiped at the start of each apply (§8.5) so persisting them would always be stale. If the daemon needs to repaint after a re-detection, it re-runs the pipeline from the source. State is restored on daemon start so the slideshow doesn't loop back to the start after every reboot.

## 14.5 Library DB

SQLite, location `$XDG_DATA_HOME/superpanels/library.db`. Schema versioned via `PRAGMA user_version`; migrations are pure-Rust, idempotent, applied on startup. Tables: `entries`, `tags`, `entry_tags`, `roots`.

## 14.6 Migration

Each persistent file (`config.toml`, `state.json`, `library.db`) carries a `version` field. On load, if the version is older than the binary expects, a migration step runs and a backup is left at `<file>.v<N>.bak`. If the version is newer (downgrade), the binary refuses to write and prints a clear error.
