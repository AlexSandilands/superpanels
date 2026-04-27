#![forbid(unsafe_code)]

//! Superpanels CLI entry point.

// reason: the CLI is the program's user-facing surface; printing structured
// output to stdout (plain table, JSON) is exactly its job.
#![allow(clippy::print_stdout)]
// reason: friendly user errors and `--debug` diagnostics go to stderr.
#![allow(clippy::print_stderr)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use superpanels_core::config::{Config, ConfigError};
use superpanels_core::display::kscreen::KscreenDoctorDetector;
use superpanels_core::image::ImageError;
use superpanels_core::layout::{FitMode, LayoutError};
use superpanels_core::{DetectError, DisplayDetector, Monitor, Rotation, VERSION, detect};
use tracing_subscriber::EnvFilter;

mod monitor_cmd;
mod set_cmd;

use monitor_cmd::monitor_configure_cmd;
use set_cmd::{BackendUnavailable, SetArgs};

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
    /// Do not contact the running daemon; run in-process. (No-op until Phase 2.)
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
        /// Horizontal gap between monitors (mm).
        #[arg(long, value_name = "MM")]
        bezel_h: Option<f32>,
        /// Vertical gap between monitors (mm).
        #[arg(long, value_name = "MM")]
        bezel_v: Option<f32>,
        #[arg(long, value_enum, default_value_t = FitArg::Fill)]
        fit: FitArg,
        /// Accepted in Phase 1 but not yet honoured.
        #[arg(long, value_name = "X,Y", value_parser = parse_offset)]
        offset: Option<(i32, i32)>,
        #[arg(long, value_name = "NAME")]
        backend: Option<String>,
        /// Manual monitor spec (`SPEC.md` §6.2).
        #[arg(long, value_name = "SPEC")]
        monitors: Option<String>,
        /// Pin an image to a monitor. Phase 2 feature.
        #[arg(long = "monitor", value_name = "NAME=PATH", action = clap::ArgAction::Append)]
        monitor: Vec<String>,
        /// Print computed crop specs as JSON; do not apply.
        #[arg(long)]
        dry_run: bool,
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
        if cause.is::<BackendUnavailable>() {
            return 4;
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
    }
    1
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Set {
            images,
            bezel_h,
            bezel_v,
            fit,
            offset,
            backend,
            monitors,
            monitor,
            dry_run,
        } => {
            let mut images = images.clone();
            let first = images.remove(0);
            let args = SetArgs {
                image: first,
                extra_images: images,
                bezel_h: *bezel_h,
                bezel_v: *bezel_v,
                fit: (*fit).into(),
                offset: *offset,
                backend: backend.clone(),
                monitors: monitors.clone(),
                pins: monitor.clone(),
                dry_run: *dry_run,
            };
            set_cmd::run(&args, cli.config.as_deref(), None)
        }
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
                format!("{w}x{h}mm{ppi}")
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
        let err = anyhow::Error::new(BackendUnavailable {
            backend: "kde",
            detail: "wrong env".to_owned(),
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
    fn map_exit_code_unknown_error_is_one() {
        // Arrange
        let err = anyhow::anyhow!("something unspecified");

        // Act + Assert
        assert_eq!(map_exit_code(&err), 1);
    }
}
