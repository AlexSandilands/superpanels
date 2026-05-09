#![forbid(unsafe_code)]

//! Superpanels CLI entry point.

// reason: the CLI is the program's user-facing surface; printing structured
// output to stdout (plain table, JSON) is exactly its job.
#![allow(clippy::print_stdout)]
// reason: friendly user errors and `--debug` diagnostics go to stderr.
#![allow(clippy::print_stderr)]

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use superpanels_core::backends::BackendError;
use superpanels_core::config::{Config, ConfigError};
use superpanels_core::display::kscreen::KscreenDoctorDetector;
use superpanels_core::image::ImageError;
pub(crate) use superpanels_core::ipc::client as ipc_client;
use superpanels_core::ipc::{IpcResponse, socket_path};
use superpanels_core::layout::{FitMode, LayoutError};
use superpanels_core::{DetectError, DisplayDetector, Monitor, Rotation, VERSION, detect};
use tracing_subscriber::EnvFilter;

mod monitor_cmd;
mod profile_cmd;
mod schedule_cmd;
mod set_cmd;

use monitor_cmd::monitor_configure_cmd;
use set_cmd::SetArgs;

#[derive(Parser, Debug)]
#[command(
    name = "superpanels",
    about = "Bezel-aware multi-monitor wallpaper manager",
    version = VERSION,
)]
struct Cli {
    /// Increase logging verbosity (`-v` = debug, `-vv` = trace).
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    /// Suppress non-error output (overrides `--verbose`).
    #[arg(long, global = true)]
    quiet: bool,
    /// Use an alternate config file instead of `$XDG_CONFIG_HOME/superpanels/config.toml`.
    #[arg(long, value_name = "PATH", global = true)]
    config: Option<PathBuf>,
    /// Do not contact the running daemon; run in-process.
    #[arg(long, global = true)]
    no_daemon: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Set wallpaper from one or more image paths (`SPEC.md` §11.1).
    Set {
        /// One or more source image paths. Phase 1 takes a single image.
        #[arg(value_name = "IMAGE", required = true, num_args = 1..)]
        images: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = FitArg::Fill)]
        fit: FitArg,
        /// Accepted but not yet honoured.
        #[arg(long, value_name = "X,Y", value_parser = parse_offset)]
        offset: Option<(i32, i32)>,
        #[arg(long, value_name = "NAME")]
        backend: Option<String>,
        /// Manual monitor spec (`SPEC.md` §6.2).
        #[arg(long, value_name = "SPEC")]
        monitors: Option<String>,
        /// Pin an image to a monitor.
        #[arg(long = "monitor", value_name = "NAME=PATH", action = clap::ArgAction::Append)]
        monitor: Vec<String>,
        /// Process image but don't apply; print what would happen.
        #[arg(long)]
        dry_run: bool,
        /// Save the current settings as a named profile before applying.
        #[arg(long, value_name = "NAME")]
        save_as: Option<String>,
    },
    /// Advance the slideshow to the next wallpaper (requires a running daemon).
    Next,
    /// Step back to the previous wallpaper (requires a running daemon).
    Prev,
    /// Pause the slideshow timer (requires a running daemon).
    Pause,
    /// Resume the slideshow timer (requires a running daemon).
    Resume,
    /// Manage wallpaper profiles (`SPEC.md` §11.2).
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Print the detected monitor layout and exit.
    Detect {
        #[arg(long)]
        json: bool,
        /// Print which detectors were tried and their availability reason.
        #[arg(long)]
        debug: bool,
    },
    /// Print the resolved configuration as TOML.
    Config,
    /// Manage monitor-config blocks in `$XDG_CONFIG_HOME/superpanels/config.toml`.
    Monitor {
        #[command(subcommand)]
        action: MonitorAction,
    },
    /// Manage schedule rules.
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

#[derive(Subcommand, Debug)]
enum ProfileAction {
    /// List all profile names.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Apply a profile immediately.
    Apply { name: String },
    /// Delete a profile from the config.
    Delete { name: String },
    /// Rename a profile.
    Rename { old: String, new_name: String },
    /// Export a profile to a portable TOML bundle.
    Export {
        name: String,
        /// Write to file instead of stdout.
        #[arg(short = 'o', value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Import profiles from a bundle file.
    Import { file: PathBuf },
    /// Show a profile in detail (TOML).
    Show { name: String },
    /// Duplicate a profile under a new name.
    Duplicate { name: String, new_name: String },
}

#[derive(Subcommand, Debug)]
enum ScheduleAction {
    /// List schedule rules.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Add a daily rule. Time is `HH:MM` 24-hour.
    Add {
        profile: String,
        #[arg(long, value_name = "HH:MM")]
        daily: Option<String>,
    },
    /// Remove a rule by 1-based index.
    Remove { index: usize },
    /// Enable a rule by index.
    Enable { index: usize },
    /// Disable a rule by index.
    Disable { index: usize },
    /// Pause all schedules.
    Pause,
    /// Resume schedules.
    Resume,
}

#[derive(Subcommand, Debug)]
enum MonitorAction {
    /// Add or update a [[monitor]] block. Pass exactly one of `--mm` or `--diagonal`.
    Configure {
        /// Output name (`DP-1`) or stable id; flag below selects which.
        identifier: String,
        #[arg(long)]
        stable_id: bool,
        /// Direct mm, e.g. `597x336`.
        #[arg(long, value_name = "WxH", conflicts_with_all = ["diagonal", "aspect"])]
        mm: Option<String>,
        /// Panel diagonal, e.g. `27in`.
        #[arg(long, value_name = "DIAGONAL", requires = "aspect")]
        diagonal: Option<String>,
        /// Aspect ratio, e.g. `16:9`.
        #[arg(long, value_name = "W:H", requires = "diagonal")]
        aspect: Option<String>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum FitArg {
    Fill,
    Fit,
    Stretch,
    Center,
}

impl From<FitArg> for FitMode {
    fn from(v: FitArg) -> Self {
        match v {
            FitArg::Fill => Self::Fill,
            FitArg::Fit => Self::Fit,
            FitArg::Stretch => Self::Stretch,
            FitArg::Center => Self::Center,
        }
    }
}

/// Sentinel for IPC/daemon errors — maps to exit code 7 (`SPEC.md` §11.6).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct IpcError(pub String);

fn parse_offset(s: &str) -> Result<(i32, i32), String> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("expected `X,Y`, got `{s}`"))?;
    let x: i32 = x.trim().parse().map_err(|e| format!("bad X: {e}"))?;
    let y: i32 = y.trim().parse().map_err(|e| format!("bad Y: {e}"))?;
    Ok((x, y))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet);
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !cli.quiet {
                eprintln!("error: {e:#}");
            }
            ExitCode::from(map_exit_code(&e))
        }
    }
}

fn init_tracing(verbose: u8, quiet: bool) {
    // RUST_LOG wins when set; otherwise map -v / --quiet to a level (`SPEC.md` §11).
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = if quiet {
            "error"
        } else {
            match verbose {
                0 => "warn",
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

/// Map an `anyhow::Error` to its `SPEC.md` §11.6 exit code. Clap argument
/// errors short-circuit in `Cli::parse()` and never reach here.
fn map_exit_code(err: &anyhow::Error) -> u8 {
    for cause in err.chain() {
        if cause.is::<ConfigError>() {
            return 3;
        }
        if let Some(be) = cause.downcast_ref::<BackendError>() {
            return if matches!(be, BackendError::Unavailable { .. }) {
                4
            } else {
                1
            };
        }
        if cause.is::<DetectError>() {
            return 5;
        }
        if cause.is::<LayoutError>() {
            // Shares code 5 with detection: both mean "layout unknown; run `monitor configure`".
            return 5;
        }
        if cause.is::<ImageError>() {
            return 6;
        }
        if cause.is::<IpcError>() {
            return 7;
        }
    }
    1
}

/// Try to connect to a running daemon. Returns `None` if no daemon is found.
fn try_ipc() -> Option<UnixStream> {
    ipc_client::try_connect(&socket_path())
}

/// Connect to the daemon or return an `IpcError` (exit code 7).
fn require_daemon() -> Result<UnixStream> {
    try_ipc().ok_or_else(|| {
        anyhow::anyhow!(IpcError(
            "no daemon is running — start one with `superpanels daemon`".to_owned()
        ))
    })
}

/// Return exit code 7 for a daemon-returned error string.
fn ipc_err(resp: IpcResponse) -> Result<()> {
    if resp.is_ok() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(IpcError(
            resp.error
                .unwrap_or_else(|| "daemon returned error".to_owned())
        )))
    }
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Set {
            images,
            fit,
            offset,
            backend,
            monitors,
            monitor,
            dry_run,
            save_as,
        } => {
            let mut images = images.clone();
            let first = images.remove(0);
            let args = SetArgs {
                image: first,
                extra_images: images,
                fit: (*fit).into(),
                offset: *offset,
                backend: backend.clone(),
                monitors: monitors.clone(),
                pins: monitor.clone(),
                dry_run: *dry_run,
                save_as: save_as.clone(),
            };
            // Forward to daemon unless dry-run, manual --monitors, or --no-daemon.
            if !dry_run && monitors.is_none() && !cli.no_daemon {
                if let Some(mut stream) = try_ipc() {
                    return set_cmd::run_via_ipc(&args, cli.config.as_deref(), &mut stream);
                }
            }
            set_cmd::run(&args, cli.config.as_deref(), None)
        }
        Command::Next => slideshow_cmd(
            "slideshow_next",
            serde_json::Value::Null,
            "Advanced to next wallpaper.",
        ),
        Command::Prev => slideshow_cmd(
            "slideshow_prev",
            serde_json::Value::Null,
            "Stepped back to previous wallpaper.",
        ),
        Command::Pause => slideshow_cmd(
            "slideshow_pause",
            serde_json::json!({"paused": true}),
            "Slideshow paused.",
        ),
        Command::Resume => slideshow_cmd(
            "slideshow_pause",
            serde_json::json!({"paused": false}),
            "Slideshow resumed.",
        ),
        Command::Profile { action } => profile_action(action, cli),
        Command::Detect { json, debug } => detect_cmd(*json, *debug, cli.config.as_deref()),
        Command::Config => config_cmd(cli.config.as_deref()),
        Command::Monitor { action } => match action {
            MonitorAction::Configure {
                identifier,
                stable_id,
                mm,
                diagonal,
                aspect,
            } => monitor_configure_cmd(
                identifier,
                *stable_id,
                mm.as_deref(),
                diagonal.as_deref(),
                aspect.as_deref(),
            ),
        },
        Command::Schedule { action } => schedule_action(action, cli),
    }
}

/// Dispatch slideshow control commands — all require a running daemon.
fn slideshow_cmd(method: &str, params: serde_json::Value, success_msg: &str) -> Result<()> {
    let mut stream = require_daemon()?;
    let resp = ipc_client::call(&mut stream, method, params)?;
    ipc_err(resp)?;
    println!("{success_msg}");
    Ok(())
}

fn profile_action(action: &ProfileAction, cli: &Cli) -> Result<()> {
    let cfg = cli.config.as_deref();
    match action {
        ProfileAction::List { json } => profile_cmd::list_cmd(*json, cfg),
        ProfileAction::Apply { name } => {
            let mut ipc = if cli.no_daemon { None } else { try_ipc() };
            profile_cmd::apply_cmd(name, cfg, ipc.as_mut())
        }
        ProfileAction::Delete { name } => profile_cmd::delete_cmd(name, cfg),
        ProfileAction::Rename { old, new_name } => profile_cmd::rename_cmd(old, new_name, cfg),
        ProfileAction::Export { name, output } => {
            profile_cmd::export_cmd(name, output.as_deref(), cfg)
        }
        ProfileAction::Import { file } => profile_cmd::import_cmd(file, cfg),
        ProfileAction::Show { name } => profile_cmd::show_cmd(name, cfg),
        ProfileAction::Duplicate { name, new_name } => {
            profile_cmd::duplicate_cmd(name, new_name, cfg)
        }
    }
}

fn schedule_action(action: &ScheduleAction, cli: &Cli) -> Result<()> {
    let cfg_path = cli.config.as_deref();
    match action {
        ScheduleAction::List { json } => schedule_cmd::list(*json, cfg_path),
        ScheduleAction::Add { profile, daily } => {
            schedule_cmd::add(profile, daily.as_deref(), cfg_path)
        }
        ScheduleAction::Remove { index } => schedule_cmd::remove(*index, cfg_path),
        ScheduleAction::Enable { index } => schedule_cmd::set_enabled(*index, true, cfg_path),
        ScheduleAction::Disable { index } => schedule_cmd::set_enabled(*index, false, cfg_path),
        ScheduleAction::Pause => schedule_cmd::set_paused(true, cfg_path),
        ScheduleAction::Resume => schedule_cmd::set_paused(false, cfg_path),
    }
}

fn config_cmd(config_path: Option<&std::path::Path>) -> Result<()> {
    let cfg = match config_path {
        Some(p) => Config::load_from(p),
        None => Config::load_or_default(),
    }?;
    let toml_text =
        toml::to_string_pretty(&cfg).map_err(|e| ConfigError::Serialise(e.to_string()))?;
    println!("{toml_text}");
    Ok(())
}

fn detect_cmd(json: bool, debug: bool, config_path: Option<&Path>) -> Result<()> {
    if debug {
        print_debug_attempts();
    }
    let mut monitors = detect(None).context("detecting monitors")?;
    let cfg = match config_path {
        Some(p) => Config::load_from(p)?,
        None => Config::load_or_default()?,
    };
    cfg.merge_into_monitors(&mut monitors);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &monitors).context("writing JSON")?;
        writeln!(out).ok();
    } else {
        write_table(&mut out, &monitors).context("writing detect table")?;
    }
    Ok(())
}

fn print_debug_attempts() {
    let kscreen = KscreenDoctorDetector;
    eprintln!(
        "detector {name}: availability = {avail:?}",
        name = kscreen.name(),
        avail = kscreen.availability(),
    );
}

fn write_table<W: Write>(out: &mut W, monitors: &[Monitor]) -> std::io::Result<()> {
    for (i, m) in monitors.iter().enumerate() {
        let physical = match m.physical_size_mm {
            Some((w, h)) => {
                let ppi = m.ppi.map(|p| format!("  {p:.0} PPI")).unwrap_or_default();
                format!("{w:.1}x{h:.1}mm{ppi}")
            }
            None => "(no physical size — run `monitor configure`)".to_owned(),
        };
        let rotation = match m.rotation {
            Rotation::None => "none",
            Rotation::Right => "right",
            Rotation::Inverted => "inverted",
            Rotation::Left => "left",
        };
        writeln!(
            out,
            "Monitor {i}: {name}  {w}x{h} at ({x},{y})  scale {scale}  rotation: {rotation}  {physical}",
            name = m.name,
            w = m.resolution.0,
            h = m.resolution.1,
            x = m.position.0,
            y = m.position.1,
            scale = m.scale,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests assert on Ok variant; failure is a test bug
mod tests {
    use super::*;
    use superpanels_core::backends::BackendError;

    // parse_offset

    #[test]
    fn parse_offset_happy_path_returns_pair() {
        // Act
        let got = parse_offset("100,-50").unwrap();

        // Assert
        assert_eq!(got, (100, -50));
    }

    #[test]
    fn parse_offset_missing_comma_is_rejected() {
        // Act
        let result = parse_offset("100 50");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_offset_non_numeric_is_rejected() {
        // Act
        let result = parse_offset("a,b");

        // Assert
        assert!(result.is_err());
    }

    // map_exit_code

    #[test]
    fn map_exit_code_config_error_is_three() {
        // Arrange
        let err = anyhow::Error::new(ConfigError::NoConfigDir);

        // Act + Assert
        assert_eq!(map_exit_code(&err), 3);
    }

    #[test]
    fn map_exit_code_backend_unavailable_is_four() {
        // Arrange
        let err = anyhow::Error::new(BackendError::Unavailable {
            backend: "kde",
            reason: "wrong env".to_owned(),
        });

        // Act + Assert
        assert_eq!(map_exit_code(&err), 4);
    }

    #[test]
    fn map_exit_code_detect_error_is_five() {
        // Arrange
        let err = anyhow::Error::new(DetectError::EmptyResult);

        // Act + Assert
        assert_eq!(map_exit_code(&err), 5);
    }

    #[test]
    fn map_exit_code_layout_error_is_five() {
        // Arrange
        let err = anyhow::Error::new(LayoutError::EmptyMonitorList);

        // Act + Assert
        assert_eq!(map_exit_code(&err), 5);
    }

    #[test]
    fn map_exit_code_image_error_is_six() {
        // Arrange
        let err = anyhow::Error::new(ImageError::NoCacheDir);

        // Act + Assert
        assert_eq!(map_exit_code(&err), 6);
    }

    #[test]
    fn map_exit_code_ipc_error_is_seven() {
        // Arrange
        let err = anyhow::Error::new(IpcError("no daemon".to_owned()));

        // Act + Assert
        assert_eq!(map_exit_code(&err), 7);
    }

    #[test]
    fn map_exit_code_unknown_error_is_one() {
        // Arrange
        let err = anyhow::anyhow!("something unspecified");

        // Act + Assert
        assert_eq!(map_exit_code(&err), 1);
    }
}
