#!/bin/sh
# Superpanels universal installer.
#
#   curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh
#
# Installs all three binaries (superpanels, superpanels-gui, superpanels-daemon)
# plus the desktop entry and icons from a GitHub release. Works on any glibc
# Linux distro; native .deb/.rpm/.AppImage are also published for users who
# prefer their package manager.
#
# Options (pass after `| sh -s --`, or when running the file directly):
#   --version <v>    install a specific version (default: latest release)
#   --prefix <dir>   install root (default: /usr/local; use ~/.local for no sudo)
#   --uninstall      remove a previous install
#   -h, --help       show this help
set -eu

REPO="AlexSandilands/superpanels"
PREFIX="${PREFIX:-/usr/local}"
VERSION=""
ACTION="install"
ICON_SIZES="32x32 128x128 256x256"

say()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  cat <<'EOF'
Superpanels universal installer.

  curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh

Installs all three binaries (superpanels, superpanels-gui, superpanels-daemon)
plus the desktop entry and icons from a GitHub release.

Options (after `| sh -s --`, or when running the file directly):
  --version <v>    install a specific version (default: latest release)
  --prefix <dir>   install root (default: /usr/local; use ~/.local for no sudo)
  --uninstall      remove a previous install
  -h, --help       show this help
EOF
  exit "${1:-0}"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:?--version needs an argument}"; shift 2;;
    --version=*) VERSION="${1#*=}"; shift;;
    --prefix) PREFIX="${2:?--prefix needs an argument}"; shift 2;;
    --prefix=*) PREFIX="${1#*=}"; shift;;
    --uninstall) ACTION="uninstall"; shift;;
    -h|--help) usage 0;;
    *) err "unknown option: $1 (see --help)";;
  esac
done

[ "$(uname -s)" = "Linux" ] || err "Superpanels only runs on Linux"

case "$(uname -m)" in
  x86_64|amd64) ARCH="x86_64";;
  *) err "unsupported architecture: $(uname -m) (only x86_64 is published today)";;
esac

# Writing under a system prefix needs root; a $HOME prefix does not.
run_priv() {
  if [ -w "$PREFIX" ] || [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif have sudo; then
    sudo "$@"
  else
    err "need root or sudo to write to $PREFIX (or pass --prefix \"\$HOME/.local\")"
  fi
}

fetch() { # <url> <outfile>
  if have curl; then curl -fsSL "$1" -o "$2"
  elif have wget; then wget -qO "$2" "$1"
  else err "need curl or wget"; fi
}

fetch_stdout() { # <url>
  if have curl; then curl -fsSL "$1"
  elif have wget; then wget -qO- "$1"
  else err "need curl or wget"; fi
}

refresh_caches() {
  if have update-desktop-database; then
    run_priv update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
  fi
  if have gtk-update-icon-cache; then
    run_priv gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
  fi
}

do_uninstall() {
  say "removing Superpanels from $PREFIX"
  run_priv rm -f \
    "$PREFIX/bin/superpanels" \
    "$PREFIX/bin/superpanels-gui" \
    "$PREFIX/bin/superpanels-daemon" \
    "$PREFIX/share/applications/superpanels-gui.desktop"
  for s in $ICON_SIZES; do
    run_priv rm -f "$PREFIX/share/icons/hicolor/$s/apps/superpanels-gui.png"
  done
  run_priv rm -rf "$PREFIX/share/doc/superpanels"
  refresh_caches
  say "done. (your config under ~/.config/superpanels was left untouched)"
}

webkit_note() {
  pkg="webkit2gtk-4.1"
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    id="$(. /etc/os-release 2>/dev/null && echo "${ID:-} ${ID_LIKE:-}")"
    case "$id" in
      *fedora*|*rhel*) pkg="webkit2gtk4.1 (dnf)";;
      *debian*|*ubuntu*) pkg="libwebkit2gtk-4.1-0 (apt)";;
      *arch*) pkg="webkit2gtk-4.1 (pacman) — or just: yay -S superpanels";;
    esac
  fi
  echo
  say "the GUI needs WebKitGTK 4.1 at runtime: install '$pkg' if it isn't already."
}

do_install() {
  if [ -z "$VERSION" ]; then
    say "resolving latest release"
    VERSION="$(fetch_stdout "https://api.github.com/repos/$REPO/releases/latest" \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
    [ -n "$VERSION" ] || err "could not determine the latest release"
  fi
  VERSION="${VERSION#v}"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  name="superpanels-${VERSION}-${ARCH}-linux"
  base="https://github.com/$REPO/releases/download/v${VERSION}"

  say "downloading $name.tar.gz"
  fetch "$base/$name.tar.gz" "$tmp/$name.tar.gz"

  # Every release ships SHA256SUMS; a missing one means a broken/tampered
  # download, so refuse rather than install unverified.
  say "verifying checksum"
  fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" \
    || err "could not download SHA256SUMS — refusing to install unverified"
  if have sha256sum; then
    ( cd "$tmp" && grep -- "${name}.tar.gz" SHA256SUMS | sha256sum -c - ) \
      || err "checksum verification failed"
  elif have shasum; then
    ( cd "$tmp" && grep -- "${name}.tar.gz" SHA256SUMS | shasum -a 256 -c - ) \
      || err "checksum verification failed"
  else
    warn "no sha256sum/shasum available — cannot verify (proceeding)"
  fi

  tar -C "$tmp" -xzf "$tmp/$name.tar.gz"
  src="$tmp/$name"

  # install(1) (not cp -a) so files land root-owned with explicit modes; cp -a
  # under sudo would preserve the unprivileged extractor's uid, leaving
  # root-executed binaries user-writable.
  say "installing to $PREFIX (may prompt for sudo)"
  for b in superpanels superpanels-gui superpanels-daemon; do
    run_priv install -Dm755 "$src/bin/$b" "$PREFIX/bin/$b"
  done
  run_priv install -Dm644 "$src/share/applications/superpanels-gui.desktop" \
    "$PREFIX/share/applications/superpanels-gui.desktop"
  for s in $ICON_SIZES; do
    run_priv install -Dm644 "$src/share/icons/hicolor/$s/apps/superpanels-gui.png" \
      "$PREFIX/share/icons/hicolor/$s/apps/superpanels-gui.png"
  done
  for d in README.md LICENSE-MIT LICENSE-APACHE; do
    run_priv install -Dm644 "$src/share/doc/superpanels/$d" \
      "$PREFIX/share/doc/superpanels/$d"
  done
  refresh_caches

  say "installed Superpanels $VERSION — run 'superpanels-gui' or 'superpanels --help'"
  webkit_note
}

case "$ACTION" in
  install) do_install;;
  uninstall) do_uninstall;;
esac
