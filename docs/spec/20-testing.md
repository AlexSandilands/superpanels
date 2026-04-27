# 20. Testing strategy

## 20.1 Layers

- **Unit tests** in each module. Bezel math is the most heavily tested; every documented edge case in §4.5 has at least one test.
- **Snapshot tests** for parsers (kscreen-doctor, xrandr, hyprctl) using captured real-world fixtures under `crates/superpanels-core/tests/fixtures/`. `insta` for diffs.
- **Property tests** with `proptest` for bezel math: random monitor layouts, random image sizes — invariants like "sum of crop widths == canvas pixel width" and "no two crops overlap" must hold.
- **Integration tests** with a `MockBackend` that records `apply()` calls instead of touching the desktop. The whole `set` pipeline runs against this in CI.
- **Golden-image tests** for the image processing pipeline: "given input image X and config Y, the file written for monitor 0 has SHA256 Z." Catches regressions in scaling/rotation/cropping.
- **Manual smoke tests** before each release on KDE, GNOME, Sway, Hyprland — see `docs/release-checklist.md`.

## 20.2 What is NOT auto-tested

- The GUI canvas rendering pipeline visually. We assert the IPC outputs, not the rendered pixels — that's the user's job to verify.
- The compositor actually painting the wallpaper. `MockBackend` proves we sent the right paths; we trust the compositor.

## 20.3 CI

GitHub Actions matrix:
- `ubuntu-22.04` and `ubuntu-24.04` (proxy for Linux variance — Arch builds are tested via the AUR PKGBUILD on tag).
- `cargo test --workspace --all-features`.
- `cargo clippy --workspace --all-features -- -D warnings`.
- `cargo fmt --check`.
- `cargo audit` weekly.
- `cargo deny check` for licence policy.
- `cargo bench` smoke run (build only) on PR; full bench with regression check on main.
