//! Installs the app-menu `.desktop` entry and hicolor icons into
//! `$XDG_DATA_HOME` so the taskbar shows the app icon.
//!
//! On Wayland the compositor ignores any window-level icon: it resolves the
//! taskbar icon by matching the window's `app_id` against an installed
//! `<app_id>.desktop` entry and loading its `Icon=` from the icon theme.
//! Tauri leaves the GTK application id unset, so the `app_id` falls back to
//! the process name `superpanels-gui`. Without these files every launch —
//! including a packaged one until the user installs system-wide — shows the
//! generic Wayland gear.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::IpcError;

const APP_ID: &str = "superpanels-gui";

// Transparent-background variants: the taskbar/launcher draws the icon over
// the panel's own surface, where the navy bundle background reads as a hard
// rectangle. The opaque originals stay for the bundle and tray "Blue" style.
const ICONS: [(&str, &[u8]); 3] = [
    ("32x32", include_bytes!("../icons/32x32-transparent.png")),
    (
        "128x128",
        include_bytes!("../icons/128x128-transparent.png"),
    ),
    ("256x256", include_bytes!("../icons/icon-transparent.png")),
];

/// Install (or refresh) the desktop entry and icons for the current binary.
/// Rewrites only on content change, so repeat launches are no-ops.
///
/// No-op when a packaged/system install already provides the entry (pacman,
/// `.deb`/`.rpm`, or `install.sh` to a system prefix). Those own their files
/// under `/usr` and clean them on removal; a copy of ours in `$XDG_DATA_HOME`
/// would linger in the launcher after the package is uninstalled, which the
/// package manager can't reach.
pub(crate) fn install() -> Result<(), IpcError> {
    if system_entry_present() {
        return Ok(());
    }
    install_at(&data_dir()?, &exec_value())
}

/// Whether a `<app_id>.desktop` already exists in a system application dir
/// (`$XDG_DATA_DIRS/applications`, default `/usr/local/share` + `/usr/share`).
fn system_entry_present() -> bool {
    let dirs = std::env::var_os("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| OsString::from("/usr/local/share:/usr/share"));
    system_entry_present_in(&dirs)
}

fn system_entry_present_in(data_dirs: &OsStr) -> bool {
    std::env::split_paths(data_dirs).any(|dir| {
        dir.join("applications")
            .join(format!("{APP_ID}.desktop"))
            .exists()
    })
}

pub(crate) fn install_at(data_dir: &Path, exec: &str) -> Result<(), IpcError> {
    let desktop = data_dir
        .join("applications")
        .join(format!("{APP_ID}.desktop"));
    write_if_changed(&desktop, desktop_body(exec).as_bytes())?;
    for (size, png) in ICONS {
        let icon = data_dir
            .join("icons/hicolor")
            .join(size)
            .join("apps")
            .join(format!("{APP_ID}.png"));
        write_if_changed(&icon, png)?;
    }
    Ok(())
}

// The WebKitGTK DMABUF workaround is no longer baked into `Exec=` — the binary
// self-detects NVIDIA-on-Wayland and re-execs with the env set. See `dmabuf.rs`
// and GitHub #57.
fn desktop_body(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Superpanels\n\
         Comment=Bezel-aware multi-monitor wallpaper manager\n\
         Exec={exec}\n\
         Icon={APP_ID}\n\
         Categories=Graphics;Utility;\n\
         Terminal=false\n\
         StartupWMClass={APP_ID}\n"
    )
}

/// Path of the running binary, quoted for the desktop-entry `Exec` key, so
/// the menu entry launches whichever build last ran (dev target dir or an
/// installed binary). Falls back to the bare name if the path is unreadable.
fn exec_value() -> String {
    std::env::current_exe().ok().map_or_else(
        || APP_ID.to_owned(),
        |p| quote_exec_path(&p.to_string_lossy()),
    )
}

/// Quote a path per the desktop-entry spec's `Exec` rules: plain paths pass
/// through; anything else is double-quoted with `\` `"` `` ` `` `$` escaped.
fn quote_exec_path(path: &str) -> String {
    let plain = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '.' | '-' | '+'));
    if plain {
        return path.to_owned();
    }
    let mut out = String::with_capacity(path.len() + 2);
    out.push('"');
    for c in path.chars() {
        if matches!(c, '\\' | '"' | '`' | '$') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn data_dir() -> Result<PathBuf, IpcError> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|h| h.join(".local").join("share"))
        })
        .ok_or_else(|| IpcError::internal("neither $XDG_DATA_HOME nor $HOME is set"))
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<(), IpcError> {
    if fs::read(path).is_ok_and(|current| current == contents) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(IpcError::from)?;
    }
    fs::write(path, contents).map_err(IpcError::from)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn install_at_writes_desktop_entry_and_all_icon_sizes() {
        let tmp = tempdir().unwrap();
        install_at(tmp.path(), "superpanels-gui").unwrap();

        let desktop = tmp.path().join("applications/superpanels-gui.desktop");
        assert!(desktop.exists());
        for (size, _) in ICONS {
            let icon = tmp
                .path()
                .join(format!("icons/hicolor/{size}/apps/superpanels-gui.png"));
            assert!(icon.exists(), "missing icon for {size}");
        }
    }

    #[test]
    fn install_at_twice_is_idempotent() {
        let tmp = tempdir().unwrap();
        install_at(tmp.path(), "superpanels-gui").unwrap();
        let desktop = tmp.path().join("applications/superpanels-gui.desktop");
        let first = fs::read(&desktop).unwrap();
        install_at(tmp.path(), "superpanels-gui").unwrap();
        assert_eq!(fs::read(&desktop).unwrap(), first);
    }

    #[test]
    fn system_entry_present_detects_a_packaged_entry() {
        let tmp = tempdir().unwrap();
        let apps = tmp.path().join("applications");
        fs::create_dir_all(&apps).unwrap();
        // Absent until a "packaged" entry lands in the system dir.
        assert!(!system_entry_present_in(tmp.path().as_os_str()));
        fs::write(apps.join(format!("{APP_ID}.desktop")), b"x").unwrap();
        assert!(system_entry_present_in(tmp.path().as_os_str()));
    }

    #[test]
    fn system_entry_present_scans_every_data_dir() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        fs::create_dir_all(b.path().join("applications")).unwrap();
        fs::write(
            b.path().join(format!("applications/{APP_ID}.desktop")),
            b"x",
        )
        .unwrap();
        // Entry only in the second of the colon-joined dirs must still be found.
        let joined = std::env::join_paths([a.path(), b.path()]).unwrap();
        assert!(system_entry_present_in(&joined));
    }

    #[test]
    fn desktop_body_names_match_the_wayland_app_id() {
        // KDE resolves the taskbar icon by matching the window's `app_id`
        // (the binary name) to `<app_id>.desktop` and its `Icon=` line.
        // Renaming either side silently brings the generic gear icon back.
        let body = desktop_body("superpanels-gui");
        assert!(body.contains("Icon=superpanels-gui\n"));
        assert!(body.contains("StartupWMClass=superpanels-gui\n"));
    }

    #[test]
    fn desktop_body_execs_the_binary_directly() {
        // The DMABUF workaround now lives in the binary (`dmabuf.rs`, GitHub
        // #57), not the launcher: `Exec=` must invoke the binary with no `env`
        // prefix so non-NVIDIA GPUs keep DMABUF acceleration.
        let body = desktop_body("/usr/bin/superpanels-gui");
        assert!(body.contains("Exec=/usr/bin/superpanels-gui\n"));
        assert!(!body.contains("WEBKIT_DISABLE_DMABUF_RENDERER"));
    }

    #[test]
    fn quote_exec_path_passes_plain_paths_through() {
        assert_eq!(
            quote_exec_path("/usr/bin/superpanels-gui"),
            "/usr/bin/superpanels-gui"
        );
    }

    #[test]
    fn quote_exec_path_quotes_and_escapes_special_chars() {
        assert_eq!(
            quote_exec_path("/home/a user/target/debug/superpanels-gui"),
            "\"/home/a user/target/debug/superpanels-gui\""
        );
        assert_eq!(quote_exec_path("/tmp/$x\"y"), "\"/tmp/\\$x\\\"y\"");
    }
}
