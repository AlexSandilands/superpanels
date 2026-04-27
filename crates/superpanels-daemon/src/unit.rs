//! Systemd user unit file generation (`PLAN.md` §2.5).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Write a systemd user unit to the standard location and print instructions.
pub(crate) fn install_unit(socket_path: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current executable path")?;
    let unit_dir = unit_dir()?;
    std::fs::create_dir_all(&unit_dir).context("creating systemd unit dir")?;
    let unit_path = unit_dir.join("superpanels-daemon.service");
    let contents = unit_contents(&exe, socket_path);
    std::fs::write(&unit_path, &contents)
        .with_context(|| format!("writing unit file to {}", unit_path.display()))?;
    eprintln!("Installed: {}", unit_path.display());
    eprintln!();
    eprintln!("Enable and start with:");
    eprintln!("  systemctl --user daemon-reload");
    eprintln!("  systemctl --user enable --now superpanels-daemon.service");
    Ok(())
}

fn unit_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return Ok(p.join("systemd").join("user"));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user"));
    }
    anyhow::bail!("could not determine systemd user unit directory ($HOME not set)");
}

fn unit_contents(exe: &Path, socket_path: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=Superpanels wallpaper daemon\n\
         After=graphical-session.target\n\
         PartOf=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe} --foreground --socket {socket}\n\
         Restart=on-failure\n\
         RestartSec=5s\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        exe = exe.display(),
        socket = socket_path.display(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests on string content; failure is a test bug
mod tests {
    use super::*;

    #[test]
    fn unit_contents_includes_exe_and_socket() {
        // Arrange
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let sock = PathBuf::from("/run/user/1000/superpanels/daemon.sock");

        // Act
        let text = unit_contents(&exe, &sock);

        // Assert
        assert!(
            text.contains("/usr/bin/superpanels-daemon"),
            "missing exe: {text}"
        );
        assert!(text.contains("daemon.sock"), "missing socket: {text}");
        assert!(text.contains("[Unit]"), "missing [Unit]: {text}");
        assert!(text.contains("[Service]"), "missing [Service]: {text}");
        assert!(text.contains("[Install]"), "missing [Install]: {text}");
    }

    #[test]
    fn unit_contents_has_foreground_flag() {
        let exe = PathBuf::from("/bin/sp");
        let sock = PathBuf::from("/tmp/d.sock");
        let text = unit_contents(&exe, &sock);
        assert!(
            text.contains("--foreground"),
            "missing --foreground: {text}"
        );
    }
}
