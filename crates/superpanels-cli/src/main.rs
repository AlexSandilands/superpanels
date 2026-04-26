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

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
    match cli.command {
        Command::Detect { json, debug } => detect_cmd(json, debug),
    }
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
