#!/usr/bin/env bash
# Collect built binaries and Tauri bundles into dist/ as the release artefacts:
# a universal tarball carrying all three binaries + desktop entry + icons, the
# GUI .deb/.rpm (if `cargo tauri build` ran), and a SHA256SUMS file.
#
# Usage: packaging/assemble-release.sh <version> [arch]   # arch defaults x86_64
#
# Run after `cargo build --release -p superpanels-cli -p superpanels-daemon`
# and `cargo tauri build`.
set -euo pipefail

VERSION="${1:?usage: assemble-release.sh <version> [arch]}"
ARCH="${2:-x86_64}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/target/release"
ICONS="$ROOT/crates/superpanels-gui/icons"
DIST="$ROOT/dist"

rm -rf "$DIST"
mkdir -p "$DIST"

name="superpanels-${VERSION}-${ARCH}-linux"
work="$(mktemp -d)"
stage="$work/$name"
mkdir -p \
  "$stage/bin" \
  "$stage/share/applications" \
  "$stage/share/icons/hicolor/32x32/apps" \
  "$stage/share/icons/hicolor/128x128/apps" \
  "$stage/share/icons/hicolor/256x256/apps" \
  "$stage/share/superpanels/autostart" \
  "$stage/share/doc/superpanels"

for bin in superpanels superpanels-gui superpanels-daemon; do
  install -m755 "$TARGET/$bin" "$stage/bin/$bin"
done

install -m644 "$ROOT/packaging/superpanels-gui.desktop" \
  "$stage/share/applications/superpanels-gui.desktop"

# System autostart entry. Not prefix-relative (its home is /etc/xdg/autostart),
# so it's staged under share/ for the installers to place: install.sh copies it
# to /etc/xdg/autostart (or ~/.config/autostart for a userland prefix), and the
# pacman-repo PKGBUILD installs it to /etc/xdg/autostart.
install -m644 "$ROOT/packaging/superpanels-autostart.desktop" \
  "$stage/share/superpanels/autostart/superpanels.desktop"

# Transparent icon variants match what the GUI installs at runtime for the
# Wayland taskbar (see crates/superpanels-gui/src/desktop_entry.rs).
install -m644 "$ICONS/32x32-transparent.png"   "$stage/share/icons/hicolor/32x32/apps/superpanels-gui.png"
install -m644 "$ICONS/128x128-transparent.png" "$stage/share/icons/hicolor/128x128/apps/superpanels-gui.png"
install -m644 "$ICONS/icon-transparent.png"    "$stage/share/icons/hicolor/256x256/apps/superpanels-gui.png"

install -m644 "$ROOT/README.md" "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" \
  "$stage/share/doc/superpanels/"

tar -C "$work" -czf "$DIST/$name.tar.gz" "$name"

# GUI native bundles, if the Tauri bundler produced them.
copy_bundle() { # <find-dir> <glob> <dest-name>
  local found
  found="$(find "$1" -name "$2" 2>/dev/null | head -1 || true)"
  if [ -n "$found" ]; then
    cp "$found" "$DIST/$3"
    echo "  + $3"
  fi
}

debarch=amd64
[ "$ARCH" = aarch64 ] && debarch=arm64
copy_bundle "$TARGET/bundle/deb"      '*.deb'      "superpanels-gui_${VERSION}_${debarch}.deb"
copy_bundle "$TARGET/bundle/rpm"      '*.rpm'      "superpanels-gui-${VERSION}.${ARCH}.rpm"

# Checksums over the published assets only (glob excludes SHA256SUMS itself).
( cd "$DIST" && sha256sum superpanels-* > SHA256SUMS )

echo "assembled into dist/:"
ls -1 "$DIST"
rm -rf "$work"
