//! One-time re-exec that enables the `WebKitGTK` DMABUF-renderer workaround only
//! when the session looks like NVIDIA-on-Wayland — the stack where the
//! zero-copy DMABUF path crashes (`Gdk-Message: Error 71`). Applying it
//! everywhere forces a slow copy-fallback render path on GPUs that handle
//! DMABUF fine (Intel/AMD), so the binary detects and re-execs itself instead
//! of every launcher baking the env in. See GitHub #57 (and #8 for the
//! upstream fix that retires this entirely). This is the single source of
//! truth — launchers no longer set the variable.

use std::path::Path;

const ENV_VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// Apply the workaround by re-execing with the env set, when warranted. Must
/// run before any thread spawns or GTK/WebKit initialises.
///
/// A no-op when the user has already set `WEBKIT_DISABLE_DMABUF_RENDERER`
/// (either value) — a deliberate choice always wins over detection — or when
/// the session isn't NVIDIA-on-Wayland.
pub(crate) fn apply() {
    // An explicit setting in either direction is the escape hatch, and it also
    // breaks the post-re-exec loop: the child inherits the `1` we set below, so
    // this returns immediately the second time through.
    if std::env::var_os(ENV_VAR).is_some() {
        return;
    }
    if warranted(&Facts::probe()) {
        reexec_with_workaround();
    }
}

/// Session facts the decision reads. Gathered once so [`warranted`] stays a
/// pure function the tests can drive without touching the real env or `/dev`.
struct Facts {
    wayland: bool,
    glx_vendor_nvidia: bool,
    nvidia_dev_node: bool,
}

impl Facts {
    fn probe() -> Self {
        Self {
            wayland: env_eq_ignore_case("XDG_SESSION_TYPE", "wayland"),
            glx_vendor_nvidia: env_contains_ignore_case("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
            nvidia_dev_node: Path::new("/dev/nvidiactl").exists()
                || Path::new("/dev/nvidia0").exists(),
        }
    }
}

/// Warranted only on Wayland with a sign NVIDIA is the active GL stack. A
/// device node is a coarse hint — a hybrid laptop may actually render on its
/// iGPU — but "laggy-but-working" beats the DMABUF crash for the weak-NVIDIA
/// case that genuinely needs it, and the explicit env override is the escape
/// hatch for anything this gets wrong. See GitHub #57.
fn warranted(f: &Facts) -> bool {
    f.wayland && (f.glx_vendor_nvidia || f.nvidia_dev_node)
}

#[cfg(target_os = "linux")]
fn reexec_with_workaround() {
    use std::os::unix::process::CommandExt;

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            tracing::warn!(error = %e, "DMABUF workaround: cannot resolve current exe; continuing without it");
            return;
        }
    };
    // `exec` replaces this process image in place — no `std::env::set_var`,
    // which is `unsafe` and forbidden by the workspace. It only returns on
    // failure, in which case we carry on with the original (un-workaround'd)
    // process rather than aborting startup.
    let err = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(ENV_VAR, "1")
        .exec();
    tracing::warn!(error = %err, "DMABUF workaround: re-exec failed; continuing without it");
}

#[cfg(not(target_os = "linux"))]
fn reexec_with_workaround() {}

fn env_eq_ignore_case(key: &str, want: &str) -> bool {
    std::env::var(key).is_ok_and(|v| v.eq_ignore_ascii_case(want))
}

fn env_contains_ignore_case(key: &str, needle: &str) -> bool {
    std::env::var(key).is_ok_and(|v| v.to_ascii_lowercase().contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(wayland: bool, glx_vendor_nvidia: bool, nvidia_dev_node: bool) -> Facts {
        Facts {
            wayland,
            glx_vendor_nvidia,
            nvidia_dev_node,
        }
    }

    #[test]
    fn warranted_on_wayland_with_an_nvidia_signal() {
        assert!(warranted(&facts(true, true, false)));
        assert!(warranted(&facts(true, false, true)));
        assert!(warranted(&facts(true, true, true)));
    }

    #[test]
    fn not_warranted_off_wayland_even_with_nvidia() {
        assert!(!warranted(&facts(false, true, true)));
    }

    #[test]
    fn not_warranted_on_wayland_without_any_nvidia_signal() {
        assert!(!warranted(&facts(true, false, false)));
    }
}
