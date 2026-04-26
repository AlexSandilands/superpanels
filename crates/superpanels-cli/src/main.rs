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
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use superpanels_core::config::{Config, MonitorIdentifier, diagonal_to_mm, write_monitor_block};
use superpanels_core::display::kscreen::KscreenDoctorDetector;
use superpanels_core::{DisplayDetector, Monitor, Rotation, VERSION, detect};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "superpanels",
    about = "Bezel-aware multi-monitor wallpaper manager",
    version = VERSION,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the detected monitor layout and exit.
    Detect {
        /// Emit Vec<Monitor> as JSON instead of a human-readable table.
        #[arg(long)]
        json: bool,
        /// Print which detectors were tried and their availability reason.
        #[arg(long)]
        debug: bool,
    },
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

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            // SPEC §11.6 exit code 5 is "display detection failure"; this
            // grows when more subcommands arrive.
            ExitCode::from(5)
        }
    }
}

fn init_tracing() {
    // RUST_LOG controls verbosity; default is silent so scripted JSON stays
    // clean.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Detect { json, debug } => detect_cmd(*json, *debug),
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

fn monitor_configure_cmd(
    identifier: &str,
    stable_id: bool,
    mm: Option<&str>,
    diagonal: Option<&str>,
    aspect: Option<&str>,
) -> Result<()> {
    let physical_mm = match (mm, diagonal, aspect) {
        (Some(s), None, None) => parse_mm(s)?,
        (None, Some(d), Some(a)) => {
            let inches = parse_diagonal(d)?;
            let (aw, ah) = parse_aspect(a)?;
            diagonal_to_mm(inches, aw, ah)
        }
        (None, None, None) => bail!("provide one of `--mm WxH` or `--diagonal D --aspect W:H`"),
        _ => bail!("`--mm` is mutually exclusive with `--diagonal`/`--aspect`"),
    };

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
