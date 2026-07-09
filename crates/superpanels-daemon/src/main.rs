#![forbid(unsafe_code)]

//! Superpanels background daemon.

// reason: tracing replaces println for structured output; the daemon has no
// intentional stdout output. Print suppression warnings are correct.
#![allow(clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use superpanels_core::ipc::socket_path;
use superpanels_core::slideshow::persist_state as persist_slideshow;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify, watch};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

mod apply;
mod display_watch;
mod pool;
mod schedule;
mod server;
mod state;
mod thumbnail_cache;
mod timer;
mod unit;
mod watcher;

use crate::state::DaemonState;

#[derive(Parser, Debug)]
#[command(name = "superpanels-daemon", about = "Superpanels background daemon")]
struct Cli {
    /// Stay in the foreground (useful for systemd user units). Default: fork
    /// to background using a child process.
    #[arg(long)]
    foreground: bool,
    /// Alternate Unix socket path (overrides `$XDG_RUNTIME_DIR`).
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
    /// Use an alternate config file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Generate and install a systemd user unit file, then exit.
    #[arg(long)]
    install_unit: bool,
    /// Increase log verbosity (`-v` = debug, `-vv` = trace).
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Suppress non-error output.
    #[arg(long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // `--install-unit` doesn't need a runtime or a socket.
    if cli.install_unit {
        let sock = cli.socket.clone().unwrap_or_else(socket_path);
        if let Err(e) = unit::install_unit(&sock) {
            eprintln!("error: {e:#}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    // In background mode, re-exec ourselves with `--foreground` and exit.
    if !cli.foreground {
        // Init tracing in the parent too: its stderr is still the launching
        // terminal (only the detached child's stdio is nulled), so the
        // "started" line and any setsid-missing warning actually reach the user.
        init_tracing(cli.verbose, cli.quiet);
        if let Err(e) = daemonize(&cli) {
            eprintln!("error: could not start daemon in background: {e:#}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    init_tracing(cli.verbose, cli.quiet);

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building Tokio runtime")
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e:#}");
            return ExitCode::from(1);
        }
    };

    match rt.block_on(run_daemon(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!(error = %e, "daemon exited with error");
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

#[allow(clippy::too_many_lines)] // reason: linear startup sequence; splitting reads worse than the current shape
async fn run_daemon(cli: Cli) -> Result<()> {
    let sock_path = cli.socket.clone().unwrap_or_else(socket_path);

    // Create the socket directory with 0700 so only the owner can connect.
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating socket directory {}", parent.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // XDG_RUNTIME_DIR is already 0700 by spec; only the fallback
            // /tmp/superpanels-$UID directory needs us to enforce 0700, and
            // there a permission-set failure is fatal — the socket would be
            // world-reachable on a multi-user host.
            let in_xdg_runtime = std::env::var_os("XDG_RUNTIME_DIR")
                .filter(|v| !v.is_empty())
                .is_some_and(|v| sock_path.starts_with(std::path::PathBuf::from(v)));
            let perms = std::fs::Permissions::from_mode(0o700);
            if let Err(e) = std::fs::set_permissions(parent, perms) {
                if in_xdg_runtime {
                    warn!(
                        dir = %parent.display(),
                        error = %e,
                        "could not chmod socket dir; trusting XDG_RUNTIME_DIR's own 0700"
                    );
                } else {
                    return Err(anyhow::Error::from(e)
                        .context(format!("setting 0700 on socket dir {}", parent.display())));
                }
            }
        }
    }

    let listener = bind_exclusive(&sock_path)
        .await
        .context("binding IPC socket")?;
    // Restrict the socket file itself to 0600 so even if the parent dir
    // becomes traversable the connect() is owner-only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("setting 0600 on socket {}", sock_path.display()))?;
    }
    info!(socket = %sock_path.display(), "daemon IPC socket bound");

    let mut daemon_state = DaemonState::load(cli.config.as_deref())?;

    // Resume where the user left off: an explicit `default_profile` wins,
    // otherwise the persisted last-active profile from the previous run.
    let resume = daemon_state.resume_path.as_deref().and_then(|p| {
        superpanels_core::resume::load(p).unwrap_or_else(|e| {
            warn!(error = %e, "ignoring unreadable resume state");
            None
        })
    });
    let initial_profile = choose_initial_profile(&daemon_state.config, resume.as_ref());
    if let Some(r) = resume {
        // Seed apply metadata immediately — the wallpaper from the previous
        // run is still on screen (compositors persist it), so `current_state`
        // should reflect it even before the startup re-apply lands.
        daemon_state.active_profile = initial_profile.clone();
        daemon_state.last_apply_backend = r.last_apply_backend;
        daemon_state.last_apply_unix_secs = r.last_apply_unix_secs;
    }

    // Restore the initial profile's slideshow picker from disk so history and
    // sort order survive a restart, then apply it at startup.
    if let Some(ref name) = initial_profile {
        if let Some(dir) = DaemonState::state_dir() {
            daemon_state.restore_slideshow(name, &dir.join("slideshow-state.json"));
        }
    }
    let state = Arc::new(Mutex::new(daemon_state));

    let (timer_tx, timer_rx) = watch::channel::<Option<Duration>>(None);
    let (watcher_tx, watcher_rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();
    // Broadcast tick fired by `display_watch` when the OS pushes a display
    // configuration change. Sized small — late subscribers don't need history.
    let (monitors_tx, _monitors_rx) = tokio::sync::broadcast::channel::<()>(8);

    // The watcher lives inside DaemonState so handlers (specifically
    // `save_config`) can rebuild it when library roots change without a
    // daemon restart.
    {
        let mut guard = state.lock().await;
        guard.watcher_tx = Some(watcher_tx);
        guard.monitors_tx = Some(monitors_tx.clone());
        guard.refresh_watcher();
    }

    // Spawn background tasks.
    let timer_state = Arc::clone(&state);
    tokio::spawn(async move { timer::run_timer(timer_state, timer_rx).await });

    let watcher_state = Arc::clone(&state);
    tokio::spawn(async move { watcher::run_watcher(watcher_state, watcher_rx).await });

    let sched_state = Arc::clone(&state);
    let sched_timer_tx = timer_tx.clone();
    tokio::spawn(async move { schedule::run_schedule_checker(sched_state, sched_timer_tx).await });

    // OS-rotation push: KDE kscreen D-Bus signal (best-effort, KDE only).
    // Manual refresh in Settings > Monitors covers stacks where the signal
    // doesn't fire. See §6.3.
    display_watch::spawn(Arc::clone(&state), monitors_tx.clone());

    // Apply the default profile (if set) after a short delay to allow compositor
    // readiness. At session login the compositor (e.g. plasmashell) may not be up
    // when the first apply fires, so retry with backoff before giving up —
    // otherwise the desktop keeps whatever wallpaper the compositor cached.
    if let Some(profile_name) = initial_profile {
        let state_clone = Arc::clone(&state);
        let timer_tx_clone = timer_tx.clone();
        tokio::spawn(async move {
            // Cumulative delays from boot: ~0.5s, ~2s, ~5s.
            const BACKOFF_MS: [u64; 3] = [500, 1500, 3000];
            for (attempt, delay_ms) in BACKOFF_MS.iter().enumerate() {
                tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
                let req = superpanels_core::ipc::IpcRequest {
                    v: superpanels_core::ipc::PROTOCOL_VERSION,
                    method: "apply_profile".to_owned(),
                    params: serde_json::json!({"name": profile_name}),
                };
                let resp = server::dispatch_for_tests(
                    req,
                    Arc::clone(&state_clone),
                    timer_tx_clone.clone(),
                )
                .await;
                if resp.is_ok() {
                    break;
                }
                if attempt + 1 == BACKOFF_MS.len() {
                    warn!(error = ?resp.error, attempts = BACKOFF_MS.len(),
                        "default profile apply on startup failed after retries");
                } else {
                    warn!(error = ?resp.error, attempt = attempt + 1,
                        "default profile apply on startup failed; retrying");
                }
            }
        });
    }

    // Boot catch-up: if any enabled schedule rule fired earlier today and
    // resolves to a profile different from the active one, apply it.
    let catch_up_state = Arc::clone(&state);
    let catch_up_timer_tx = timer_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(750)).await;
        let (target, active) = {
            let guard = catch_up_state.lock().await;
            (
                guard.schedule_checker.boot_catch_up(&guard.config),
                guard.active_profile.clone(),
            )
        };
        if let Some(name) = target {
            if active.as_deref() == Some(name.as_str()) {
                return;
            }
            info!(profile = %name, "schedule boot catch-up applying past rule");
            let req = superpanels_core::ipc::IpcRequest {
                v: superpanels_core::ipc::PROTOCOL_VERSION,
                method: "apply_profile".to_owned(),
                params: serde_json::json!({"name": name}),
            };
            let resp = server::dispatch_for_tests(req, catch_up_state, catch_up_timer_tx).await;
            if !resp.is_ok() {
                warn!(error = ?resp.error, "boot catch-up apply failed");
            }
        }
    });

    // Main accept loop — runs until SIGTERM or a `shutdown` IPC request.
    let shutdown = Arc::new(Notify::new());
    let server_state = Arc::clone(&state);
    let server_shutdown = Arc::clone(&shutdown);
    let server_handle = tokio::spawn(async move {
        server::run_server(listener, server_state, timer_tx, server_shutdown).await;
    });

    // Wait for SIGTERM, SIGINT, or an IPC `shutdown`, then persist before exit.
    wait_for_shutdown(&shutdown).await;
    info!("shutdown signal received; persisting state and exiting");

    server_handle.abort();
    persist_daemon_state(&state).await;
    info!("daemon exited cleanly");
    Ok(())
}

async fn persist_daemon_state(state: &Arc<Mutex<DaemonState>>) {
    let guard = state.lock().await;
    let Some(dir) = DaemonState::state_dir() else {
        warn!("could not determine state dir; skipping state persistence");
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(error = %e, "could not create state dir");
        return;
    }
    // Library DB writes are flushed on each mutation; nothing to persist here.
    // Persist slideshow state.
    if let Some(picker) = &guard.slideshow_picker {
        let state_path = dir.join("slideshow-state.json");
        if let Err(e) = persist_slideshow(picker.state(), &state_path) {
            warn!(error = %e, "failed to persist slideshow state");
        }
    }
}

async fn wait_for_shutdown(shutdown: &Notify) {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "SIGTERM handler could not be registered; only Ctrl-C will stop the daemon");
            // Fall back to waiting on the IPC signal only (SIGKILL still works).
            shutdown.notified().await;
            return;
        }
    };
    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "SIGINT handler could not be registered");
            shutdown.notified().await;
            return;
        }
    };
    tokio::select! {
        _ = sigterm.recv() => info!("received SIGTERM"),
        _ = sigint.recv()  => info!("received SIGINT"),
        () = shutdown.notified() => info!("received shutdown IPC request"),
    }
}

/// Bind to `path` exclusively. If `path` exists but is stale (no daemon
/// listening), removes it and re-binds. Errors if a live daemon is found.
async fn bind_exclusive(path: &std::path::Path) -> Result<UnixListener> {
    match UnixListener::bind(path) {
        Ok(l) => return Ok(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {}
        Err(e) => return Err(e.into()),
    }
    // Check if the existing socket is live.
    if UnixStream::connect(path).await.is_ok() {
        bail!("a daemon is already running at {}", path.display());
    }
    // Stale socket — remove and retry.
    std::fs::remove_file(path)
        .with_context(|| format!("removing stale socket at {}", path.display()))?;
    Ok(UnixListener::bind(path)?)
}

/// Startup profile: an explicit `default_profile` is honoured unconditionally
/// (a not-found apply error is more actionable than a silent skip); a resumed
/// last-active profile must still exist in config, since a stale resume file
/// would otherwise warn on every boot.
fn choose_initial_profile(
    config: &superpanels_core::config::Config,
    resume: Option<&superpanels_core::resume::ResumeState>,
) -> Option<String> {
    config.general.default_profile.clone().or_else(|| {
        resume
            .map(|r| r.active_profile.clone())
            .filter(|name| config.profiles.iter().any(|p| p.name == *name))
    })
}

/// Re-exec the daemon in the foreground as a detached child so it survives the
/// launching terminal's SIGHUP.
///
/// Packaged installs should prefer the systemd user unit (`--install-unit`),
/// which runs `--foreground` directly and never reaches this path.
fn daemonize(cli: &Cli) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current executable")?;
    let setsid = find_in_path("setsid");
    if setsid.is_none() {
        warn!(
            "setsid(1) not found; detaching via a new process group only. The \
             daemon may still be killed by SIGHUP if the launching terminal \
             closes — install util-linux, or run under the systemd user unit \
             (superpanels-daemon --install-unit)."
        );
    }
    let mut cmd = build_daemon_command(&exe, cli, setsid.as_deref());
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning background daemon process")?;
    info!(socket = ?cli.socket, detached = setsid.is_some(), "daemon started in background");
    Ok(())
}

/// Build the re-exec command for the background daemon.
///
/// Prefers `setsid(1)`: it starts the daemon in a fresh session with no
/// controlling terminal, so a terminal hangup can never reach it. Calling
/// `setsid(2)` in-process would require `pre_exec` (unsafe) or a manual fork,
/// both barred by `#![forbid(unsafe_code)]`, hence the subprocess. When
/// `setsid(1)` is absent, falls back to `process_group(0)` so the daemon at
/// least leaves the launching foreground process group (weaker: it keeps the
/// controlling terminal, so SIGHUP is still possible).
fn build_daemon_command(exe: &Path, cli: &Cli, setsid: Option<&Path>) -> std::process::Command {
    let mut cmd = if let Some(setsid) = setsid {
        let mut cmd = std::process::Command::new(setsid);
        cmd.arg(exe);
        cmd
    } else {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(exe);
        cmd.process_group(0);
        cmd
    };
    cmd.arg("--foreground");
    if let Some(sock) = &cli.socket {
        cmd.arg("--socket").arg(sock);
    }
    if let Some(cfg) = &cli.config {
        cmd.arg("--config").arg(cfg);
    }
    for _ in 0..cli.verbose {
        cmd.arg("-v");
    }
    if cli.quiet {
        cmd.arg("--quiet");
    }
    cmd
}

/// First executable match for `bin` on `$PATH`, or `None` if not found.
fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| candidate.is_file())
}

fn init_tracing(verbose: u8, quiet: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = if quiet {
            "error"
        } else {
            match verbose {
                0 => "info",
                1 => "debug",
                _ => "trace",
            }
        };
        EnvFilter::new(level)
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use std::ffi::{OsStr, OsString};

    use super::*;
    use superpanels_core::config::Config;
    use superpanels_core::resume::ResumeState;

    fn cli_with(socket: Option<PathBuf>, config: Option<PathBuf>, verbose: u8, quiet: bool) -> Cli {
        Cli {
            foreground: false,
            socket,
            config,
            install_unit: false,
            verbose,
            quiet,
        }
    }

    #[test]
    fn build_command_prefers_setsid_and_passes_exe_as_first_arg() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let setsid = PathBuf::from("/usr/bin/setsid");
        let cmd = build_daemon_command(&exe, &cli_with(None, None, 0, false), Some(&setsid));
        assert_eq!(cmd.get_program(), setsid.as_os_str());
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args, vec![exe.as_os_str(), OsStr::new("--foreground")]);
    }

    #[test]
    fn build_command_without_setsid_execs_daemon_directly() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let cmd = build_daemon_command(&exe, &cli_with(None, None, 0, false), None);
        assert_eq!(cmd.get_program(), exe.as_os_str());
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args, vec![OsStr::new("--foreground")]);
    }

    #[test]
    fn build_command_forwards_socket_config_and_verbosity() {
        let exe = PathBuf::from("/usr/bin/superpanels-daemon");
        let sock = PathBuf::from("/run/user/1000/sp.sock");
        let cfg = PathBuf::from("/home/me/config.toml");
        let cli = cli_with(Some(sock.clone()), Some(cfg.clone()), 2, true);
        let cmd = build_daemon_command(&exe, &cli, None);
        let args: Vec<OsString> = cmd.get_args().map(OsStr::to_owned).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("--foreground"),
                OsString::from("--socket"),
                sock.into_os_string(),
                OsString::from("--config"),
                cfg.into_os_string(),
                OsString::from("-v"),
                OsString::from("-v"),
                OsString::from("--quiet"),
            ]
        );
    }

    #[test]
    fn find_in_path_returns_none_for_missing_binary() {
        assert!(find_in_path("definitely-not-a-real-binary-xyz-superpanels").is_none());
    }

    fn resume(name: &str) -> ResumeState {
        ResumeState {
            active_profile: name.to_owned(),
            last_apply_backend: None,
            last_apply_unix_secs: None,
        }
    }

    fn config_with_profile(name: &str) -> Config {
        use std::collections::HashMap;
        use superpanels_core::TopologyFingerprint;
        use superpanels_core::config::{Profile, ProfileBody, StandardLayer, StandardProfile};
        use superpanels_core::layout::ImageRectMm;

        let now = superpanels_core::config::now_timestamp();
        let mut cfg = Config::default();
        cfg.profiles.push(Profile {
            name: name.to_owned(),
            body: ProfileBody::Standard(StandardProfile {
                layers: vec![StandardLayer {
                    path: "/img.png".into(),
                    image_rect_mm: ImageRectMm::default(),
                }],
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        });
        cfg
    }

    #[test]
    fn default_profile_wins_over_resume() {
        let mut cfg = config_with_profile("resumed");
        cfg.general.default_profile = Some("explicit".to_owned());
        let got = choose_initial_profile(&cfg, Some(&resume("resumed")));
        assert_eq!(got.as_deref(), Some("explicit"));
    }

    #[test]
    fn resume_used_when_no_default_profile() {
        let cfg = config_with_profile("resumed");
        let got = choose_initial_profile(&cfg, Some(&resume("resumed")));
        assert_eq!(got.as_deref(), Some("resumed"));
    }

    #[test]
    fn stale_resume_profile_is_dropped() {
        let cfg = config_with_profile("other");
        let got = choose_initial_profile(&cfg, Some(&resume("deleted")));
        assert_eq!(got, None);
    }

    #[test]
    fn no_default_and_no_resume_yields_none() {
        let got = choose_initial_profile(&Config::default(), None);
        assert_eq!(got, None);
    }
}
