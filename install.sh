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
PRERELEASE=0
ICON_SIZES="32x32 128x128 256x256"

# Default XDG dirs the GUI writes into at runtime (autostart entry, app-menu
# entry + icons, and config/data/state). install.sh never creates these, but
# uninstall must clean them or the launcher keeps listing a removed app.
xdg_config_home() { printf '%s' "${XDG_CONFIG_HOME:-$HOME/.config}"; }
xdg_data_home()   { printf '%s' "${XDG_DATA_HOME:-$HOME/.local/share}"; }
xdg_state_home()  { printf '%s' "${XDG_STATE_HOME:-$HOME/.local/state}"; }
xdg_cache_home()  { printf '%s' "${XDG_CACHE_HOME:-$HOME/.cache}"; }

# Colorize warnings (stderr) and the dependency-OK line (stdout) only when the
# stream is a TTY, so piped/redirected output (`| sh`, logs) stays escape-free.
if [ -t 2 ]; then YELLOW="$(printf '\033[33m')"; YRESET="$(printf '\033[0m')"; else YELLOW=""; YRESET=""; fi
if [ -t 1 ]; then GREEN="$(printf '\033[32m')";  GRESET="$(printf '\033[0m')";  else GREEN="";  GRESET=""; fi

say()  { printf '==> %s\n' "$*"; }
warn() { printf '%swarning: %s%s\n' "$YELLOW" "$*" "$YRESET" >&2; }
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
  --prerelease     install the newest release including prereleases (rc/beta)
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
    --prerelease) PRERELEASE=1; shift;;
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

# The GUI's runtime libraries. WebKitGTK is a NEEDED link dep (its absence
# fails at the dynamic linker), but the appindicator tray lib is dlopen()ed at
# runtime, so a missing one only surfaces as a startup panic (GitHub #35). We
# probe for both after install and warn — in yellow — only about what's absent.
#
# Per-distro package names mirror the declared deps of the native packages; keep
# this in sync with packaging/aur-superpanels/PKGBUILD (depends=...) and
# crates/superpanels-gui/tauri.conf.json (linux.deb/.rpm depends).

# ldconfig lives in /sbin, often off a non-root PATH — try the usual locations.
ldconfig_cache() {
  if have ldconfig; then ldconfig -p 2>/dev/null
  elif [ -x /sbin/ldconfig ]; then /sbin/ldconfig -p 2>/dev/null
  elif [ -x /usr/sbin/ldconfig ]; then /usr/sbin/ldconfig -p 2>/dev/null
  fi
}

# True if any of the given sonames is findable by the runtime loader.
lib_present() { # <soname>...
  for so in "$@"; do
    ldconfig_cache | grep -Fq -- "$so" && return 0
    for d in /usr/lib /usr/lib64 /lib /lib64 /usr/lib/x86_64-linux-gnu /usr/local/lib; do
      [ -e "$d/$so" ] && return 0
    done
  done
  return 1
}

distro_family() {
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    id="$(. /etc/os-release 2>/dev/null && echo "${ID:-} ${ID_LIKE:-}")"
    case "$id" in
      *arch*)            echo arch;   return;;
      *fedora*|*rhel*)   echo fedora; return;;
      *debian*|*ubuntu*) echo debian; return;;
    esac
  fi
  echo unknown
}

pkg_name() { # <dep> <family>
  case "$1.$2" in
    webkit.fedora)       echo webkit2gtk4.1;;
    webkit.debian)       echo libwebkit2gtk-4.1-0;;
    webkit.*)            echo webkit2gtk-4.1;;
    appindicator.fedora) echo libayatana-appindicator-gtk3;;
    appindicator.debian) echo libayatana-appindicator3-1;;
    appindicator.arch)   echo libayatana-appindicator;;
    appindicator.*)      echo libayatana-appindicator3;;
  esac
}

install_cmd() { # <family> <pkgs>
  case "$1" in
    arch)   echo "sudo pacman -Syu --needed $2";;
    fedora) echo "sudo dnf install $2";;
    debian) echo "sudo apt install $2";;
    *)      echo "install with your package manager: $2";;
  esac
}

# Sets MISSING_PKGS / MISSING_LINES from a fresh library probe, so the check
# can run again after a package install.
probe_missing_deps() { # <family>
  MISSING_PKGS=""
  MISSING_LINES=""
  if ! lib_present libwebkit2gtk-4.1.so.0; then
    p="$(pkg_name webkit "$1")"
    MISSING_PKGS="${MISSING_PKGS:+$MISSING_PKGS }$p"
    MISSING_LINES="${MISSING_LINES}  $p — renders the GUI's webview (required)
"
  fi
  if ! lib_present libayatana-appindicator3.so.1 libappindicator3.so.1; then
    p="$(pkg_name appindicator "$1")"
    MISSING_PKGS="${MISSING_PKGS:+$MISSING_PKGS }$p"
    MISSING_LINES="${MISSING_LINES}  $p — provides the system-tray icon; the GUI won't start without it (required)
"
  fi
}

# Under `curl … | sh` stdin is the pipe, so interactive prompts must go through
# /dev/tty; when it can't be opened (CI, non-interactive shells) callers fall
# back to print-only behaviour.
tty_usable() { { : </dev/tty; } 2>/dev/null; }

# run_priv gates on $PREFIX writability, which is the wrong test here: a
# package install always needs root, even for a --prefix ~/.local install.
run_pkg_install() { # <family> <pkg>...
  family="$1"; shift
  case "$family" in
    # -Syu, not -S/-Sy: installing against a stale sync db is a partial upgrade
    # (Arch wiki warns it can break the system, or just fail "target not found").
    # Since we now run this for the user, do the supported full refresh+upgrade;
    # pacman still lists the transaction and asks to confirm on /dev/tty.
    arch)   set -- pacman -Syu --needed "$@";;
    fedora) set -- dnf install "$@";;
    debian) set -- apt install "$@";;
    *)      return 1;;
  esac
  # stdin from /dev/tty so the package manager's own confirmation prompt (and
  # sudo's password prompt) stay interactive under `curl … | sh`.
  if [ "$(id -u)" -eq 0 ]; then
    "$@" </dev/tty
  elif have sudo; then
    sudo "$@" </dev/tty
  else
    warn "no sudo available — run the command above as root, then re-run this script to re-check"
    return 1
  fi
}

offer_dep_install() { # <family>
  # Unknown distro → no concrete command to offer; the printed hint is all we
  # have. Never auto-run a package install without a TTY confirmation.
  case "$1" in arch|fedora|debian) ;; *) return 0;; esac
  tty_usable || return 0

  printf 'install now? [Y/n] ' >&2
  read -r ans </dev/tty || return 0
  case "$ans" in [nN]*) return 0;; esac

  # shellcheck disable=SC2086 # MISSING_PKGS is an intentionally split list
  if ! run_pkg_install "$1" $MISSING_PKGS; then
    warn "dependency install did not complete — the GUI may not start until it does"
    return 0
  fi

  probe_missing_deps "$1"
  if [ -z "$MISSING_PKGS" ]; then
    printf '%s==> all GUI runtime dependencies satisfied%s\n' "$GREEN" "$GRESET"
  else
    warn "still missing after install: $MISSING_PKGS"
  fi
}

check_runtime_deps() {
  family="$(distro_family)"
  probe_missing_deps "$family"

  echo
  if [ -z "$MISSING_PKGS" ]; then
    printf '%s==> all GUI runtime dependencies satisfied%s\n' "$GREEN" "$GRESET"
    return
  fi

  # Only the header carries the `warning:` label; the detail/command lines are
  # plain (still yellow on a TTY, still stderr) so the block reads as one notice.
  warn "the GUI needs these runtime libraries, which appear to be missing:"
  printf '%s' "$MISSING_LINES" | while IFS= read -r line; do
    [ -n "$line" ] && printf '%s%s%s\n' "$YELLOW" "$line" "$YRESET" >&2
  done
  printf '%sinstall them with:%s\n' "$YELLOW" "$YRESET" >&2
  printf '%s  %s%s\n' "$YELLOW" "$(install_cmd "$family" "$MISSING_PKGS")" "$YRESET" >&2
  if [ "$family" = arch ]; then
    printf '%s  (or add the signed pacman repo, whose package declares these — see the README'\''s Install section)%s\n' "$YELLOW" "$YRESET" >&2
  fi

  offer_dep_install "$family"
}

do_install() {
  if [ -z "$VERSION" ]; then
    if [ "$PRERELEASE" -eq 1 ]; then
      # `releases/latest` excludes prereleases; hit the unfiltered list and
      # take the first entry (GitHub returns newest-first) so `rc`/`beta`
      # tags — which the release workflow flags as prereleases — are reachable.
      say "resolving newest release (including prereleases)"
      VERSION="$(fetch_stdout "https://api.github.com/repos/$REPO/releases?per_page=1" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
      [ -n "$VERSION" ] || err "could not determine the newest release"
    else
      say "resolving latest release"
      VERSION="$(fetch_stdout "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
      [ -n "$VERSION" ] || err "could not determine the latest release"
    fi
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
  check_runtime_deps
}

case "$ACTION" in
  install) do_install;;
  uninstall) do_uninstall;;
esac
