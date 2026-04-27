# 6. Display detection

Detection produces a *layout-only* `Monitor` struct (positions, resolutions, scale, rotation, name, optional `stable_id`). Physical sizes are merged in afterwards from per-monitor config (§14.1). This split exists because no Linux compositor CLI reliably exposes `physical_size_mm` — KDE's `kscreen-doctor -o` doesn't, and EDID-from-sysfs is sometimes wrong on real hardware. Asking the user for monitor mm is one extra config step on top of the bezel mm they're already providing; in exchange we get a uniform detection surface and predictable correctness.

Detection is attempted in priority order, stopping at the first detector that succeeds and returns a non-empty monitor list.

| Priority | Detector | Detection condition | Source of truth |
|---|---|---|---|
| 1 | `kscreen-doctor -o` | KDE session detected | KDE Plasma. Provides per-output UUID usable as `stable_id`. Run with `NO_COLOR=1` so the parser doesn't have to strip ANSI. |
| 2 | `hyprctl monitors -j` | `$HYPRLAND_INSTANCE_SIGNATURE` set | Hyprland JSON output. |
| 3 | `swaymsg -t get_outputs` | `$SWAYSOCK` set | Sway-native; more reliable than `wlr-randr` on Sway specifically. |
| 4 | `wlr-randr --json` | wlroots compositor without `$SWAYSOCK` | Generic wlroots JSON output. |
| 5 | `xrandr --verbose` | `$DISPLAY` set, Wayland not in use | X11 fallback. |
| 6 | Manual override | `--monitors` CLI flag, or config | Always wins if set. |

Each detector runs as a subprocess with a **5-second timeout**. If all fail, return: `Could not detect monitor layout. Try --monitors WxH+X+Y,WxH+X+Y... to specify manually, or run 'superpanels detect --debug' to see what was attempted.`

After detection, layout `Monitor`s are merged with the user's per-monitor config (§14.1) — matching by `stable_id` first, falling back to `name`. The merged `physical_size_mm` is `None` for any monitor not yet in config; the bezel-math entry point (`compute_crop_specs`) returns `LayoutError::PhysicalSizeMissing { monitors: Vec<MonitorRef> }` listing exactly which monitors need configuration. The CLI surfaces this as a friendly "run `superpanels monitor configure DP-1` to provide its dimensions" message; the GUI surfaces it as a first-run modal.

## 6.1 Detector contract

Each detector implements:

```rust
trait DisplayDetector {
    fn name(&self) -> &str;
    fn availability(&self) -> Availability; // env-var or PATH check; never spawn
    fn detect(&self) -> Result<Vec<Monitor>, DetectError>;
}

enum Availability {
    Available,
    ToolMissing { tool: &'static str },     // e.g. "kscreen-doctor not on PATH"
    WrongEnvironment { reason: &'static str }, // e.g. "$KDE_FULL_SESSION not set"
    Disabled,                               // user pinned a different detector
}

#[derive(Debug, thiserror::Error)]
enum DetectError {
    #[error("subprocess `{cmd}` failed: {stderr}")]
    Subprocess { cmd: String, stderr: String },
    #[error("subprocess `{cmd}` timed out after {seconds}s")]
    Timeout { cmd: String, seconds: u64 },
    #[error("could not parse output of `{cmd}`: {message}")]
    Parse { cmd: String, message: String },
    #[error("detector returned an empty monitor list")]
    EmptyResult,
}
```

`Availability` returns an enum (not a `bool`) so `superpanels detect --debug` can explain *why* each detector was skipped. Errors are typed so the orchestrator can react differently to "tool missing" vs "tool present but failed" vs "parser broken".

Each detector is *individually unit-tested* against captured real-world output samples stored under `crates/superpanels-core/tests/fixtures/display/`. We never hit the system in tests.

## 6.2 Manual override syntax

`--monitors WxH+X+Y[@SCALE][/ROT][?WMMxHMM],...`

- `1920x1080+0+0` — layout-only override; physical mm still expected from `[[monitor]]` config.
- `2560x1440+0+0@1.5/right?597x336` — full override including 597×336 mm physical (skips config merge for this monitor).

Useful for SSH/headless/CI environments and for the test suite.

## 6.3 Live re-detection

The daemon re-detects monitors on:
- A `SIGHUP` signal.
- IPC `redetect` request (e.g. user clicked Refresh in the GUI).
- An optional, opt-in periodic re-detect every 60s for laptop dock hot-plug. Off by default; an FS-watch on `/sys/class/drm` is preferred where it works.

## 6.4 Monitor identity across reboots

`monitor.name` is unstable (a USB-C dock can re-label DP-1 to DP-2). For per-monitor persistent data — custom bezel overrides, per-monitor image assignments, **and physical-mm config** — we key on a stable identifier when one is available, falling back to `name`. Profiles and config refer to monitors by:

```rust
struct MonitorRef {
    stable_id: Option<String>,  // KDE per-output UUID; or hash of EDID manufacturer+model+serial
    name: Option<String>,       // "DP-1"; fallback when stable_id is unavailable
}
```

The layout step resolves `MonitorRef` to a live `MonitorId` (many-to-one: a `MonitorRef` matches the unique live `Monitor` whose `stable_id` matches, else whose `name` matches).

**`stable_id` sources, by detector:**

| Detector | Source of `stable_id` |
|---|---|
| `kscreen-doctor -o` | The per-output UUID printed in the `Output:` line (e.g. `f7f0f124-9e9b-4ef0-91a7-426d58091760`) — KDE generates this deterministically from EDID, so we use it directly without our own hashing. |
| `hyprctl monitors -j` | The `serial` field from JSON output. |
| `swaymsg -t get_outputs` | `make + model + serial` from the JSON output, hashed. |
| `wlr-randr --json` | `make + model + serial` if exposed; else `None`. |
| `xrandr --verbose` | EDID hex-dump under `EDID:` block, parsed for manufacturer/model/serial, hashed. |

When a detector can't supply `stable_id`, the fallback is `name`. This breaks if the user re-plugs a dock and `DP-1` now refers to a different physical monitor — we accept that ambiguity, and the GUI prompts to re-confirm assignments after detecting that the physical configuration has changed.
