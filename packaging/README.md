# Packaging

How Superpanels is built and shipped. Design rationale lives in
[`docs/release/packaging.md`](../docs/release/packaging.md); this is the operational
checklist.

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

## Cutting a release

1. Land everything on `main`. Pick a version `X.Y.Z`.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `release.yml` builds on the tag and publishes the GitHub release with the
   universal tarball, the `.deb`/`.rpm`/`.AppImage`, and `SHA256SUMS`.

`install.sh` (repo root) always pulls the latest release's universal tarball, so it
needs no per-release change.

## Updating the AUR package

After a release exists:

1. Bump `pkgver` in `aur-superpanels/PKGBUILD`.
2. `cd aur-superpanels && updpkgsums` — fills in the real `sha256sums` for the tag
   tarball (the committed copy carries a `SKIP` placeholder).
3. `makepkg --printsrcinfo > .SRCINFO`.
4. `makepkg -f` to confirm it builds, then push to the AUR remote.
