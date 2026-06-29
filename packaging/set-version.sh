#!/usr/bin/env bash
# Stamp a release version into the workspace manifests and the Tauri config so
# packaged binaries report the right --version and bundles get the right names.
#
# Usage: packaging/set-version.sh <version>   # version without a leading 'v'
#
# The workspace pins its inter-crate path deps as "=0.0.0"; this replaces both
# the bare workspace version ("0.0.0") and those pins ("=0.0.0") in one pass,
# anchored on the surrounding quotes so substrings of other versions are safe.
set -euo pipefail

VERSION="${1:?usage: set-version.sh <version>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

files=(
  "$ROOT/Cargo.toml"
  "$ROOT/crates/superpanels-core/Cargo.toml"
  "$ROOT/crates/superpanels-cli/Cargo.toml"
  "$ROOT/crates/superpanels-daemon/Cargo.toml"
  "$ROOT/crates/superpanels-gui/Cargo.toml"
  "$ROOT/crates/superpanels-gui/tauri.conf.json"
)

for f in "${files[@]}"; do
  sed -i \
    -e "s/\"0\\.0\\.0\"/\"${VERSION}\"/g" \
    -e "s/\"=0\\.0\\.0\"/\"=${VERSION}\"/g" \
    "$f"
done

echo "stamped version ${VERSION}"
