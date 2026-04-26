#![forbid(unsafe_code)]

//! Superpanels CLI entry point.
//!
//! Subcommand dispatch is the CLI's only job — every command body is a thin
//! wrapper around `superpanels-core` (`docs/architecture.md`).

// reason: the CLI is the program's user-facing surface; printing structured
// output to stdout (plain table, JSON) is exactly its job.
#![allow(clippy::print_stdout)]
// reason: friendly user errors and `--debug` diagnostics go to stderr.
#![allow(clippy::print_stderr)]

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use superpanels_core::config::{
    Config, ConfigError, MonitorIdentifier, diagonal_to_mm, write_monitor_block,
};
use superpanels_core::display::kscreen::KscreenDoctorDetector;
use superpanels_core::image::ImageError;
use superpanels_core::layout::{FitMode, LayoutError};
use superpanels_core::{DetectError, DisplayDetector, Monitor, Rotation, VERSION, detect};
use tracing_subscriber::EnvFilter;

mod set_cmd;

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
    /// Do not contact the running daemon; run in-process. (No-op in Phase 1
    /// — the daemon arrives in Phase 2 — but accepted so scripts written
    /// against the spec do not error.)
    #[arg(long, global = true)]
    no_daemon: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Set wallpaper from one or more image paths (`SPEC.md` §11.1).
    Set {
        /// One or more source image paths. Phase 1 supports a single image
        /// (spanned across all monitors with bezel correction); multi-image
        /// per-monitor mode arrives in Phase 2.
        #[arg(value_name = "IMAGE", required = true, num_args = 1..)]
        images: Vec<PathBuf>,
        /// Horizontal gap between monitors (mm).
        #[arg(long, value_name = "MM")]
        bezel_h: Option<f32>,
        /// Vertical gap between monitors (mm).
        #[arg(long, value_name = "MM")]
        bezel_v: Option<f32>,
        /// How to fit the source image to the canvas.
        #[arg(long, value_enum, default_value_t = FitArg::Fill)]
        fit: FitArg,
        /// Image offset within the canvas (signed pixels). Accepted in
        /// Phase 1 but not yet honoured.
        #[arg(long, value_name = "X,Y", value_parser = parse_offset)]
        offset: Option<(i32, i32)>,
        /// Override backend detection. Phase 1: only `kde` is implemented.
        #[arg(long, value_name = "NAME")]
        backend: Option<String>,
        /// Manual monitor spec (see `SPEC.md` §6.2).
        #[arg(long, value_name = "SPEC")]
        monitors: Option<String>,
        /// Pin a specific image to a specific monitor. Repeatable. Phase 2
        /// feature; Phase 1 returns a friendly "not yet supported".
        #[arg(long = "monitor", value_name = "NAME=PATH", action = clap::ArgAction::Append)]
        monitor: Vec<String>,
        /// Process the image but don't apply; print the computed crop
        /// specs (and the resolved monitor / bezel context) as JSON.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the detected monitor layout and exit.
    Detect {
        /// Emit Vec<Monitor> as JSON instead of a human-readable table.
        #[arg(long)]
        json: bool,
        /// Print which detectors were tried and their availability reason.
        #[arg(long)]
        debug: bool,
    },
    /// Print the resolved configuration as TOML (debug aid).
    Config,
    /// Manage monitor-config blocks in `$XDG_CONFIG_HOME/superpanels/config.toml`.
    Monitor {
        #[command(subcommand)]
        action: MonitorAction,
    },
}

#[derive(Subcommand, Debug)]
enum MonitorAction {
    /// Add or update a [[monitor]] block giving the named monitor its
    /// physical millimetres. Existing comments and unrelated blocks are
    /// preserved. Pass exactly one of `--mm` or `--diagonal`.
    Configure {
        /// Output name (`DP-1`) or stable id; the value chooses which
        /// field the [[monitor]] block keys on.
        identifier: String,
        /// Treat `IDENTIFIER` as a stable id (KDE per-output UUID, EDID
        /// hash) instead of a name.
        #[arg(long)]
        stable_id: bool,
        /// Direct mm: e.g. `597x336`.
        #[arg(long, value_name = "WxH", conflicts_with_all = ["diagonal", "aspect"])]
        mm: Option<String>,
        /// Panel diagonal, e.g. `27in` (`in` suffix optional).
        #[arg(long, value_name = "DIAGONAL", requires = "aspect")]
        diagonal: Option<String>,
        /// Aspect ratio, e.g. `16:9`. Required with `--diagonal`.
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
    // RUST_LOG wins when set so the user can always force it. Otherwise,
    // map the verbosity flags to a sensible default level per `SPEC.md` §11.
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

/// Map an `anyhow::Error` to its `SPEC.md` §11.6 exit code by inspecting
/// the underlying typed error chain.
///
/// Clap argument errors (`ErrorKind::Format`, `MissingRequiredArgument`,
/// …) never reach this function — clap handles them in `Cli::parse()` and
/// exits with code 2 itself, matching `SPEC.md` §11.6 entry "2: Bad
/// arguments".
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
            // PhysicalSizeMissing is the user-actionable case the brief
            // calls out — it shares exit code 5 with detection failures
            // because both indicate "we don't know the layout well enough
            // to apply", and the user fixes both via `monitor configure`.
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
            set_cmd::run(&args, cli.config.as_deref())
        }
        Command::Detect { json, debug } => detect_cmd(*json, *debug),
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

fn monitor_configure_cmd(
    identifier: &str,
    stable_id: bool,
    mm: Option<&str>,
    diagonal: Option<&str>,
    aspect: Option<&str>,
) -> Result<()> {
    let physical_mm = resolve_physical_mm(mm, diagonal, aspect)?;

    let id = if stable_id {
        MonitorIdentifier::StableId(identifier.to_owned())
    } else {
        MonitorIdentifier::Name(identifier.to_owned())
    };

    let path = Config::default_path().context("locating config path")?;
    write_monitor_block(&path, &id, physical_mm).context("writing monitor block")?;
    println!(
        "wrote {field} = \"{identifier}\", physical_mm = [{w}, {h}] to {p}",
        field = if stable_id { "stable_id" } else { "name" },
        w = physical_mm[0],
        h = physical_mm[1],
        p = path.display(),
    );
    Ok(())
}

/// Resolve the four valid `(--mm, --diagonal, --aspect)` combinations into
/// a single `[w_mm, h_mm]` pair, or surface the user-facing error for an
/// invalid combination.
///
/// Pure helper extracted from [`monitor_configure_cmd`] so each arm of the
/// `match` is unit-testable without touching disk or env.
fn resolve_physical_mm(
    mm: Option<&str>,
    diagonal: Option<&str>,
    aspect: Option<&str>,
) -> Result<[u32; 2]> {
    match (mm, diagonal, aspect) {
        (Some(s), None, None) => parse_mm(s),
        (None, Some(d), Some(a)) => {
            let inches = parse_diagonal(d)?;
            let (aw, ah) = parse_aspect(a)?;
            Ok(diagonal_to_mm(inches, aw, ah))
        }
        (None, None, None) => bail!("provide one of `--mm WxH` or `--diagonal D --aspect W:H`"),
        _ => bail!("`--mm` is mutually exclusive with `--diagonal`/`--aspect`"),
    }
}

fn parse_mm(s: &str) -> Result<[u32; 2]> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| anyhow!("expected `WxH`, got `{s}`"))?;
    let w: u32 = w.parse().context("parsing mm width")?;
    let h: u32 = h.parse().context("parsing mm height")?;
    if w == 0 || h == 0 {
        bail!("mm values must be > 0");
    }
    Ok([w, h])
}

fn parse_diagonal(s: &str) -> Result<f64> {
    let stripped = s
        .strip_suffix("in")
        .or_else(|| s.strip_suffix("\""))
        .unwrap_or(s);
    let v: f64 = stripped
        .parse()
        .with_context(|| format!("parsing diagonal `{s}`"))?;
    if v <= 0.0 || !v.is_finite() {
        bail!("diagonal must be > 0");
    }
    Ok(v)
}

fn parse_aspect(s: &str) -> Result<(u32, u32)> {
    let (w, h) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("expected `W:H`, got `{s}`"))?;
    let w: u32 = w.parse().context("parsing aspect width")?;
    let h: u32 = h.parse().context("parsing aspect height")?;
    if w == 0 || h == 0 {
        bail!("aspect components must be > 0");
    }
    Ok((w, h))
}

fn detect_cmd(json: bool, debug: bool) -> Result<()> {
    if debug {
        print_debug_attempts();
    }
    let monitors = detect(None).context("detecting monitors")?;
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
            Some((w, h)) => format!("{w}x{h}mm"),
            None => "(no physical size configured)".to_owned(),
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

    // parse_mm

    #[test]
    fn parse_mm_happy_path_returns_pair() {
        // Arrange / Act
        let got = parse_mm("597x336").unwrap();

        // Assert
        assert_eq!(got, [597, 336]);
    }

    #[test]
    fn parse_mm_missing_separator_returns_error() {
        // Act
        let result = parse_mm("597-336");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_mm_non_numeric_returns_error() {
        // Act
        let result = parse_mm("abcxdef");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_mm_zero_is_rejected() {
        // Act
        let result = parse_mm("0x336");

        // Assert
        assert!(result.is_err());
    }

    // parse_diagonal

    #[test]
    fn parse_diagonal_bare_number_parses() {
        // Act
        let got = parse_diagonal("27").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_with_in_suffix_parses() {
        // Act
        let got = parse_diagonal("27in").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_with_inch_quote_suffix_parses() {
        // Act
        let got = parse_diagonal("27\"").unwrap();

        // Assert
        assert!((got - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_diagonal_zero_is_rejected() {
        // Act
        let result = parse_diagonal("0");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_diagonal_negative_is_rejected() {
        // Act
        let result = parse_diagonal("-27");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_diagonal_non_finite_is_rejected() {
        // Act
        let result = parse_diagonal("inf");

        // Assert
        assert!(result.is_err());
    }

    // parse_aspect

    #[test]
    fn parse_aspect_happy_path_returns_pair() {
        // Act
        let got = parse_aspect("16:9").unwrap();

        // Assert
        assert_eq!(got, (16, 9));
    }

    #[test]
    fn parse_aspect_missing_separator_returns_error() {
        // Act
        let result = parse_aspect("16x9");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn parse_aspect_zero_denominator_is_rejected() {
        // Act
        let result = parse_aspect("16:0");

        // Assert
        assert!(result.is_err());
    }

    // resolve_physical_mm — all four match arms

    #[test]
    fn resolve_physical_mm_arm_mm_only_returns_parsed_pair() {
        // Act
        let got = resolve_physical_mm(Some("597x336"), None, None).unwrap();

        // Assert
        assert_eq!(got, [597, 336]);
    }

    #[test]
    fn resolve_physical_mm_arm_diagonal_and_aspect_returns_computed_pair() {
        // Arrange — 27" 16:9 → ~597x336 mm; allow generous tolerance.
        // Act
        let got = resolve_physical_mm(None, Some("27in"), Some("16:9")).unwrap();

        // Assert
        assert!(
            (590..=605).contains(&got[0]),
            "width was {}, expected ~597",
            got[0]
        );
        assert!(
            (330..=345).contains(&got[1]),
            "height was {}, expected ~336",
            got[1]
        );
    }

    #[test]
    fn resolve_physical_mm_arm_neither_returns_help_error() {
        // Act
        let result = resolve_physical_mm(None, None, None);

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--mm"), "msg was {err}");
        assert!(err.contains("--diagonal"), "msg was {err}");
    }

    #[test]
    fn resolve_physical_mm_arm_mm_with_diagonal_returns_mutex_error() {
        // Act
        let result = resolve_physical_mm(Some("597x336"), Some("27"), None);

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"), "msg was {err}");
    }

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
