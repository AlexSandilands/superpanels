#!/usr/bin/env bash
# Wipe Superpanels local data so the next launch enters the first-run flow.
# Backs up to ~/superpanels-backup-<timestamp>/ unless --no-backup is given.
#
# Usage:
#   scripts/reset-local-data.sh           # backup, then prompt before wiping
#   scripts/reset-local-data.sh -y        # backup, skip the prompt
#   scripts/reset-local-data.sh --no-backup -y
#   scripts/reset-local-data.sh --autostart   # also remove autostart + systemd unit

set -euo pipefail

BACKUP=1
ASSUME_YES=0
CLEAR_AUTOSTART=0

for arg in "$@"; do
    case "$arg" in
        --no-backup) BACKUP=0 ;;
        -y|--yes) ASSUME_YES=1 ;;
        --autostart) CLEAR_AUTOSTART=1 ;;
        -h|--help)
            awk 'NR>1 && /^#/ {sub(/^# ?/,""); print; next} NR>1 {exit}' "$0"
            exit 0
            ;;
        *)
            echo "unknown arg: $arg" >&2
            exit 2
            ;;
    esac
done

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/superpanels"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/superpanels"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/superpanels"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/superpanels"
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp}/superpanels"

AUTOSTART_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/superpanels.desktop"
SYSTEMD_UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/superpanels-daemon.service"

TARGETS=("$CONFIG_DIR" "$STATE_DIR" "$DATA_DIR" "$CACHE_DIR" "$RUNTIME_DIR")
if [[ $CLEAR_AUTOSTART -eq 1 ]]; then
    TARGETS+=("$AUTOSTART_FILE" "$SYSTEMD_UNIT")
fi

echo "Will remove:"
for t in "${TARGETS[@]}"; do
    if [[ -e "$t" ]]; then
        echo "  $t"
    else
        echo "  $t  (does not exist — skipped)"
    fi
done

if [[ $ASSUME_YES -ne 1 ]]; then
    read -r -p "Continue? [y/N] " reply
    if [[ ! "$reply" =~ ^[Yy]$ ]]; then
        echo "aborted."
        exit 1
    fi
fi

echo "Stopping daemon (if running)..."
systemctl --user stop superpanels-daemon.service 2>/dev/null || true
pkill -f superpanels-daemon 2>/dev/null || true
pkill -f superpanels-gui 2>/dev/null || true

if [[ $BACKUP -eq 1 ]]; then
    BACKUP_DIR="$HOME/superpanels-backup-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$BACKUP_DIR"
    for src in "$CONFIG_DIR" "$STATE_DIR" "$DATA_DIR" "$CACHE_DIR"; do
        if [[ -e "$src" ]]; then
            cp -a "$src" "$BACKUP_DIR/"
        fi
    done
    echo "Backed up to $BACKUP_DIR"
fi

for t in "${TARGETS[@]}"; do
    rm -rf -- "$t"
done

if [[ $CLEAR_AUTOSTART -eq 1 ]]; then
    systemctl --user daemon-reload 2>/dev/null || true
fi

echo "Done. Next launch will be a fresh first-run."
