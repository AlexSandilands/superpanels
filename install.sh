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

say()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  sed -n '2,18p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'
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
    "$PREFIX/share/applications/superpanels-gui.desktop" \
    "$PREFIX/share/icons/hicolor/32x32/apps/superpanels-gui.png" \
    "$PREFIX/share/icons/hicolor/128x128/apps/superpanels-gui.png" \
    "$PREFIX/share/icons/hicolor/256x256/apps/superpanels-gui.png"
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

  if fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" 2>/dev/null && have sha256sum; then
    say "verifying checksum"
    ( cd "$tmp" && grep -- "${name}.tar.gz" SHA256SUMS | sha256sum -c - ) \
      || err "checksum verification failed"
  else
    warn "skipping checksum verification (no SHA256SUMS or sha256sum)"
  fi

  tar -C "$tmp" -xzf "$tmp/$name.tar.gz"

  say "installing to $PREFIX (may prompt for sudo)"
  run_priv mkdir -p "$PREFIX/bin" "$PREFIX/share"
  run_priv cp -a "$tmp/$name/bin/." "$PREFIX/bin/"
  run_priv cp -a "$tmp/$name/share/." "$PREFIX/share/"
  refresh_caches

  say "installed Superpanels $VERSION — run 'superpanels-gui' or 'superpanels --help'"
  webkit_note
}

case "$ACTION" in
  install) do_install;;
  uninstall) do_uninstall;;
esac
