# Packaging

How Superpanels is built and shipped — the canonical packaging doc. Post-release
distribution work (AUR auto-push, COPR, apt repo, crates.io, Flatpak, signing,
`aarch64`) is tracked in [GitHub issues](https://github.com/AlexSandilands/superpanels/issues).

## Layout

| Path | What |
|---|---|
| `set-version.sh` | Stamp a release version into every manifest + `tauri.conf.json`. |
| `assemble-release.sh` | Collect built binaries + Tauri bundles into `dist/` as the release artefacts. |
| `superpanels-gui.desktop` | Canonical menu entry. `Exec=` bakes in the WebKitGTK DMABUF workaround (GitHub #8). Must keep `Icon=`/`StartupWMClass=`/filename as `superpanels-gui` — the Wayland `app_id` resolves the taskbar icon by matching it (see `crates/superpanels-gui/src/desktop_entry.rs`). |
| `aur-superpanels/` | The single `superpanels` AUR package (CLI + daemon + GUI). |
| `pacman-repo/` | `superpanels-bin` PKGBUILD (repackages the release tarball) + `publish.sh`, the CI script that signs it and publishes the self-hosted pacman repo (below). |

The native `.deb`/`.rpm`/`.AppImage` get their menu entry from
`../crates/superpanels-gui/desktop-entry.hbs` (wired via `desktopTemplate` in
`tauri.conf.json`). It mirrors Tauri's default template but injects the same
`WEBKIT_DISABLE_DMABUF_RENDERER=1` Exec prefix (#8), so a package-manager launch
behaves like a dev launch. Keep it to the variables Tauri exposes — `categories`,
`comment`, `exec`, `icon`, `name` — or the bundle build fails to render it.

## What a release publishes (and what it doesn't)

`release.yml` does two things on a `v*` tag: the `release` job builds the artefacts
and attaches them to a **GitHub Release**, and — on stable (non-prerelease) tags
only — the `pacman-repo` job packages that release into the self-hosted
[pacman repo](#self-hosted-pacman-repo-superpanels-bin). There is still **no** AUR
push and no dnf/apt repository.

| Channel | Automated on tag? | What a user actually runs |
|---|---|---|
| `curl … /install.sh \| sh` | ✅ yes | installs immediately from the release tarball |
| `pacman -S superpanels-bin` (self-hosted repo) | ✅ stable tags only | one-time repo + key setup ([`docs/install.md`](../docs/install.md)), then normal `pacman -Syu` upgrades |
| `makepkg -si` (in `aur-superpanels/`) | n/a — always in the repo | builds from source, installs a `pacman`-tracked package; no AUR needed (below) |
| `.deb` / `.rpm` / `.AppImage` | ⚠️ built & attached, **not** in any repo | download the file, then `sudo dnf install ./…rpm` / `sudo apt install ./…deb` |
| `yay -S superpanels` (AUR) | ❌ no | works only after a **manual** push to the AUR remote (below; registrations currently locked — see issue #46) |
| `dnf install superpanels` / `apt install superpanels` | ❌ no | needs a hosted repo (Fedora COPR / apt PPA) — not built yet |
| `cargo install superpanels` | ❌ no | needs crates.io metadata + publish — not done yet |

The native bundles are **plain files on the release page**, not a repository: `dnf`
and `apt` resolve a bare package *name* from repo metadata (`createrepo` output / an
apt archive), which the pipeline does not produce. So `dnf install superpanels` will
not work until a COPR (or equivalent) is stood up; today the rpm/deb are local-file
installs only.

The release artefacts are:

- `superpanels-<ver>-x86_64-linux.tar.gz` — the **universal bundle**: all three
  binaries + `superpanels-gui.desktop` + hicolor icons + licences. This is what
  `install.sh` pulls, so it's the only artefact carrying the CLI and daemon.
- `superpanels-gui_<ver>_amd64.deb`, `superpanels-gui-<ver>.x86_64.rpm`,
  `superpanels-gui_<ver>_x86_64.AppImage` — the Tauri GUI bundles (GUI-only by
  Tauri's design), for users who prefer their native package manager.
- `SHA256SUMS` over all of the above.

## Versioning

The workspace version stays `0.0.0` in git; `set-version.sh` stamps the tag version
into the manifests + `tauri.conf.json` at build time (in CI's release job and in the
AUR `prepare()`), so binaries report the right `--version` without a committed bump.
A committed bump is only needed for crates.io, which reads the version from the
manifest as checked in.

## Cutting a release

1. Land everything on `main`. Pick a version `X.Y.Z`. Make sure CI is green and
   `CHANGELOG.md` has the new version's notes.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `release.yml` builds on the tag and publishes the GitHub release with the
   universal tarball, the `.deb`/`.rpm`/`.AppImage`, and `SHA256SUMS`. On a
   stable tag (no `-` in it), the `pacman-repo` job then publishes
   `superpanels-bin` to the self-hosted pacman repo automatically.

That produces a GitHub Release plus, for stable tags, a pacman-repo update (see
the table above). It does **not** push to the AUR or to any `dnf`/`apt`
repository; the AUR step below is manual.

`install.sh` (repo root) always pulls the latest release's universal tarball, so it
needs no per-release change.

> **Tip:** shake the pipeline out with a `vX.Y.Z-rc.1` pre-release tag before the
> real tag — the live Tauri bundle / AppImage build isn't exercised by PR CI.

## Build & install from source with `makepkg`

`aur-superpanels/PKGBUILD` doubles as a from-source installer that needs **no AUR
account and no hosted repo** — it works straight from a checkout today. It compiles
the workspace and installs a real, `pacman`-tracked package (so upgrades and
uninstall are clean), pulling the runtime `depends` in automatically:

```sh
cd packaging/aur-superpanels
makepkg -si
```

Needs `base-devel` (for `makepkg`) plus the `makedepends` below; `makepkg` installs
the runtime `depends` for you. It builds the **latest tagged release** — `source=()`
downloads that tag's tarball, so local working-tree edits are *not* included; for
those, use the `cargo build` flow in the root README's "Building from source".

> On **CachyOS** — and any Arch box with `lto` enabled globally in `makepkg.conf` —
> this only links because the PKGBUILD sets `options=('!lto')`; see
> [Known constraints](#known-constraints).

## Updating the AUR package

After a release exists:

1. Bump `pkgver` in `aur-superpanels/PKGBUILD`.
2. `cd aur-superpanels && updpkgsums` — refresh `sha256sums` for the new tag's
   tarball (the committed PKGBUILD carries the previous release's real hash, not a
   `SKIP` placeholder).
3. `makepkg --printsrcinfo > .SRCINFO`.
4. `makepkg -f` to confirm it builds, then push to the AUR remote.

The AUR `PKGBUILD` runs `set-version.sh` in `prepare()` (the tag tarball ships
`0.0.0`) and builds without `--locked` so the version rewrite can re-resolve the
lock. `depends=(webkit2gtk-4.1 gtk3 libayatana-appindicator)`; `makedepends=(rust
nodejs npm)` since it builds the frontend then the workspace and installs files by
hand (no Tauri bundler). It also sets `options=('!lto')` — the bundled `rusqlite`
`sqlite3.c` must stay a plain object rather than a GCC-LTO one (see Known
constraints).

## Self-hosted pacman repo (`superpanels-bin`)

A signed pacman repository served by **GitHub Pages** from the **`gh-pages`
branch** (Pages is configured as "deploy from branch", not an Actions deploy —
the branch *is* the repo state, so each publish is an incremental commit and the
history doubles as an audit log). Users set it up once ([`docs/install.md`](../docs/install.md))
and get dependency-resolved installs plus normal `-Syu` upgrades.

Layout on `gh-pages`:

```
superpanels.gpg                  # ASCII-armored public signing key (stable URL)
x86_64/
  superpanels.db / .files        # repo-add output — real files, not symlinks
  superpanels.db.tar.gz / …      #   (Pages serves git symlinks as text files)
  superpanels-bin-<ver>-<rel>-x86_64.pkg.tar.zst  + .sig
```

### How a release lands there

The `pacman-repo` job in `release.yml` (`needs: release`, `container:
archlinux:base-devel`) runs `pacman-repo/publish.sh`, which:

1. creates a `builder` user (makepkg refuses to run as root) and imports the
   signing key into its keyring with loopback pinentry (no prompts in CI);
2. downloads the just-published universal tarball with `gh release download`
   and verifies it against the release's `SHA256SUMS`;
3. stamps `pkgver`/`sha256sums` into a copy of `pacman-repo/PKGBUILD` and runs
   `makepkg --nodeps --sign` (no compile — it repackages the tarball, so the
   `!lto` concern from the source PKGBUILD doesn't apply, and runtime deps
   don't need to exist in the container);
4. checks out `gh-pages` (creating it on first run), copies in the package +
   signature and the exported public key, **prunes to the current + previous
   package version**, rebuilds the db from scratch with `repo-add --sign`
   (idempotent on re-runs), replaces the `.db`/`.files` symlinks with real
   copies, and commits + pushes.

**Prereleases never publish here** — the job skips any tag containing `-`
(`v1.2.3-rc.1` etc.); bleeding-edge testers use `install.sh --prerelease`. A
separate `[superpanels-testing]` repo is possible later if wanted.

### One-time setup (repo owner)

- Generate the signing key (no expiry, sign-only):
  `gpg --quick-gen-key "Superpanels Release Signing <profile.alex@proton.me>" ed25519 sign never`
- Add Actions secrets: `PACMAN_GPG_PRIVATE_KEY` (ASCII-armored secret-key
  export) and `PACMAN_GPG_PASSPHRASE` (only if the key has one).
- Enable GitHub Pages: repo Settings → Pages → deploy from branch `gh-pages`,
  root. (The branch appears after the first publish run.)
- Put the key's full fingerprint in the root README's `pacman-key --lsign-key`
  line so users can trust it. The live key is
  `BC01ACB0DF880D61793D7C44094918A9D106F9DC` (ed25519, `Superpanels Release
  Signing <profile.alex@proton.me>`); re-keying means updating that line.

### Testing / re-publishing

Stable-tag-only means an rc tag can't exercise this job. Instead, run the
**workflow_dispatch** on `release.yml` with an existing release version (a
throwaway draft release works too) — it runs just the `pacman-repo` job against
that release's assets. Re-running for an already-published version is safe: the
db is rebuilt, not appended.

### Relationship to the AUR

`pacman-repo/PKGBUILD` is deliberately AUR-shaped (`provides`/`conflicts` on
`superpanels`, same `depends` as `aur-superpanels/`). When AUR registrations
reopen (locked as of mid-2026 — issue #46), it becomes the `superpanels-bin`
AUR submission nearly as-is.

## crates.io (CLI escape hatch — not yet published)

`cargo install superpanels` is the headless/CLI-only path. Before first publish:

- Fill in each crate's `description`, `keywords`
  (`["wallpaper", "kde", "wayland", "multi-monitor", "linux"]`), `categories`
  (`command-line-utilities`, `gui`), `repository`, and `readme`.
- `[package.metadata.docs.rs]` on `superpanels-core` so docs.rs builds with
  `--all-features`.
- Publish order: `superpanels-core` → `superpanels-cli` → `superpanels-daemon` →
  `superpanels-gui`.

## Known constraints

- **glibc, not static.** Binaries link dynamic glibc; the release builds on the
  oldest supported runner (`ubuntu-22.04`) for the widest compatibility. Minimal
  containers need `glibc >= 2.17`.
- **WebKitGTK DMABUF crash** on Wayland + NVIDIA + Plasma 6 — the
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround stays until upstream fixes it (#8).
  It is duplicated, in lockstep, across the dev launchers (`.cargo/config.toml`,
  `justfile`), the runtime-written entries (`autostart.rs`, `desktop_entry.rs`), and
  the packaged launchers (`superpanels-gui.desktop`, `superpanels-autostart.desktop`,
  `desktop-entry.hbs`). Collapsing
  these — plus the `[Desktop Entry]` body and the icon-size→hicolor mapping — onto a
  single generated source is tracked as a follow-up.
- **makepkg LTO breaks the bundled sqlite link.** `rusqlite`'s bundled `sqlite3.c`,
  compiled under makepkg's `lto` option, becomes a GCC-GIMPLE (fat-LTO) object whose
  `sqlite3_*` symbols `rust-lld` can't resolve — the daemon link fails with
  `undefined symbol: sqlite3_column_type` and friends. Stock Arch leaves LTO opt-in,
  so it never bites there; **CachyOS (the primary target) enables it globally**, so
  the PKGBUILD carries `options=('!lto')`. Don't drop it, and build-test the PKGBUILD
  on an LTO-enabled machine — a stock-Arch build passes without it and hides the bug.
- **AUR review feedback** may require PKGBUILD changes; budget a day after submission.
