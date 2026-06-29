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

The native `.deb`/`.rpm`/`.AppImage` get their menu entry from
`../crates/superpanels-gui/desktop-entry.hbs` (wired via `desktopTemplate` in
`tauri.conf.json`). It mirrors Tauri's default template but injects the same
`WEBKIT_DISABLE_DMABUF_RENDERER=1` Exec prefix (#8), so a package-manager launch
behaves like a dev launch. Keep it to the variables Tauri exposes — `categories`,
`comment`, `exec`, `icon`, `name` — or the bundle build fails to render it.

## What a release publishes (and what it doesn't)

`release.yml` does exactly one thing on a `v*` tag: it builds the artefacts and
attaches them to a **GitHub Release**. That is the whole automated surface — the
pipeline does **no** package-repository publishing and **no** AUR push.

| Channel | Automated on tag? | What a user actually runs |
|---|---|---|
| `curl … /install.sh \| sh` | ✅ yes | installs immediately from the release tarball |
| `.deb` / `.rpm` / `.AppImage` | ⚠️ built & attached, **not** in any repo | download the file, then `sudo dnf install ./…rpm` / `sudo apt install ./…deb` |
| `yay -S superpanels` (AUR) | ❌ no | works only after a **manual** push to the AUR remote (below) |
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
   universal tarball, the `.deb`/`.rpm`/`.AppImage`, and `SHA256SUMS`.

That produces a **GitHub Release only** (see the table above). It does **not** push
to the AUR or to any `dnf`/`apt` repository; the AUR step below is manual.

`install.sh` (repo root) always pulls the latest release's universal tarball, so it
needs no per-release change.

> **Tip:** shake the pipeline out with a `vX.Y.Z-rc.1` pre-release tag before the
> real tag — the live Tauri bundle / AppImage build isn't exercised by PR CI.

## Updating the AUR package

After a release exists:

1. Bump `pkgver` in `aur-superpanels/PKGBUILD`.
2. `cd aur-superpanels && updpkgsums` — fills in the real `sha256sums` for the tag
   tarball (the committed copy carries a `SKIP` placeholder).
3. `makepkg --printsrcinfo > .SRCINFO`.
4. `makepkg -f` to confirm it builds, then push to the AUR remote.

The AUR `PKGBUILD` runs `set-version.sh` in `prepare()` (the tag tarball ships
`0.0.0`) and builds without `--locked` so the version rewrite can re-resolve the
lock. `depends=(webkit2gtk-4.1 gtk3 libayatana-appindicator)`; `makedepends=(rust
nodejs npm)` since it builds the frontend then the workspace and installs files by
hand (no Tauri bundler).

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
  the packaged launchers (`superpanels-gui.desktop`, `desktop-entry.hbs`). Collapsing
  these — plus the `[Desktop Entry]` body and the icon-size→hicolor mapping — onto a
  single generated source is tracked as a follow-up.
- **AUR review feedback** may require PKGBUILD changes; budget a day after submission.
