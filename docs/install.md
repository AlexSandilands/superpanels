# Installing Superpanels

Every method installs the same build; pick the one that fits your distro. Each section includes how to remove it again.

- [Install script (any glibc Linux)](#install-script-any-glibc-linux)
- [Arch / CachyOS: pacman repo](#arch--cachyos-pacman-repo)
- [Native bundles (.deb / .rpm)](#native-bundles-deb--rpm)
- [Troubleshooting](#troubleshooting)
- [From source](#from-source)

## Runtime dependencies

The GUI needs two runtime libraries — **WebKitGTK 4.1** (renders the webview) and a **system-tray library** (provides the tray icon; the GUI won't start without it):

| Distro | WebKitGTK | System tray |
|---|---|---|
| Arch / CachyOS | `webkit2gtk-4.1` | `libayatana-appindicator` |
| Fedora / RHEL | `webkit2gtk4.1` | `libayatana-appindicator-gtk3` |
| Debian / Ubuntu | `libwebkit2gtk-4.1-0` | `libayatana-appindicator3-1` |

The install script offers to install these for you, and the pacman repo resolves them automatically. For the native bundles you install them yourself.

## Install script (any glibc Linux)

Pulls the latest release, drops the CLI, daemon, and GUI into place, and registers the app icon — on any glibc Linux distro.

```sh
curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh
```

**Options** (after `| sh -s --`):

| Option | Effect |
|---|---|
| `--version <v>` | install a specific release instead of the latest |
| `--prerelease` | install the newest release including prereleases (`rc`/`beta`) |
| `--prefix <dir>` | install root (default `/usr/local`; use `~/.local` for no sudo) |

After installing, the script scans for the two runtime libraries above and, if either is missing, prints the exact command for your distro and offers to run it.

### Uninstall

Same script with `--uninstall`. It stops the daemon and tray, then removes the binaries, the app-menu and autostart entries, and the icons. Your config under `~/.config/superpanels` is left untouched:

```sh
curl -fsSL https://raw.githubusercontent.com/AlexSandilands/superpanels/main/install.sh | sh -s -- --uninstall
```

To also delete your settings, slideshow state, and data, use `--purge` instead of `--uninstall`. If you installed with a custom `--prefix`, pass the same one when uninstalling.

## Arch / CachyOS: pacman repo

Add the signed Superpanels package repo — dependencies resolve automatically and `pacman -Syu` picks up new releases like any other package. It carries **stable releases only** (for prereleases, use the install script's `--prerelease`).

One-time key trust:

```sh
curl -fsSLo /tmp/superpanels.gpg https://alexsandilands.github.io/superpanels/superpanels.gpg
sudo pacman-key --add /tmp/superpanels.gpg
sudo pacman-key --lsign-key BC01ACB0DF880D61793D7C44094918A9D106F9DC
```

Then append to `/etc/pacman.conf`:

```ini
[superpanels]
SigLevel = Required DatabaseOptional
Server = https://alexsandilands.github.io/superpanels/$arch
```

And install:

```sh
sudo pacman -Syu superpanels-bin
```

Upgrades come with a normal `sudo pacman -Syu` afterwards.

### Uninstall

```sh
sudo pacman -Rns superpanels-bin                                   # remove the package (and now-unused deps)
sudo pacman-key --delete BC01ACB0DF880D61793D7C44094918A9D106F9DC  # untrust the signing key
sudo rm -f /var/lib/pacman/sync/superpanels.db*                    # forget the cached repo db
```

Then delete the `[superpanels]` section from `/etc/pacman.conf` and run `sudo pacman -Sy`.

## Native bundles (.deb / .rpm)

Each release attaches GUI bundles for users who prefer their native package manager. They're **plain files on the [releases page](https://github.com/AlexSandilands/superpanels/releases/latest)**, not a hosted repo — download the file and install it locally. They bundle the **GUI only** (Tauri's design); for the CLI and daemon, use the install script or build from source.

Install the [runtime dependencies](#runtime-dependencies) first, then:

**Debian / Ubuntu (`.deb`):**

```sh
sudo apt install ./superpanels-gui_<ver>_amd64.deb
```

**Fedora / RHEL (`.rpm`):**

```sh
sudo dnf install ./superpanels-gui-<ver>.x86_64.rpm
```

### Uninstall

The `.deb` and `.rpm` install a package named **`superpanels`**:

```sh
sudo apt remove superpanels      # Debian / Ubuntu
sudo dnf remove superpanels      # Fedora / RHEL
```

## Autostart on login

The install script and the pacman repo (and the source PKGBUILD) set Superpanels to **start in the tray on login by default** — it's a wallpaper manager, so it's meant to be running. The entry lives in `/etc/xdg/autostart/superpanels.desktop`, owned by the install, so removing Superpanels removes it too.

To turn it off, toggle **Autostart on login** off in the GUI (Settings → General), or use your desktop's autostart settings. Disabling writes a small `~/.config/autostart/superpanels.desktop` override.

That override lives in your home, so a package-manager removal (`pacman -R`, `apt/dnf remove`) can't clean it — it survives an uninstall. Harmless on its own, but if you later **reinstall and autostart unexpectedly stays off**, delete `~/.config/autostart/superpanels.desktop` to clear the stale override. (`install.sh --uninstall` removes it for you.)

(The `.deb`/`.rpm` bundles don't set up autostart; enable it from the GUI toggle if you want it.)

## Troubleshooting

**Laggy webview UI, or the GUI crashes on launch with `Gdk-Message: Error 71`**

The GUI auto-detects NVIDIA-on-Wayland and applies a WebKitGTK DMABUF-renderer
workaround only there (it's off elsewhere, so Intel/AMD keep GPU acceleration).
Override it if detection gets your setup wrong:

- Force it **on** (if you hit the crash): `WEBKIT_DISABLE_DMABUF_RENDERER=1 superpanels-gui`
- Force it **off** (NVIDIA driver no longer crashes, want acceleration back):
  `env -u WEBKIT_DISABLE_DMABUF_RENDERER superpanels-gui`

An explicit setting always wins over auto-detection.

## From source

See [Building from source](../README.md#building-from-source) in the README — a plain `cargo build`, or the bundled PKGBUILD (`makepkg -si`) for a `pacman`-tracked package on Arch that upgrades and uninstalls cleanly. Packaging and release mechanics live in [`packaging/README.md`](../packaging/README.md).
