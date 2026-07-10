//! One-time re-exec that enables the NVIDIA-on-Wayland `WebKitGTK` workarounds
//! only when the session looks like that stack — where showing the webview
//! crashes the process with a Wayland protocol error (`Error 71`). Applying the
//! workarounds everywhere forces a slow copy-fallback render path on GPUs that
//! handle the fast paths fine (Intel/AMD), so the binary detects and re-execs
//! itself instead of every launcher baking the env in. See GitHub #57 / #76
//! (and #8 for the upstream fix that retires this entirely). This is the single
//! source of truth — launchers no longer set the variables.
//!
//! Two variables, because they address different halves of the crash:
//! - `WEBKIT_DISABLE_DMABUF_RENDERER=1` moves the renderer off the zero-copy
//!   GBM DMABUF path onto shared-memory buffers.
//! - `__NV_DISABLE_EXPLICIT_SYNC=1` disables NVIDIA's Wayland explicit-sync
//!   protocol, which is the actual trigger for the `Error 71` protocol abort on
//!   this stack and which the DMABUF variable alone does **not** dodge on a
//!   cold boot (the crash #76's fix was assumed to close, but did not). See
//!   Tauri's Linux-graphics NVIDIA notes.

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
            ),
            glx_vendor_nvidia: env_contains_ignore_case("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
            nvidia_dev_node: Path::new("/dev/nvidiactl").exists()
                || Path::new("/dev/nvidia0").exists(),
        }
    }
}

/// `XDG_SESSION_TYPE` alone is not enough: an `/etc/xdg/autostart` process
/// inherits a thinner environment than a login shell and may not have it, which
/// silently skipped the workaround and crashed the webview on first render.
/// `WAYLAND_DISPLAY` survives that. See GitHub #76.
fn wayland_session(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    session_type.is_some_and(|v| v.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|v| !v.is_empty())
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
        assert!(wayland_session(Some("wayland"), None));
        assert!(wayland_session(Some("Wayland"), None));
    }

    #[test]
    fn wayland_display_alone_is_a_wayland_signal() {
        // The autostart case: `/etc/xdg/autostart` does not pass through
        // `XDG_SESSION_TYPE`, but `WAYLAND_DISPLAY` is present. See GitHub #76.
        assert!(wayland_session(None, Some("wayland-0")));
    }

    #[test]
    fn empty_wayland_display_is_not_a_wayland_signal() {
        assert!(!wayland_session(None, Some("")));
    }

    #[test]
    fn no_wayland_signal_when_neither_is_set() {
        assert!(!wayland_session(None, None));
        assert!(!wayland_session(Some("x11"), None));
    }
}
