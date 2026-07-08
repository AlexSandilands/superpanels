#!/usr/bin/env bash
# Build superpanels-bin for $VERSION and publish it to the signed pacman repo
# on the gh-pages branch (served by GitHub Pages). Runs as root inside an
# archlinux:base-devel container — release.yml's `pacman-repo` job — with git
# and github-cli installed and the repo checked out as the working directory.
#
# Env:
#   VERSION                  release version, no leading v (e.g. 0.2.0)
#   GH_TOKEN                 token for `gh release download` and the gh-pages push
#   GITHUB_REPOSITORY        owner/repo
#   PACMAN_GPG_PRIVATE_KEY   ASCII-armored signing key (secret)
#   PACMAN_GPG_PASSPHRASE    key passphrase (secret; optional, empty = none)
set -euo pipefail

: "${VERSION:?}" "${GH_TOKEN:?}" "${GITHUB_REPOSITORY:?}" "${PACMAN_GPG_PRIVATE_KEY:?}"

TARBALL="superpanels-$VERSION-x86_64-linux.tar.gz"

# makepkg refuses to run as root, so everything build/sign-related runs as a
# throwaway user. sudo -H gives it a real $HOME for gnupg.
useradd -m builder
as_builder() { sudo -u builder -H "$@"; }

# Loopback pinentry so gpg (invoked by makepkg --sign and repo-add --sign)
# never prompts; works for both passphrase-less and passphrase-set keys.
GNUPG_DIR=/home/builder/.gnupg
install -d -m700 -o builder -g builder "$GNUPG_DIR"
echo "pinentry-mode loopback" > "$GNUPG_DIR/gpg.conf"
echo "allow-loopback-pinentry" > "$GNUPG_DIR/gpg-agent.conf"
if [ -n "${PACMAN_GPG_PASSPHRASE:-}" ]; then
  (umask 077 && printf '%s' "$PACMAN_GPG_PASSPHRASE" > "$GNUPG_DIR/passphrase")
  echo "passphrase-file $GNUPG_DIR/passphrase" >> "$GNUPG_DIR/gpg.conf"
fi
chown -R builder:builder "$GNUPG_DIR"

printf '%s' "$PACMAN_GPG_PRIVATE_KEY" | as_builder gpg --batch --import
KEYID="$(as_builder gpg --list-secret-keys --with-colons | awk -F: '/^sec/ {print $5; exit}')"
[ -n "$KEYID" ] || { echo "error: no secret key imported" >&2; exit 1; }
echo "==> signing as key $KEYID"

BUILD_DIR="$(mktemp -d)"
cp packaging/pacman-repo/PKGBUILD "$BUILD_DIR/"

echo "==> downloading $TARBALL from release v$VERSION"
gh release download "v$VERSION" --repo "$GITHUB_REPOSITORY" --dir "$BUILD_DIR" \
  --pattern "$TARBALL" --pattern SHA256SUMS
(cd "$BUILD_DIR" && grep -- "$TARBALL" SHA256SUMS | sha256sum -c -)

SHA256="$(sha256sum "$BUILD_DIR/$TARBALL" | awk '{print $1}')"
sed -i "s/^pkgver=.*/pkgver=$VERSION/" "$BUILD_DIR/PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('$SHA256')/" "$BUILD_DIR/PKGBUILD"
chown -R builder:builder "$BUILD_DIR"

# --nodeps: the runtime depends (webkit2gtk-4.1 …) matter on the user's box,
# not in this container — package() only repacks the tarball. makepkg finds
# the already-downloaded tarball next to the PKGBUILD, so no re-download.
echo "==> building and signing superpanels-bin-$VERSION"
(cd "$BUILD_DIR" && as_builder env GPGKEY="$KEYID" makepkg --nodeps --sign)

PAGES_DIR="$(mktemp -d)/pages"
REMOTE="https://x-access-token:${GH_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
if ! git clone --depth 1 --branch gh-pages "$REMOTE" "$PAGES_DIR" 2>/dev/null; then
  echo "==> no gh-pages branch yet — starting one"
  git init -b gh-pages "$PAGES_DIR"
  git -C "$PAGES_DIR" remote add origin "$REMOTE"
fi

touch "$PAGES_DIR/.nojekyll"
as_builder gpg --export --armor "$KEYID" > "$PAGES_DIR/superpanels.gpg"
mkdir -p "$PAGES_DIR/x86_64"
cp "$BUILD_DIR"/superpanels-bin-*.pkg.tar.zst{,.sig} "$PAGES_DIR/x86_64/"

cd "$PAGES_DIR/x86_64"

# Keep current + previous package version only (rollback window).
ls superpanels-bin-*.pkg.tar.zst | sort -V | head -n -2 | while read -r old; do
  echo "==> pruning $old"
  rm -f "$old" "$old.sig"
done

# Rebuild the db from the surviving packages instead of incremental repo-add —
# idempotent on re-runs and self-heals a corrupt/missing db.
rm -f superpanels.db* superpanels.files*
chown builder:builder .
as_builder repo-add --sign --key "$KEYID" superpanels.db.tar.gz ./*.pkg.tar.zst

# repo-add leaves superpanels.db{,.sig} as symlinks; Pages serves git symlinks
# as one-line text files, so replace them with real copies.
for link in superpanels.db superpanels.files superpanels.db.sig superpanels.files.sig; do
  if [ -L "$link" ]; then
    target="$(readlink "$link")"
    rm "$link"
    cp "$target" "$link"
  fi
done

cd "$PAGES_DIR"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add -A
if git diff --cached --quiet; then
  echo "==> repo unchanged — nothing to publish"
else
  git commit -m "pacman repo: superpanels-bin $VERSION"
  git push origin gh-pages
  echo "==> published superpanels-bin $VERSION to the pacman repo"
fi
