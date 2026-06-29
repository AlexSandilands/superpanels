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
#   --uninstall      stop the daemon/tray and remove the install (keeps config)
#   --purge          like --uninstall, and also delete config/data/state
#   -h, --help       show this help
set -eu

REPO="AlexSandilands/superpanels"
PREFIX="${PREFIX:-/usr/local}"
VERSION=""
ACTION="install"
PURGE=0
ICON_SIZES="32x32 128x128 256x256"

# Default XDG dirs the GUI writes into at runtime (autostart entry, app-menu
# entry + icons, and config/data/state). install.sh never creates these, but
# uninstall must clean them or the launcher keeps listing a removed app.
xdg_config_home() { printf '%s' "${XDG_CONFIG_HOME:-$HOME/.config}"; }
xdg_data_home()   { printf '%s' "${XDG_DATA_HOME:-$HOME/.local/share}"; }
xdg_state_home()  { printf '%s' "${XDG_STATE_HOME:-$HOME/.local/state}"; }
xdg_cache_home()  { printf '%s' "${XDG_CACHE_HOME:-$HOME/.cache}"; }

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
  --uninstall      stop the daemon/tray and remove the install (keeps config)
  --purge          like --uninstall, and also delete config/data/state
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
    --purge) ACTION="uninstall"; PURGE=1; shift;;
    -h|--help) usage 0;;
    *) err "unknown option: $1 (see --help)";;
  esac
done

# Normalise a trailing slash so "$PREFIX/bin" comparisons (warn_other_installs)
# don't spuriously fire when the user passes e.g. --prefix /usr/local/.
PREFIX="${PREFIX%/}"
[ -n "$PREFIX" ] || PREFIX="/"

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

# User-owned (no sudo) caches for the GUI's self-installed entries/icons.
refresh_user_caches() {
  data="$(xdg_data_home)"
  if have update-desktop-database; then
    update-desktop-database "$data/applications" 2>/dev/null || true
  fi
  if have gtk-update-icon-cache; then
    gtk-update-icon-cache -f "$data/icons/hicolor" 2>/dev/null || true
  fi
}

# Set by the rm helpers so we can warn (issue #28) instead of a silent no-op
# when an uninstall matches nothing — usually a --prefix that differs from the
# original install, or a distro-package install we don't own.
REMOVED_ANY=0

rm_priv() { # remove system paths (may need root); record if any existed
  for p in "$@"; do [ -e "$p" ] && REMOVED_ANY=1; done
  run_priv rm -rf "$@"
}

rm_user() { # remove user-owned paths as the invoking user; record if any existed
  for p in "$@"; do
    if [ -e "$p" ] || [ -L "$p" ]; then REMOVED_ANY=1; rm -rf "$p"; fi
  done
}

# rm(1) unlinks the binary but never stops a process already running off it —
# the original bug: the daemon kept switching wallpapers post-uninstall.
# Stopping the xdg-autostart unit tears down the GUI/tray/daemon cgroup it
# launched at login; pkill mops up any manually started strays.
#
# Patterns are anchored to a binary invocation — "(^|/)name($|[[:space:]])" — so
# -f only matches the executable as argv[0] (bare name or full path), never an
# unrelated process that merely mentions the string (e.g. an editor open on
# crates/superpanels-daemon/src or a journalctl follower).
sp_pkill() { # <pkill flags...>: signal each running Superpanels process once
  for pat in \
    '(^|/)superpanels-daemon($|[[:space:]])' \
    '(^|/)superpanels-gui($|[[:space:]])'; do
    pkill "$@" -f "$pat" 2>/dev/null || true
  done
}

stop_processes() {
  if have systemctl; then
    systemctl --user stop 'app-superpanels@autostart.service' 2>/dev/null || true
  fi
  if have pkill; then
    sp_pkill
    sleep 1
    sp_pkill -9
  fi
}

warn_other_installs() {
  found=""
  for d in /usr/local/bin /usr/bin "$HOME/.local/bin"; do
    [ "$d" = "$PREFIX/bin" ] && continue
    for b in superpanels superpanels-gui superpanels-daemon; do
      [ -e "$d/$b" ] && found="$found $d/$b"
    done
  done
  if [ -n "$found" ]; then
    warn "other Superpanels binaries remain outside $PREFIX:$found"
    warn "these are from a distro package or a different --prefix; remove them with that method."
  fi
}

do_uninstall() {
  say "stopping Superpanels (daemon, tray, GUI)"
  stop_processes

  say "removing Superpanels from $PREFIX"
  rm_priv \
    "$PREFIX/bin/superpanels" \
    "$PREFIX/bin/superpanels-gui" \
    "$PREFIX/bin/superpanels-daemon" \
    "$PREFIX/share/applications/superpanels-gui.desktop" \
    "$PREFIX/share/doc/superpanels"
  for s in $ICON_SIZES; do
    rm_priv "$PREFIX/share/icons/hicolor/$s/apps/superpanels-gui.png"
  done

  # The GUI self-registers these into the user's XDG dirs at runtime; without
  # removing them the app lingers in the launcher and re-autostarts on login.
  config_home="$(xdg_config_home)"
  data_home="$(xdg_data_home)"
  say "removing autostart and desktop entries"
  rm_user "$config_home/autostart/superpanels.desktop"
  rm_user "$data_home/applications/superpanels-gui.desktop"
  for s in $ICON_SIZES; do
    rm_user "$data_home/icons/hicolor/$s/apps/superpanels-gui.png"
  done

  # Drop the generated xdg-autostart unit now its source .desktop is gone.
  if have systemctl; then
    systemctl --user daemon-reload 2>/dev/null || true
  fi

  refresh_caches
  refresh_user_caches

  if [ "$PURGE" -eq 1 ]; then
    say "purging configuration, data, cache, and state"
    cache_home="$(xdg_cache_home)"
    # superpanels/* are our own config/state and the cached rendered-wallpaper
    # images (XDG_CACHE_HOME/superpanels/temp); com.superpanels.app/* are the
    # Tauri/WebKitGTK identifier dirs the GUI's webview writes.
    rm_user \
      "$config_home/superpanels" \
      "$config_home/com.superpanels.app" \
      "$data_home/superpanels" \
      "$data_home/com.superpanels.app" \
      "$cache_home/superpanels" \
      "$cache_home/com.superpanels.app" \
      "$(xdg_state_home)/superpanels"
  fi

  warn_other_installs

  if [ "$REMOVED_ANY" -eq 0 ]; then
    warn "nothing to remove — no Superpanels files found at $PREFIX or in your XDG dirs."
    warn "if you installed with a different --prefix, re-run --uninstall with the same one."
  elif [ "$PURGE" -eq 1 ]; then
    say "done. Superpanels fully removed, including your configuration."
  else
    say "done. (config under $config_home/superpanels kept — use --purge to remove it too)"
  fi
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
