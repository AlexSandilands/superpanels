# Phase 5 — Packaging & first release (v0.7)

**Goal.** Anyone on Arch can install Superpanels in one command. The release artefacts are pre-built and signed. The README sells the project in 30 seconds.

**Definition of done.**
- [ ] `yay -S superpanels` and `yay -S superpanels-gui` work on Arch and CachyOS.
- [ ] `cargo install superpanels` works on a fresh Rust toolchain.
- [ ] GitHub release tagged `v0.7.0` (first public, GUI-bundled release) with binary attachments.
- [ ] README has one screenshot, an install line for Arch, an install line for crates.io, and a paragraph explaining what's special.

## 5.1 PKGBUILDs (AUR)
- [ ] `superpanels` (CLI-only): `makedepends=(rust)`, `depends=()`.
- [ ] `superpanels-gui`: `makedepends=(rust nodejs npm)`, `depends=(webkit2gtk-4.1)`.
- [ ] PKGBUILDs reviewed against AUR style guidelines.
- [ ] `.SRCINFO` generated.
- [ ] Submitted to AUR.

## 5.2 Crates.io
- [ ] `cargo publish` for `superpanels-core` then `superpanels`.
- [ ] Crate metadata complete: description, keywords, categories, repository, documentation URL.
- [ ] `[package.metadata.docs.rs]` configured for `cargo doc` builds.

## 5.3 Flatpak (best-effort)
- [ ] Flatpak manifest under `packaging/flatpak/io.github.alex.superpanels.yaml`.
- [ ] Builds locally; submission to Flathub deferred to v0.8.

## 5.4 GitHub Actions release pipeline
- [ ] On tag push: build CLI binary (statically linked where viable), build GUI binary (Tauri bundler), attach to release.
- [ ] Build matrix: x86_64, aarch64.
- [ ] SHA-256 checksums alongside binaries.
- [ ] Optional `cosign` signing.

## 5.5 Documentation
- [ ] `README.md`: hero screenshot, install lines, three-line pitch, link to SPEC and the docs site.
- [ ] `docs/getting-started.md`.
- [ ] `docs/cli-reference.md` (auto-generated from clap where possible).
- [ ] `docs/configuration.md` (the TOML schema).
- [ ] `docs/troubleshooting.md`.
- [ ] `CHANGELOG.md` following Keep-a-Changelog.

**Risks for this phase.**
- Static linking against glibc on Arch is harder than it looks. Plan B: dynamic glibc linkage with `glibc >= 2.17` documented; users on minimal containers get a warning.
- AUR review feedback may require PKGBUILD changes; budget a day.
