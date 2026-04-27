#![forbid(unsafe_code)]

//! Superpanels background daemon (`SPEC.md` §5.1, §5.2, §5.3; `PLAN.md` §2.5).

// reason: tracing replaces println for structured output; the daemon has no
// intentional stdout output. Print suppression warnings are correct.
#![allow(clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use superpanels_core::ipc::socket_path;
use superpanels_core::library::persist_index;
use superpanels_core::slideshow::persist_state as persist_slideshow;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, watch};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

mod apply;
mod pool;
mod schedule;
mod server;
mod state;
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

    // If a default profile is configured, restore its slideshow picker from disk
    // so history and sort order survive a restart, then apply it at startup.
    let initial_profile = daemon_state.config.general.default_profile.clone();
    if let Some(ref name) = initial_profile {
        if let Some(dir) = DaemonState::state_dir() {
            daemon_state.restore_slideshow(name, &dir.join("slideshow-state.json"));
        }
    }
    let state = Arc::new(Mutex::new(daemon_state));

    let (timer_tx, timer_rx) = watch::channel::<Option<Duration>>(None);
    let (watcher_tx, watcher_rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();

    // Start FS watcher if library roots are configured.
    let roots: Vec<PathBuf> = {
        let guard = state.lock().await;
        guard.config.library.roots.clone()
    };
    let _watcher = if roots.is_empty() {
        None
    } else {
        match watcher::make_watcher(&roots, watcher_tx) {
            Ok(w) => Some(w),
            Err(e) => {
                warn!(error = %e, "FS watcher could not be initialised; library will not auto-update");
                None
            }
        }
    };

    // Spawn background tasks.
    let timer_state = Arc::clone(&state);
    tokio::spawn(async move { timer::run_timer(timer_state, timer_rx).await });

    let watcher_state = Arc::clone(&state);
    tokio::spawn(async move { watcher::run_watcher(watcher_state, watcher_rx).await });

    let sched_state = Arc::clone(&state);
    let sched_timer_tx = timer_tx.clone();
    tokio::spawn(async move { schedule::run_schedule_checker(sched_state, sched_timer_tx).await });

    // Apply the default profile (if set) after a short delay to allow compositor readiness.
    if let Some(profile_name) = initial_profile {
        let state_clone = Arc::clone(&state);
        let timer_tx_clone = timer_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let req = superpanels_core::ipc::IpcRequest {
                v: superpanels_core::ipc::PROTOCOL_VERSION,
                method: "apply_profile".to_owned(),
                params: serde_json::json!({"name": profile_name}),
            };
            let resp = server::dispatch_for_tests(req, state_clone, timer_tx_clone).await;
            if !resp.is_ok() {
                warn!(error = ?resp.error, "default profile apply on startup failed");
            }
        });
    }

    // Main accept loop — runs until SIGTERM.
    let server_state = Arc::clone(&state);
    let server_handle = tokio::spawn(async move {
        server::run_server(listener, server_state, timer_tx).await;
    });

    // Wait for SIGTERM or SIGINT and persist state before exit.
    wait_for_shutdown().await;
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
    // Persist library index.
    let index_path = dir.join("library-index.json");
    if let Err(e) = persist_index(&guard.library, &index_path) {
        warn!(error = %e, "failed to persist library index");
    }
    // Persist slideshow state.
    if let Some(picker) = &guard.slideshow_picker {
        let state_path = dir.join("slideshow-state.json");
        if let Err(e) = persist_slideshow(picker.state(), &state_path) {
            warn!(error = %e, "failed to persist slideshow state");
        }
    }
}

async fn wait_for_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "SIGTERM handler could not be registered; only Ctrl-C will stop the daemon");
            // Fall back to waiting forever (SIGKILL will terminate us).
            std::future::pending::<()>().await;
            return;
        }
    };
    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "SIGINT handler could not be registered");
            std::future::pending::<()>().await;
            return;
        }
    };
    tokio::select! {
        _ = sigterm.recv() => info!("received SIGTERM"),
        _ = sigint.recv()  => info!("received SIGINT"),
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

/// Re-exec the daemon in the foreground via a child process.
fn daemonize(cli: &Cli) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current executable")?;
    let mut cmd = std::process::Command::new(&exe);
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
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning background daemon process")?;
    info!(socket = ?cli.socket, "daemon started in background");
    Ok(())
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
