# Display detection

How Superpanels reads the connected monitor set. The runtime contract — what each detector must guarantee, how monitor identity persists across reboots, and how the manual-override syntax works.

Code: [`crates/superpanels-core/src/display.rs`](../../crates/superpanels-core/src/display.rs) and `display/*.rs`.

## Detection produces layout only

Detection returns a `Monitor` struct with positions, resolutions, scale, rotation, name, and (optionally) a stable identifier. **Physical size in mm is `None` at detection time** — no Linux compositor CLI reliably exposes it, and EDID-from-sysfs is sometimes wrong. The user provides physical mm via `[[monitor]]` blocks in config; merging happens after detection.

Asking the user for monitor mm is one extra config step in exchange for a uniform detection surface and predictable correctness. The GUI's first-run flow writes the blocks for the user (via diagonal + aspect-ratio entry, or direct mm).

## Detector ladder

Detection is attempted in priority order and stops at the first detector that succeeds and returns a non-empty layout.

| Priority | Detector | Detection condition | `stable_id` source |
|---|---|---|---|
| 1 | `kscreen-doctor -o` | KDE session detected | Per-output UUID from the `Output:` line (KDE generates it deterministically from EDID) |
| 2 | `hyprctl monitors -j` | `$HYPRLAND_INSTANCE_SIGNATURE` set | `serial` field |
| 3 | `wlr-randr --json` | wlroots compositor | `make + model + serial` if exposed, else `None` |
| 4 | `xrandr --verbose` | `$DISPLAY` set, no Wayland | EDID hex-dump parsed for make/model/serial, hashed |
| — | Manual override | `--monitors` CLI flag or config | n/a; wins unconditionally |

Each detector runs as a subprocess with a 5-second timeout. KDE's detector runs with `NO_COLOR=1` so the parser doesn't have to strip ANSI.

If all fail: `Could not detect monitor layout. Try --monitors WxH+X+Y,... to specify manually, or run 'superpanels detect --debug' to see what was attempted.`

## Detector trait

```rust
pub trait DisplayDetector {
    fn name(&self) -> &str;
    /// Cheap check — env vars and PATH only, no subprocess spawn.
    fn availability(&self) -> Availability;
    fn detect(&self) -> Result<Vec<Monitor>, DetectError>;
}

pub enum Availability {
    Available,
    ToolMissing { tool: &'static str },
    WrongEnvironment { reason: &'static str },
    Disabled,
}

pub enum DetectError {
    Subprocess { cmd: String, stderr: String },
    Timeout { cmd: String, seconds: u64 },
    Parse { cmd: String, message: String },
    /// Soft failure — orchestrator falls through to the next detector.
    EmptyResult,
}
```

`Availability` is an enum (not `bool`) so `superpanels detect --debug` can explain *why* each detector was skipped. Errors are typed so the orchestrator can react differently to "tool missing" vs "tool present but parser broken".

Each detector is unit-tested against captured real-world output samples under `crates/superpanels-core/tests/fixtures/display/`. We never hit the system in tests.

## Monitor identity across reboots

`Monitor.name` is unstable — a USB-C dock can re-label `DP-1` to `DP-2`. Persistent data (profile placements, physical-mm config) keys on `MonitorRef`:

```rust
pub struct MonitorRef {
    pub stable_id: String,  // empty string when the detector couldn't supply one
    pub name: String,       // e.g. "DP-1"; fallback when stable_id is unavailable
}
```

The merge step (detection → live `Monitor` list with `physical_size_mm` filled in) matches `[[monitor]]` config entries by `stable_id` first, then `name`. Same lookup applies to profile `monitor_state` keys.

When `stable_id` is unavailable, the `name`-only fallback can break across dock re-plugs. The GUI prompts to re-confirm assignments after detecting that the physical configuration has changed.

## Manual override syntax

`--monitors WxH+X+Y[@SCALE][/ROT][?WMMxHMM][,...]`

- `1920x1080+0+0` — layout-only override; physical mm still expected from `[[monitor]]` config.
- `2560x1440+0+0@1.5/right?597x336` — full override including 597×336 mm physical (skips config merge for that monitor).

Useful for SSH/headless/CI and the test suite.

## Live re-detection

The daemon re-detects monitors on:

- `SIGHUP`.
- IPC `redetect` request (e.g. user clicked Refresh in the GUI).

OS-driven monitor rotation is observed at the daemon's watch layer; see `crates/superpanels-daemon/src/`.
