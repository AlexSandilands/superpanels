//! One-time re-exec that enables the NVIDIA-on-Wayland `WebKitGTK` workarounds
//! only when the session looks like that stack — where showing the webview
//! crashes the process with a Wayland protocol error (`Error 71`). Applying the
//! workarounds everywhere forces a slow copy-fallback render path on GPUs that
//! handle the fast paths fine (Intel/AMD), so the binary detects and re-execs
//! itself instead of every launcher baking the env in. See GitHub #57 / #76
//! (and #8 for the upstream fix that retires this entirely). This is the single
//! source of truth — launchers no longer set the variables.
//!
//! The failing mechanism (captured with `WAYLAND_DEBUG=1`): with no workaround,
//! the webview initialises EGL, NVIDIA's egl-wayland attaches a
//! `wp_linux_drm_syncobj_surface_v1` to the window surface from a render
//! thread, and the GTK main thread's next commit lands without an acquire
//! point — `KWin` kills the connection ("explicit sync is used, but no acquire
//! point is set", surfacing as `Error 71`). `WEBKIT_DISABLE_DMABUF_RENDERER=1`
//! keeps the webview off EGL entirely, so the sync object never exists;
//! `__NV_DISABLE_EXPLICIT_SYNC=1` is belt-and-braces from Tauri's
//! Linux-graphics NVIDIA notes.

use std::path::Path;

const DMABUF_VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
const EXPLICIT_SYNC_VAR: &str = "__NV_DISABLE_EXPLICIT_SYNC";

/// Apply the workaround by re-execing with the env set, when warranted. Must
/// run before any thread spawns or GTK/WebKit initialises.
///
/// A no-op when the user has already set `WEBKIT_DISABLE_DMABUF_RENDERER`
/// (either value) — a deliberate choice always wins over detection — or when
/// the session isn't NVIDIA-on-Wayland.
pub(crate) fn apply() {
    // An explicit setting in either direction is the escape hatch, and it also
    // breaks the post-re-exec loop: the child inherits the `1` we set below, so
    // this returns immediately the second time through. Whether `=0` re-enables
    // DMABUF is WebKitGTK's call — some versions gate on the var's presence, not
    // its value — but honouring the user's setting untouched is the contract
    // either way (to force acceleration back on, unset the var rather than `=0`).
    // The DMABUF var is the loop-break sentinel: it is always among the ones we
    // set, so the child always sees it even when only the explicit-sync var was
    // the effective fix.
    if std::env::var_os(DMABUF_VAR).is_some() {
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
            wayland: wayland_session(
                std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
                std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
                default_wayland_socket_exists(std::env::var_os("XDG_RUNTIME_DIR").as_deref()),
            ),
            glx_vendor_nvidia: env_contains_ignore_case("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
            nvidia_dev_node: Path::new("/dev/nvidiactl").exists()
                || Path::new("/dev/nvidia0").exists(),
        }
    }
}

/// Env sniffing alone cannot decide this. At cold-boot login the autostart
/// unit can start before the session manager exports `WAYLAND_DISPLAY` /
/// `XDG_SESSION_TYPE` into the systemd activation environment — yet GTK still
/// reaches the compositor, because `wl_display_connect(NULL)` falls back to
/// `$XDG_RUNTIME_DIR/wayland-0`. So a missing env var must not read as "not
/// Wayland": probe for that default socket too, mirroring libwayland's own
/// fallback. Reproduce the failure with
/// `env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE superpanels-gui --tray`,
/// then Open from the tray. See GitHub #76.
fn wayland_session(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    default_socket_exists: bool,
) -> bool {
    session_type.is_some_and(|v| v.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|v| !v.is_empty())
        || default_socket_exists
}

/// Only `wayland-0`, exactly what `wl_display_connect(NULL)` tries: if the
/// compositor listens on a non-default socket and the env var is absent, GTK
/// falls back to X11 and the crash cannot happen — so neither should the
/// workaround.
fn default_wayland_socket_exists(runtime_dir: Option<&std::ffi::OsStr>) -> bool {
    runtime_dir.is_some_and(|dir| Path::new(dir).join("wayland-0").exists())
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
        .env(DMABUF_VAR, "1")
        .env(EXPLICIT_SYNC_VAR, "1")
        .exec();
    tracing::warn!(error = %err, "DMABUF workaround: re-exec failed; continuing without it");
}

#[cfg(not(target_os = "linux"))]
fn reexec_with_workaround() {}

fn env_contains_ignore_case(key: &str, needle: &str) -> bool {
    std::env::var(key).is_ok_and(|v| v.to_ascii_lowercase().contains(needle))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
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

    #[test]
    fn session_type_wayland_is_a_wayland_signal() {
        assert!(wayland_session(Some("wayland"), None, false));
        assert!(wayland_session(Some("Wayland"), None, false));
    }

    #[test]
    fn wayland_display_alone_is_a_wayland_signal() {
        assert!(wayland_session(None, Some("wayland-0"), false));
    }

    #[test]
    fn default_socket_alone_is_a_wayland_signal() {
        // The cold-boot autostart case: the unit starts before the session
        // manager exports WAYLAND_DISPLAY / XDG_SESSION_TYPE into the systemd
        // activation environment, but GTK connects via libwayland's
        // $XDG_RUNTIME_DIR/wayland-0 fallback anyway. See GitHub #76.
        assert!(wayland_session(None, None, true));
    }

    #[test]
    fn empty_wayland_display_is_not_a_wayland_signal() {
        assert!(!wayland_session(None, Some(""), false));
    }

    #[test]
    fn no_wayland_signal_when_nothing_is_set() {
        assert!(!wayland_session(None, None, false));
        assert!(!wayland_session(Some("x11"), None, false));
    }

    #[test]
    fn socket_probe_requires_a_runtime_dir_and_an_existing_socket() {
        assert!(!default_wayland_socket_exists(None));
        let dir = tempfile::tempdir().unwrap();
        assert!(!default_wayland_socket_exists(Some(dir.path().as_os_str())));
        std::fs::write(dir.path().join("wayland-0"), b"").unwrap();
        assert!(default_wayland_socket_exists(Some(dir.path().as_os_str())));
    }
}
