# Packaging & first release

What needs to happen before Superpanels ships a versioned binary anyone can install in one command. The codebase is well past the prototyping phase — the UI overhaul has landed, the apply pipeline is stable on KDE Wayland — but no public artefact exists yet.

**Definition of done.**

- [ ] `yay -S superpanels` and `yay -S superpanels-gui` work on Arch and CachyOS.
- [ ] `cargo install superpanels` works on a fresh stable toolchain.
- [ ] A GitHub release tagged `v0.1.0` with attached binaries + checksums.
- [ ] README has a screenshot, an install line for Arch, an install line for crates.io, and a paragraph that explains what's special about bezel correction.

Workspace version is currently `0.0.0` — bump in `Cargo.toml [workspace.package].version` as part of the release commit.

## Current state (2026-05)

- CI runs `pre-commit`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` on every PR (`.github/workflows/ci.yml`).
- No release pipeline. No artefacts produced on tag push.
- No `packaging/` directory. No PKGBUILDs. No Flatpak manifest.
- `crates.io` metadata (description, keywords, categories, repository, docs URL) is not yet filled in on any crate.
- The `webkit2gtk` DMABUF workaround (`WEBKIT_DISABLE_DMABUF_RENDERER=1`) is set in three places (`.cargo/config.toml`, the `justfile`, and `autostart::DESKTOP_BODY`); the AUR `superpanels-gui` `PKGBUILD` should set it in the installed `.desktop` file too until WebKitGTK ships a fix. See `docs/followups.md`.

## AUR PKGBUILDs

Live under `packaging/aur-superpanels/` and `packaging/aur-superpanels-gui/` (to be created).

- `superpanels` (CLI-only):
  - `makedepends=(rust)`
  - `depends=()` — no runtime libs beyond glibc.
- `superpanels-gui` (Tauri shell):
  - `makedepends=(rust nodejs npm)`
  - `depends=(webkit2gtk-4.1)`
  - Installs the `.desktop` entry from `crates/superpanels-gui/resources/superpanels.desktop` if one is bundled there; otherwise generate it during build.
- Both PKGBUILDs reviewed against AUR style guidelines.
- `.SRCINFO` regenerated and committed alongside the `PKGBUILD`.

## crates.io

- `cargo publish` order: `superpanels-core` → `superpanels-cli` → `superpanels-daemon` → `superpanels-gui`.
- Each crate's `Cargo.toml` needs:
  - `description`, `keywords` (`["wallpaper", "kde", "wayland", "multi-monitor", "linux"]`), `categories` (e.g. `command-line-utilities`, `gui`).
  - `repository = "https://github.com/<owner>/superpanels"`.
  - `documentation = "https://docs.rs/superpanels-core"` (auto for libs).
  - `license = "Apache-2.0 OR MIT"`.
  - `readme = "../../README.md"` for the binaries; `superpanels-core` gets its own short README.
- `[package.metadata.docs.rs]` on `superpanels-core` so docs.rs builds with `--all-features`.

## Flatpak (best-effort)

Manifest at `packaging/flatpak/io.github.<owner>.superpanels.yaml`. Build locally with `flatpak-builder`. Submission to Flathub is deferred — get it building first, ship to Flathub later.

## GitHub Actions release pipeline

Add `.github/workflows/release.yml` triggered on `v*` tag push:

- Build matrix: `x86_64-unknown-linux-gnu` (and `aarch64` once cross-compilation works on the runner).
- Artefacts:
  - `superpanels-x86_64-linux-cli` (statically linked where viable; otherwise dynamic glibc).
  - `superpanels-x86_64-linux-gui.tar.zst` (the Tauri bundle).
  - `SHA256SUMS` alongside.
- Optional `cosign` signing of artefacts.
- Use `softprops/action-gh-release` to attach assets to the GitHub release that the tag push created.

## Documentation refresh

- `README.md`: hero screenshot to add before release; install lines, three-line pitch, and `docs/` map are in place.
- `docs/getting-started.md` — short walkthrough; create.
- `docs/cli-reference.md` — auto-generated from clap where possible.
- `docs/troubleshooting.md` — KDE quirks, the DMABUF workaround, "monitors detect but apply does nothing" etc.
- `CHANGELOG.md` following Keep-a-Changelog — create on first release.

## Risks

- **Static linking against glibc on Arch is harder than it looks.** Plan B: dynamic glibc linkage with `glibc >= 2.17` documented; users on minimal containers get a warning.
- **AUR review feedback may require PKGBUILD changes.** Budget a day after submission.
- **WebKitGTK DMABUF crash on Wayland + NVIDIA + Plasma 6.** The env-var workaround stays until upstream fixes it.
