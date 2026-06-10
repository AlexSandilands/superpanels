//! `~/.config/autostart/superpanels.desktop` writer.
//!
//! XDG autostart spec: a `.desktop` file in the user's `autostart` directory
//! is launched by every compliant DE on session start. Removing the file
//! disables it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::IpcError;

const FILE_NAME: &str = "superpanels.desktop";

// reason: `env WEBKIT_DISABLE_DMABUF_RENDERER=1` works around a WebKitGTK
// crash on Wayland (`Gdk-Message: Error 71`) seen on common KDE Plasma 6
// + Mesa/NVIDIA stacks. Mirrored in `.cargo/config.toml` for dev `cargo run`.
const DESKTOP_BODY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Superpanels\n\
Comment=Bezel-aware multi-monitor wallpaper manager\n\
Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 superpanels-gui\n\
Icon=superpanels-gui\n\
Categories=Graphics;Utility;\n\
Terminal=false\n\
X-GNOME-Autostart-enabled=true\n";

pub(crate) fn autostart_dir() -> Result<PathBuf, IpcError> {
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

pub(crate) fn desktop_path() -> Result<PathBuf, IpcError> {
    Ok(autostart_dir()?.join(FILE_NAME))
}

pub(crate) fn is_enabled() -> bool {
    desktop_path().is_ok_and(|p| p.exists())
}

pub(crate) fn set_enabled(enabled: bool) -> Result<(), IpcError> {
    set_enabled_at(&desktop_path()?, enabled)
}

/// Toggle the autostart desktop file at an explicit path. Used by tests so
/// they can avoid mutating the process-wide environment (which would require
/// `unsafe`, forbidden by the workspace).
pub(crate) fn set_enabled_at(path: &Path, enabled: bool) -> Result<(), IpcError> {
    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(IpcError::from)?;
        }
        fs::write(path, DESKTOP_BODY).map_err(IpcError::from)?;
    } else if path.exists() {
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
    fn set_enabled_at_true_writes_desktop_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("autostart").join(FILE_NAME);
        set_enabled_at(&path, true).unwrap();
        assert!(path.exists());
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("superpanels-gui"));
        assert!(body.contains("WEBKIT_DISABLE_DMABUF_RENDERER=1"));
    }

    #[test]
    fn set_enabled_at_false_removes_desktop_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        set_enabled_at(&path, true).unwrap();
        assert!(path.exists());
        set_enabled_at(&path, false).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn set_enabled_at_false_when_missing_is_noop() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        set_enabled_at(&path, false).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn desktop_body_uses_gui_binary_name() {
        // Frontend depends on this name to detect "is autostart pointing at us"
        assert!(DESKTOP_BODY.contains("superpanels-gui"));
    }

    #[test]
    fn desktop_body_includes_webkit_dmabuf_workaround() {
        // The `env WEBKIT_DISABLE_DMABUF_RENDERER=1` prefix exists to dodge a
        // WebKitGTK Wayland crash; without it the autostart entry will flash
        // the window and exit on affected setups. Don't drop the prefix
        // without first confirming the upstream fix has shipped on Arch.
        assert!(DESKTOP_BODY.contains("WEBKIT_DISABLE_DMABUF_RENDERER=1"));
    }
}
