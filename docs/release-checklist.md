# Release checklist

> Manual smoke tests + housekeeping for tagging a release. Filled in as phases ship; many items are placeholders pre-Phase 5.

This is not a runbook for any one release — it's the canonical *list of things to walk through* before tagging. Skip items that aren't yet relevant for the current version (noted inline).

---

## Pre-release housekeeping

- [ ] `git status` clean on the release branch.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] `cargo deny check` passes.
- [ ] `pre-commit run --all-files --hook-stage pre-push` passes.
- [ ] `CHANGELOG.md` updated with the new version's notes (Keep-a-Changelog format).
- [ ] Version bumped in workspace `Cargo.toml` (`workspace.package.version`); per-crate inheritance verified.
- [ ] If schema changed: migration code present + tested + version bumped in `library.db`, `state.json`, `config.toml`.

---

## Manual smoke tests (per backend)

Run on a real desktop session for each supported backend before tagging. Capture a screenshot of each working setup; attach to the release notes.

### KDE (primary — required every release)

- [ ] `superpanels detect` lists all monitors with correct physical mm.
- [ ] `superpanels set <pano>` spans a panorama across all monitors with bezel correction.
- [ ] Wallpaper survives logout / login.
- [ ] GUI canvas matches the physical layout (proportional sizes; portrait monitors rotated).
- [ ] Drag-to-offset in the GUI updates the live preview at ≥ 60 fps.
- [ ] Slideshow advances on schedule and persists state across daemon restart.

### Sway / Hyprland / GNOME / X11+feh (Phase 2+)

- [ ] `superpanels set` produces a bezel-correct wallpaper on each.
- [ ] Profile switch from the tray works on each.
- [ ] Backend auto-detection picks the right backend without config.

### Custom backend (Phase 2+)

- [ ] A custom-command profile applies an image successfully.
- [ ] Custom-command field's safety callout is visible in the GUI settings.

---

## Packaging (Phase 5+)

- [ ] AUR PKGBUILDs (`superpanels`, `superpanels-gui`) build cleanly with `makepkg -si`.
- [ ] `.SRCINFO` regenerated and committed to the AUR repos.
- [ ] `cargo install superpanels` works on a fresh Rust toolchain (test in a clean container).
- [ ] GitHub release tag created; binaries attached:
  - [ ] `superpanels-x86_64-linux-cli`
  - [ ] `superpanels-x86_64-linux-gui.tar.zst`
  - [ ] `superpanels-aarch64-linux-cli`
  - [ ] SHA-256 checksums alongside each.
- [ ] Flatpak builds locally (Phase 5+ best-effort).

---

## Post-release

- [ ] Release notes posted to GitHub Releases.
- [ ] AUR `bin` package bumped (if present).
- [ ] Move tick boxes in the relevant `docs/plan/phase-*.md` to "completed" for the phase that shipped.
- [ ] Open issues for anything that *almost* shipped but slipped.
- [ ] If 1.0+: announcement (changelog summary on r/unixporn / r/archlinux / HN).
