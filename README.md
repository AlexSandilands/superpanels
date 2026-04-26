# Superpanels

> Linux wallpaper manager focused on physical-bezel-aware multi-monitor spanning and folder-driven slideshows.

**Status: Phase 1 (CLI MVP, KDE Wayland).** The Rust core and `superpanels` CLI are working; multi-backend, slideshow, daemon, and GUI follow in [PLAN.md](./PLAN.md) Phases 2–4.

Single binary. Rust core, Tauri v2 + Svelte 5 GUI. Primary target: Arch / CachyOS on KDE Wayland.

## Install

Not yet on crates.io or the AUR. Build from source:

```sh
git clone https://github.com/alex/superpanels.git
cd superpanels
cargo build --release -p superpanels-cli
# binary at target/release/superpanels
```

Once Phase 5 ships, `cargo install superpanels` and `yay -S superpanels` will be the supported install paths.

## Quick start

```sh
# Tell Superpanels how big each monitor is in mm (one-off, see SPEC.md §6).
superpanels monitor configure DP-1 --diagonal 27in --aspect 16:9

# Span a panorama across every monitor, bezel-corrected.
superpanels set panorama.jpg --bezel-h 8

# Or preview the crops without touching the wallpaper.
superpanels set panorama.jpg --bezel-h 8 --dry-run
```

See [SPEC.md](./SPEC.md) for the full design and [`superpanels --help`](./crates/superpanels-cli/src/main.rs) for the current command surface.

## What it does

- Spans a panorama across multiple monitors with the **physical bezel gap** accounted for, so the image stays continuous as the eye sees it across the desk.
- Drives a folder-of-wallpapers slideshow on a schedule, with smart aspect-ratio filtering and recent-history suppression.
- Ships a CLI for headless / scripted use and an optional GUI with a live, scaled, bezel-accurate monitor preview canvas.
- Supports KDE, GNOME, Sway, Hyprland, X11/feh, and a custom-command escape hatch.

## Documentation

| Read this when | Doc |
|---|---|
| You want to know what's being built | [SPEC.md](./SPEC.md) |
| You want to know what's next | [PLAN.md](./PLAN.md) |
| You want to contribute | [CONTRIBUTING.md](./CONTRIBUTING.md) |
| You're writing Rust here | [docs/style-rust.md](./docs/style-rust.md) |
| You're writing TypeScript / Svelte here | [docs/style-frontend.md](./docs/style-frontend.md) |
| You're adding modules / dependencies | [docs/architecture.md](./docs/architecture.md) |
| You're writing tests | [docs/testing.md](./docs/testing.md) |

## Licence

Dual-licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT License](./LICENSE-MIT) at your option. Contributions are accepted under the same terms.
