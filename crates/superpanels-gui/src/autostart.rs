//! Login-autostart state for the GUI (tray + daemon at session start).
//!
//! Two worlds, unified behind [`is_enabled`] / [`set_enabled`]:
//!
//! - **Packaged / system installs** ship `superpanels.desktop` in a system
//!   autostart dir (`/etc/xdg/autostart`), so autostart is **on by default**.
//!   Disabling writes a per-user `Hidden=true` shadow with the same filename,
//!   which the XDG spec treats as removing the entry; enabling deletes it.
//! - **Dev runs and userland installs** have no system entry, so enabling
//!   writes a full `~/.config/autostart/superpanels.desktop` and disabling
//!   removes it (the classic behaviour).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::IpcError;

const FILE_NAME: &str = "superpanels.desktop";

// reason: `env WEBKIT_DISABLE_DMABUF_RENDERER=1` works around a WebKitGTK
// crash on Wayland (`Gdk-Message: Error 71`) seen on common KDE Plasma 6
// + Mesa/NVIDIA stacks. Mirrored in `.cargo/config.toml`, the justfile,
// `desktop_entry.rs`, and `packaging/superpanels-autostart.desktop`.
//
// `--tray` keeps login startup in the background: it installs the tray and
// starts the daemon without popping the window. The app-menu entry
// (`desktop_entry.rs`) deliberately omits it so a manual launch opens the GUI.
const DESKTOP_BODY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Superpanels\n\
Comment=Bezel-aware multi-monitor wallpaper manager\n\
Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 superpanels-gui --tray\n\
Icon=superpanels-gui\n\
Categories=Graphics;Utility;\n\
Terminal=false\n\
X-GNOME-Autostart-enabled=true\n";

// Per-user shadow that suppresses a packaged system autostart entry. `Hidden`
// is the spec's "treat as removed"; `X-GNOME-Autostart-enabled=false` covers
// GNOME, which keys off its own field.
const HIDDEN_BODY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Superpanels\n\
Hidden=true\n\
X-GNOME-Autostart-enabled=false\n";

fn autostart_dir() -> Result<PathBuf, IpcError> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|h| h.join(".config"))
        })
        .ok_or_else(|| IpcError::internal("neither $XDG_CONFIG_HOME nor $HOME is set"))?;
    Ok(base.join("autostart"))
}

fn user_path() -> Result<PathBuf, IpcError> {
    Ok(autostart_dir()?.join(FILE_NAME))
}

/// Whether a packaged autostart entry exists in a system config dir
/// (`$XDG_CONFIG_DIRS/autostart`, default `/etc/xdg/autostart`).
fn system_present() -> bool {
    let dirs = std::env::var_os("XDG_CONFIG_DIRS")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| OsString::from("/etc/xdg"));
    system_present_in(&dirs)
}

fn system_present_in(config_dirs: &OsStr) -> bool {
    std::env::split_paths(config_dirs).any(|dir| dir.join("autostart").join(FILE_NAME).exists())
}

pub(crate) fn is_enabled() -> bool {
    user_path().is_ok_and(|p| effective_enabled(&p, system_present()))
}

/// Effective state: a user file wins (enabled unless it disables us); with no
/// user file, the default is whether a system entry is present.
fn effective_enabled(user_path: &Path, system_present: bool) -> bool {
    match fs::read_to_string(user_path) {
        Ok(body) => !entry_disabled(&body),
        Err(_) => system_present,
    }
}

/// Whether a `.desktop` body marks the entry as off — `Hidden=true` (the XDG
/// "treat as removed") or GNOME's `X-GNOME-Autostart-enabled=false`. Parsed
/// leniently (trim + case-insensitive key and value) so an override written by
/// any desktop's autostart UI is honoured, not just our exact byte sequence.
fn entry_disabled(body: &str) -> bool {
    body.lines().any(|line| {
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        let (key, value) = (key.trim(), value.trim());
        (key.eq_ignore_ascii_case("Hidden") && value.eq_ignore_ascii_case("true"))
            || (key.eq_ignore_ascii_case("X-GNOME-Autostart-enabled")
                && value.eq_ignore_ascii_case("false"))
    })
}

pub(crate) fn set_enabled(enabled: bool) -> Result<(), IpcError> {
    set_enabled_at(&user_path()?, system_present(), enabled)
}

/// Toggle autostart, given the user file path and whether a system entry backs
/// it. Split out so tests avoid mutating the process-wide environment (which
/// would require `unsafe`, forbidden by the workspace).
fn set_enabled_at(user_path: &Path, system_present: bool, enabled: bool) -> Result<(), IpcError> {
    if enabled && !system_present {
        // No system entry to rely on — write our own enabling entry.
        write_body(user_path, DESKTOP_BODY)
    } else if !enabled && system_present {
        // Suppress the packaged entry with a `Hidden` shadow.
        write_body(user_path, HIDDEN_BODY)
    } else {
        // Either the system entry already autostarts us (enabled) or there's
        // nothing to suppress (disabled) — in both cases no user file is needed.
        remove_if_exists(user_path)
    }
}

fn write_body(path: &Path, body: &str) -> Result<(), IpcError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(IpcError::from)?;
    }
    fs::write(path, body).map_err(IpcError::from)
}

fn remove_if_exists(path: &Path) -> Result<(), IpcError> {
    if path.exists() {
        fs::remove_file(path).map_err(IpcError::from)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn no_system_entry_enable_writes_full_entry() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("autostart").join(FILE_NAME);
        set_enabled_at(&path, false, true).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("superpanels-gui --tray"));
        assert!(body.contains("WEBKIT_DISABLE_DMABUF_RENDERER=1"));
        assert!(effective_enabled(&path, false));
    }

    #[test]
    fn no_system_entry_disable_removes_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        set_enabled_at(&path, false, true).unwrap();
        set_enabled_at(&path, false, false).unwrap();
        assert!(!path.exists());
        assert!(!effective_enabled(&path, false));
    }

    #[test]
    fn system_entry_is_enabled_by_default() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME); // no user file
        assert!(effective_enabled(&path, true));
    }

    #[test]
    fn system_entry_disable_writes_hidden_shadow() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("autostart").join(FILE_NAME);
        set_enabled_at(&path, true, false).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("Hidden=true"));
        assert!(!effective_enabled(&path, true));
    }

    #[test]
    fn system_entry_reenable_removes_shadow() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("autostart").join(FILE_NAME);
        set_enabled_at(&path, true, false).unwrap(); // shadow it
        set_enabled_at(&path, true, true).unwrap(); // re-enable
        assert!(!path.exists());
        assert!(effective_enabled(&path, true));
    }

    #[test]
    fn system_present_scans_config_dirs() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        fs::create_dir_all(b.path().join("autostart")).unwrap();
        fs::write(b.path().join("autostart").join(FILE_NAME), b"x").unwrap();
        assert!(!system_present_in(a.path().as_os_str()));
        let joined = std::env::join_paths([a.path(), b.path()]).unwrap();
        assert!(system_present_in(&joined));
    }

    #[test]
    fn autostart_entry_launches_in_tray_mode() {
        // Login startup must come up backgrounded (tray + daemon, no window).
        assert!(DESKTOP_BODY.contains("superpanels-gui --tray"));
    }

    #[test]
    fn entry_disabled_honours_override_variants() {
        // Our own shadow, and shapes another desktop's autostart UI might write.
        assert!(entry_disabled(HIDDEN_BODY));
        assert!(entry_disabled("[Desktop Entry]\nHidden=true\n"));
        assert!(entry_disabled("[Desktop Entry]\nHidden=True\n"));
        assert!(entry_disabled("[Desktop Entry]\nHidden = true \n"));
        assert!(entry_disabled(
            "[Desktop Entry]\nX-GNOME-Autostart-enabled=false\n"
        ));
    }

    #[test]
    fn entry_disabled_false_for_an_enabling_entry() {
        assert!(!entry_disabled(DESKTOP_BODY));
        assert!(!entry_disabled("[Desktop Entry]\nHidden=false\n"));
        // A path that merely contains the word must not trip the parser.
        assert!(!entry_disabled("Exec=/opt/Hidden=true/superpanels-gui\n"));
    }
}
