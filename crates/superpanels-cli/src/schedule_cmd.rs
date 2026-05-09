//! `superpanels schedule` subcommand implementation. Talks to the on-disk
//! config; the daemon picks the changes up via its config watcher.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use superpanels_core::config::Config;
use superpanels_core::schedule::{Schedule, Trigger, validate_trigger};

fn resolve_path(path: Option<&Path>) -> Result<PathBuf> {
    Ok(match path {
        Some(p) => p.to_owned(),
        None => Config::default_path()?,
    })
}

pub(crate) fn list(json: bool, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_path(config_path)?;
    let cfg = Config::load_or_default_from(&cfg_path)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &cfg.schedules)?;
        writeln!(out).ok();
        return Ok(());
    }
    if cfg.schedules.is_empty() {
        writeln!(out, "(no schedules)")?;
        return Ok(());
    }
    for (i, r) in cfg.schedules.iter().enumerate() {
        writeln!(
            out,
            "{:>2}. {}  →  {}  [{}]",
            i + 1,
            describe(&r.trigger),
            r.profile,
            if r.enabled { "enabled" } else { "disabled" },
        )?;
    }
    if cfg.schedules_paused {
        writeln!(out, "\nschedules are PAUSED.")?;
    }
    Ok(())
}

pub(crate) fn add(profile: &str, daily: Option<&str>, config_path: Option<&Path>) -> Result<()> {
    let trigger = if let Some(s) = daily {
        let (h, m) = s
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("expected HH:MM"))?;
        Trigger::Daily {
            hour: h.trim().parse().context("parsing hour")?,
            minute: m.trim().parse().context("parsing minute")?,
        }
    } else {
        bail!("at minimum, pass --daily HH:MM");
    };
    validate_trigger(&trigger)?;
    let cfg_path = resolve_path(config_path)?;
    let mut cfg = Config::load_or_default_from(&cfg_path)?;
    if !cfg.profiles.iter().any(|p| p.name == profile) {
        bail!("unknown profile '{profile}'");
    }
    cfg.schedules.push(Schedule {
        display_name: None,
        profile: profile.to_owned(),
        trigger,
        enabled: true,
    });
    cfg.save_to(&cfg_path)?;
    println!("Added rule. {} total.", cfg.schedules.len());
    Ok(())
}

pub(crate) fn remove(index: usize, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_path(config_path)?;
    let mut cfg = Config::load_or_default_from(&cfg_path)?;
    if index == 0 || index > cfg.schedules.len() {
        bail!("index out of range");
    }
    cfg.schedules.remove(index - 1);
    cfg.save_to(&cfg_path)?;
    println!("Removed rule {index}.");
    Ok(())
}

pub(crate) fn set_enabled(index: usize, enabled: bool, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_path(config_path)?;
    let mut cfg = Config::load_or_default_from(&cfg_path)?;
    if index == 0 || index > cfg.schedules.len() {
        bail!("index out of range");
    }
    cfg.schedules[index - 1].enabled = enabled;
    cfg.save_to(&cfg_path)?;
    println!(
        "Rule {index} {}.",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

pub(crate) fn set_paused(paused: bool, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_path(config_path)?;
    let mut cfg = Config::load_or_default_from(&cfg_path)?;
    cfg.schedules_paused = paused;
    cfg.save_to(&cfg_path)?;
    println!("Schedules {}.", if paused { "paused" } else { "resumed" });
    Ok(())
}

fn describe(t: &Trigger) -> String {
    match t {
        Trigger::Daily { hour, minute } => format!("daily {hour:02}:{minute:02}"),
        Trigger::Sunset { offset_minutes } => format!("sunset {offset_minutes:+}m"),
        Trigger::Sunrise { offset_minutes } => format!("sunrise {offset_minutes:+}m"),
        Trigger::Cron { expr } => format!("cron `{expr}`"),
    }
}
