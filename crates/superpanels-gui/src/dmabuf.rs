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
        tracing::info!("nvidia-wayland workaround: env already set (user or re-exec)");
        return;
    }
    let facts = Facts::probe();
    if warranted(&facts) {
        tracing::info!("nvidia-wayland workaround: re-execing with env set");
        reexec_with_workaround();
    }
}

/// Session facts the decision reads. Gathered once so [`warranted`] stays a
/// pure function the tests can drive without touching the real env or `/dev`.
struct Facts {
    wayland: bool,
    nvidia: bool,
}

impl Facts {
    fn probe() -> Self {
        let glx_vendor_nvidia = env_contains_ignore_case("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
        // The /dev/nvidia* nodes are created lazily by the NVIDIA userspace
        // (nvidia-modprobe) the first time something opens the device — at
        // cold boot that can be *this process's own* later EGL init, so at
        // probe time they may not exist yet (observed born the same second
        // the autostart unit started). The kernel module, by contrast, must
        // be loaded before the compositor can bring up an NVIDIA-driven
        // session at all, so /sys/module/nvidia is ordered strictly before
        // any Wayland socket we could have connected to.
        let nvidia_dev_node =
            Path::new("/dev/nvidiactl").exists() || Path::new("/dev/nvidia0").exists();
        let nvidia_module_loaded = Path::new("/sys/module/nvidia").exists();
        let wayland = wayland_session(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
            default_wayland_socket_exists(std::env::var_os("XDG_RUNTIME_DIR").as_deref()),
        );
        // The decision has been wrong at cold boot twice without leaving a
        // trace (#76, PR #80) — always journal the raw probes so the next
        // boot-only failure is diagnosable from `journalctl --user` alone.
        tracing::info!(
            wayland,
            glx_vendor_nvidia,
            nvidia_dev_node,
            nvidia_module_loaded,
            "nvidia-wayland workaround probe"
        );
        Self {
            wayland,
            nvidia: nvidia_signal(glx_vendor_nvidia, nvidia_dev_node, nvidia_module_loaded),
        }
    }
}

/// Any one signal is enough: the env hint, the (lazily created) device nodes,
/// or the loaded kernel module — the only one guaranteed to exist by the time
/// an NVIDIA-driven compositor has a socket up.
fn nvidia_signal(glx_vendor_nvidia: bool, dev_node: bool, module_loaded: bool) -> bool {
    glx_vendor_nvidia || dev_node || module_loaded
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

/// Warranted only on Wayland with a sign NVIDIA is the active GL stack. The
/// module/device signals are coarse hints — a hybrid laptop may actually render
/// on its iGPU — but "laggy-but-working" beats the DMABUF crash for the
/// weak-NVIDIA case that genuinely needs it, and the explicit env override is
/// the escape hatch for anything this gets wrong. See GitHub #57.
fn warranted(f: &Facts) -> bool {
    f.wayland && f.nvidia
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

    #[test]
    fn warranted_needs_both_wayland_and_nvidia() {
        assert!(warranted(&Facts {
            wayland: true,
            nvidia: true
        }));
        assert!(!warranted(&Facts {
            wayland: false,
            nvidia: true
        }));
        assert!(!warranted(&Facts {
            wayland: true,
            nvidia: false
        }));
    }

    #[test]
    fn any_single_nvidia_probe_is_a_signal() {
        assert!(nvidia_signal(true, false, false));
        assert!(nvidia_signal(false, true, false));
        assert!(nvidia_signal(false, false, true));
        assert!(!nvidia_signal(false, false, false));
    }

    #[test]
    fn module_alone_is_an_nvidia_signal() {
        // The cold-boot race: /dev/nvidia* are created lazily and can be born
        // *after* the probe (observed same-second as the autostart unit's
        // start), while /sys/module/nvidia is loaded strictly before the
        // compositor's socket can exist. See GitHub #76.
        assert!(nvidia_signal(false, false, true));
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
