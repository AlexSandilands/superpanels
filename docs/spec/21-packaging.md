# 21. Packaging & distribution

## 21.1 Arch / CachyOS (primary target)

**CLI-only PKGBUILD** (`superpanels`):
```
makedepends=(rust)
depends=()                           # zero runtime deps; statically linked where viable
```

**GUI PKGBUILD** (`superpanels-gui`):
```
makedepends=(rust nodejs npm)
depends=(webkit2gtk-4.1)             # Tauri's only Linux runtime dep
```

WebKitGTK is already present on KDE/GNOME systems; on minimal installs it's the only addition. Both packages are submitted to the AUR. We aim for `extra/` inclusion once stable.

## 21.2 Crates.io

The CLI is published as `superpanels`. `cargo install superpanels` works without additional setup.

## 21.3 Flatpak

A Flatpak manifest under `packaging/flatpak/` for non-Arch distros. Not the primary distribution channel; provided for breadth.

## 21.4 GitHub Releases

Pre-built binaries attached to each release tag:
- `superpanels-x86_64-linux-cli` — statically linked where viable; glibc ≥ 2.17.
- `superpanels-x86_64-linux-gui.tar.zst` — includes Tauri app bundle.
- `superpanels-aarch64-linux-cli` — for ARM SBC users.

CI via GitHub Actions: `cargo build --release` + Tauri bundler on each tag push, attached automatically.

## 21.5 Versioning

SemVer. `0.x.y` until the config schema is frozen; `1.0.0` when the schema is stable enough that we'll write migration code rather than break it. Pre-1.0 minor bumps may include breaking changes; the changelog is explicit.
