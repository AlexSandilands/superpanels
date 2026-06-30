<div align="center">

# Superpanels

**A Linux wallpaper manager built for multi-monitor desks.**
It treats your monitors as what they physically are — panels at real positions in real space, with real gaps between them — and crops and scales a single image across them so the picture stays continuous to your eye.

[![CI](https://github.com/AlexSandilands/superpanels/actions/workflows/ci.yml/badge.svg)](https://github.com/AlexSandilands/superpanels/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licence)
![Platform: Linux](https://img.shields.io/badge/platform-Linux-333?logo=linux&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white)
![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white)

<br />

<img src="docs/assets/span.png" alt="One image spanned across three monitors of mixed orientation, with the per-monitor inspector showing physical size in millimetres and an 18 mm bezel gap" width="100%" />

<br /><br />

<table>
<tr>
<td width="50%"><img src="docs/assets/slideshow.png" alt="A folder slideshow spanning three landscape monitors, with the library and playback controls docked along the bottom" /></td>
<td width="50%"><img src="docs/assets/arrange.png" alt="Three monitors arranged at different heights on the canvas, an image flowing across the staggered layout" /></td>
</tr>
</table>

</div>

## What it is

Most wallpaper tools think in pixels: they stretch one image edge-to-edge across a virtual framebuffer, ignoring that there's a centimetre or two of plastic between your panels. The result is a panorama with a seam — the horizon jumps where the bezels are.

Superpanels thinks in **millimetres**. You tell it how big each monitor is and how much gap sits between them, and it maps your image onto the physical desktop plane *including* those gaps — so the slice of the picture that falls "behind" a bezel is simply not shown, the way it would be if you were looking through three windows onto one scene.

You arrange everything on a live canvas that mirrors your real desk, then apply. It works for a single still image, for free-positioned multi-image layouts, and for folder-driven slideshows on a schedule. There's a GUI for arranging by hand and a CLI for headless or scripted use.

Primary target is Arch / CachyOS on KDE Wayland — and **KDE is the only desktop tested so far**. Backends for GNOME, Sway, Hyprland, and X11 are implemented but not yet verified on real setups; treat them as experimental for now.

## Install

The install script pulls the latest release, drops the CLI, daemon, and GUI into place, and registers the app icon — on any glibc Linux distro.

```sh
curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh
```

Uninstall the same way. This stops the daemon and tray, then removes the binaries, the app-menu and autostart entries, and the icons. Your config under `~/.config/superpanels` is left untouched:

```sh
curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh -s -- --uninstall
```

To also delete your settings, slideshow state, and data, use `--purge` instead of `--uninstall`. If you installed with a custom `--prefix`, pass the same one when uninstalling.

**Options** (after `| sh -s --`): `--version <v>` to pin a release, `--prefix <dir>` to install somewhere else (e.g. `~/.local` for no sudo). The GUI needs two runtime libraries — **WebKitGTK 4.1** (renders the webview) and a **system-tray library** (provides the tray icon; the GUI won't start without it). The installer scans for both and prints the exact command for your distro if either is missing:

| Distro | WebKitGTK | System tray |
|---|---|---|
| Arch / CachyOS | `webkit2gtk-4.1` | `libayatana-appindicator` |
| Fedora / RHEL | `webkit2gtk4.1` | `libayatana-appindicator-gtk3` |
| Debian / Ubuntu | `libwebkit2gtk-4.1-0` | `libayatana-appindicator3-1` |

Prefer to build it yourself? See [Building from source](#building-from-source).

## Using it

Everything below is done on the GUI canvas — a scaled model of your real desk. (Scriptable equivalents live in the [CLI](#cli) section.)

### Set up your monitors

Spanning only looks right once Superpanels knows how big your monitors physically are — something no compositor reports. On first run it walks you through it: for each monitor you give the panel's diagonal and aspect ratio (or the millimetres directly), and it remembers them. Click any monitor on the canvas to reopen its inspector — mode and refresh rate, rotation, physical size in mm and inches, and a live preview of exactly what that screen will show.

Monitors are tracked by a stable hardware id, not names like `DP-1` that shuffle across reboots and dock changes, so your setup survives replugging.

### Arrange the wallpaper

Drop in an image and it covers every monitor at once. From there it's direct manipulation:

- **Drag** the image to pan, **scroll** to zoom, rotate it — the per-monitor crop updates live.
- **Drag a monitor** to match your real arrangement: side by side, stacked, or at staggered heights.
- Set the **monitor gap** (the bezel-plus-air-gap between panels, in millimetres) and watch the seam close up.
- Hold **Alt** for fine placement without snapping.

When it looks right, **Apply** (or <kbd>Enter</kbd>) pushes it to your desktop; **Save** stores it on the current profile.

<p align="center">
  <img src="docs/assets/monitor-setup.png" alt="The Settings → Monitors panel listing three detected displays with their resolution, refresh rate, and physical size in millimetres, each with an Edit size button" width="88%" /><br />
  <sub><em>Settings → Monitors: every detected display with its physical size, editable per panel.</em></sub>
</p>

### Profiles

A profile is a whole saved setup — every monitor placement, the gaps, and the image arrangement — not a one-off. Switch between them from the dropdown at the top left or the tray menu; each carries its own accent colour so you can tell at a glance which you're in. There are two kinds:

- **Standard** — one or more images placed freely on the canvas. A single spanned panorama is the simple case; stack more and each screen shows whatever overlaps it.
- **Slideshow** — a rotating set (below).

Each profile remembers the monitors it was built for. Plug in a different setup and it greys out rather than applying something wrong, then offers to re-fit itself.

<p align="center">
  <img src="docs/assets/profiles.png" alt="A slideshow profile named Triple-Slideshow spanning a Japanese-countryside set across three monitors, the interface tinted with the profile's own blue accent" width="88%" /><br />
  <sub><em>Each profile carries its own accent colour and remembers its mode — the chip at the top left shows the active one.</em></sub>
</p>

### Slideshows and the library

The library is your wallpaper collection — indexed with thumbnails from the folders (*roots*) you point it at, and kept current in the background. The dock along the bottom of the canvas is the quick view; open the full library to search by name or tag, filter by aspect or resolution, sort, and star favourites. Drag any wallpaper straight onto a monitor, or add it to a slideshow.

<p align="center">
  <img src="docs/assets/library.png" alt="The full library view: a thumbnail grid with aspect and resolution on each tile, plus a sidebar of roots, aspect and resolution filters, folder tags, sort options, and a search bar with a favourites toggle" width="88%" /><br />
  <sub><em>The full library: search, filter by aspect or resolution, tag, sort, and favourite — then drag onto a monitor or into a slideshow.</em></sub>
</p>

A slideshow rotates through folders and hand-picked images together, and new files dropped into a watched folder join automatically. You choose how often it rotates, whether it shuffles or plays in order, and how many recent images to skip before repeating. Crop any wallpaper by hand and it keeps that crop each time it comes back around. Next / previous / pause controls sit right on the dock.

<p align="center">
  <img src="docs/assets/slideshow-settings.png" alt="The slideshow settings popover: rotation interval with preset buttons, order set to Shuffle, resume-on-start, avoid-repeating-last count, and skip-missing-files toggle" width="88%" /><br />
  <sub><em>Slideshow timing: how often it rotates, shuffle vs. in order, resume behaviour, and how many recent images to skip.</em></sub>
</p>

### Scheduling

Have profiles switch themselves on a clock — a bright look in the morning, something warmer in the evening, a work setup on weekday mornings. Add rules in the UI (a daily time, or a cron expression for anything more specific); a master switch pauses them all at once, mirrored in the tray.

<!-- screenshot slot: scheduling panel -->

### Desktop support

Superpanels detects and drives your compositor's wallpaper mechanism automatically — KDE, GNOME, Sway, Hyprland, and X11 (via feh) are built in. For anything else, point it at a custom command. See [`docs/reference/backends.md`](./docs/reference/backends.md).

## Configuration

You never have to touch a config file — the GUI writes everything. But it's all plain TOML at `$XDG_CONFIG_HOME/superpanels/config.toml` if you'd rather hand-edit, validated on every load and save so a bad edit can't brick startup or lose your current wallpaper. The full schema — profiles, monitors, slideshows, schedules — is in [`docs/reference/configuration.md`](./docs/reference/configuration.md).

## CLI

Most people will live in the GUI, but everything is scriptable. The CLI runs without the daemon (in-process fallback), so one-shot applies work anywhere; it talks to a running daemon when there is one (for the slideshow timer, file watcher, and tray).

| Command | What it does |
|---|---|
| `superpanels set <IMAGE…>` | Set wallpaper from one or more images; auto-fits to the monitor union. `--dry-run` to preview crops, `--save-as <NAME>` to capture a profile, `--monitor NAME=PATH` to pin per-monitor, `--backend <NAME>` to force one. |
| `superpanels detect` | Print the detected monitor layout. `--json` for machine output, `--debug` to see which detectors were tried. |
| `superpanels config` | Print the resolved configuration as TOML. |
| `superpanels monitor configure <ID>` | Set a monitor's physical size (`--diagonal` + `--aspect`, or `--mm`). |
| `superpanels profile <list\|apply\|show\|delete\|rename\|duplicate\|export\|import>` | Manage saved profiles. |
| `superpanels schedule <list\|add\|remove\|enable\|disable\|pause\|resume>` | Manage clock-driven profile switches. |
| `superpanels next` / `prev` / `pause` / `resume` | Drive a running slideshow. |

Global flags: `-v` / `-vv` (debug / trace logging), `--quiet`, `--config <PATH>` for an alternate config file, `--no-daemon` to force in-process. Full help on any command with `--help`.

```sh
# A typical first run, headless.
superpanels monitor configure DP-1 --diagonal 27in --aspect 16:9
superpanels monitor configure DP-2 --diagonal 27in --aspect 16:9
superpanels set panorama.jpg --dry-run     # check the crops
superpanels set panorama.jpg --save-as desk
```

## Building from source

Rust workspace (`crates/superpanels-{core,cli,daemon,gui}`) with a Svelte 5 frontend in `ui/`. Stable toolchain; Node for the GUI frontend.

```sh
git clone https://github.com/AlexSandilands/superpanels.git
cd superpanels

# CLI + daemon only:
cargo build --release -p superpanels-cli -p superpanels-daemon

# GUI (build the frontend first; tauri-build embeds it):
npm --prefix ui ci && npm --prefix ui run build
cargo build --release -p superpanels-gui
```

Tauri OS prerequisites, the dev/HMR flow, and the WebKitGTK Wayland note are in [CONTRIBUTING.md](./CONTRIBUTING.md). Packaging and release mechanics live in [`packaging/README.md`](./packaging/README.md).

## Documentation

| Read this when | Doc |
|---|---|
| You want the layout / monitor-gap math | [`docs/reference/layout-math.md`](./docs/reference/layout-math.md) |
| You're touching display detection | [`docs/reference/displays.md`](./docs/reference/displays.md) |
| You're touching a backend | [`docs/reference/backends.md`](./docs/reference/backends.md) |
| You want the config schema | [`docs/reference/configuration.md`](./docs/reference/configuration.md) |
| You're working on the security / IPC surface | [`docs/reference/security.md`](./docs/reference/security.md) |
| You're contributing code | [CONTRIBUTING.md](./CONTRIBUTING.md) and [`docs/contributing/`](./docs/contributing/) |
| You're tagging a release | [`packaging/README.md`](./packaging/README.md) |

## Licence

Dual-licensed under either [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT License](./LICENSE-MIT) at your option. Contributions are accepted under the same terms.
