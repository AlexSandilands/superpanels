# Packaging & first release

What needs to happen before Superpanels ships a versioned binary anyone can install in one command. The codebase is well past the prototyping phase — the UI overhaul has landed, the apply pipeline is stable on KDE Wayland — but no public artefact exists yet.

**Definition of done.**

- [ ] `yay -S superpanels` works on Arch and CachyOS (one package, all three binaries).
- [ ] `curl -fsSL …/install.sh | sh` installs CLI + daemon + GUI on any glibc distro.
- [ ] `cargo install superpanels` works on a fresh stable toolchain (CLI-only escape hatch).
- [ ] A GitHub release tagged `v0.1.0` with attached binaries + checksums.
- [ ] README has a screenshot, an install line for Arch, an install line for crates.io, and a paragraph that explains what's special about bezel correction.

Workspace version is currently `0.0.0` — bump in `Cargo.toml [workspace.package].version` as part of the release commit.

## Current state (2026-06)

- CI runs `pre-commit`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` on every PR (`.github/workflows/ci.yml`).
- **Release pipeline lands in `.github/workflows/release.yml`** — tag-triggered (`v*`), builds on `ubuntu-22.04` for glibc breadth, stamps the tag version into the manifests (`packaging/set-version.sh`), and publishes the artefacts assembled by `packaging/assemble-release.sh`.
- **`install.sh`** at the repo root is the distro-agnostic one-liner; it installs all three binaries + desktop entry + icons from the universal tarball.
- **`packaging/`** holds the AUR `PKGBUILD`, the canonical `superpanels-gui.desktop`, and the release scripts. No Flatpak manifest yet.
- `crates.io` metadata (description, keywords, categories, repository, docs URL) is not yet filled in on any crate.
- The `webkit2gtk` DMABUF workaround (`WEBKIT_DISABLE_DMABUF_RENDERER=1`) is now
  duplicated, in lockstep, across six places — keep them in sync until WebKitGTK
  ships a fix ([#8](https://github.com/AlexSandilands/superpanels/issues/8)):
  - dev launches: `.cargo/config.toml`, the `justfile`;
  - runtime-written entries: `autostart.rs` (`DESKTOP_BODY`), `desktop_entry.rs`;
  - packaged launchers: `packaging/superpanels-gui.desktop` (tarball + AUR) and
    `crates/superpanels-gui/desktop-entry.hbs` (the Tauri `.deb`/`.rpm`/`.AppImage`).

  The same hazard applies to the `[Desktop Entry]` body itself and the
  icon-size → hicolor mapping (`assemble-release.sh`, the `PKGBUILD`, `install.sh`).
  Collapsing these onto a single generated source is a worthwhile follow-up.

## AUR package

**Single package** under `packaging/aur-superpanels/` — `superpanels`, shipping all
three binaries. The earlier CLI/GUI split was dropped (see GitHub #22): for a
desktop GUI the headless audience is niche, so one package is the better UX. The
CLI-only path stays as `cargo install superpanels` (crates.io).

- `makedepends=(rust nodejs npm)` — builds the frontend (`npm run build`) then the
  whole workspace; no Tauri bundler needed since we install files by hand.
- `depends=(webkit2gtk-4.1 gtk3 libayatana-appindicator)`.
- Installs `packaging/superpanels-gui.desktop`, whose `Exec=` bakes in
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` (#8), plus the transparent hicolor icons.
- Reviewed against AUR style guidelines.
- `.SRCINFO` regenerated (`makepkg --printsrcinfo`) and `sha256sums` filled in
  (`updpkgsums`) at each release; the committed copies carry placeholders.

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

`.github/workflows/release.yml`, triggered on `v*` tag push. `x86_64` only for now
(add `aarch64` once cross-compilation works on the runner). Artefacts attached to
the release via `softprops/action-gh-release`:

- `superpanels-<ver>-x86_64-linux.tar.gz` — the **universal bundle**: all three
  binaries + `superpanels-gui.desktop` + hicolor icons + licences. This is what
  `install.sh` pulls, so it's the only artefact that carries the CLI and daemon.
- `superpanels-gui_<ver>_amd64.deb`, `superpanels-gui-<ver>.x86_64.rpm`,
  `superpanels-gui_<ver>_x86_64.AppImage` — the Tauri GUI bundles, for users who
  prefer their native package manager (these are GUI-only by Tauri's design). Their
  menu entry comes from `crates/superpanels-gui/desktop-entry.hbs` so it carries the
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround (#8) like every other install path.
- `SHA256SUMS` over all of the above.

The tag drives versioning: `set-version.sh` stamps the tag into the manifests +
`tauri.conf.json` before building, so `--version` and bundle filenames are correct
even if the release commit forgot to bump. `cosign` signing is still a possible
future add.

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
