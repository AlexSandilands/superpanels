//! `superpanels profile` subcommands (`SPEC.md` §11.2).

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use superpanels_core::backends::detect_backend;
use superpanels_core::config::{
    BackendKind, Config, PerMonitorAssignment, Profile, ProfileBody, SpanSource,
};
use superpanels_core::detect;
use superpanels_core::display::{Monitor, MonitorRef};
use superpanels_core::image::{
    FitMode as ImageFitMode, clear_temp_dir, load, render_slice, save_temp, scale_to_fit,
};
use superpanels_core::layout::{
    FitMode as LayoutFitMode, ImageRectMm, compute_crop_specs, cover_image_rect_mm,
    synthesise_placements,
};
use superpanels_core::schedule::MonitorPlacement;
use tracing::{info, warn};

use crate::ipc_client;

/// Portable profile bundle for `export` / `import`.
#[derive(Debug, Serialize, Deserialize)]
struct ProfileBundle {
    #[serde(rename = "profile")]
    profiles: Vec<Profile>,
}

pub(crate) fn list_cmd(json: bool, config_path: Option<&Path>) -> Result<()> {
    let cfg = load_config(config_path)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &cfg.profiles)?;
        writeln!(out).ok();
    } else {
        for p in &cfg.profiles {
            writeln!(out, "{}", p.name)?;
        }
    }
    Ok(())
}

/// Apply a profile, forwarding to the daemon via `stream` when available.
pub(crate) fn apply_cmd(
    name: &str,
    config_path: Option<&Path>,
    stream: Option<&mut UnixStream>,
) -> Result<()> {
    if let Some(stream) = stream {
        let resp = ipc_client::call(stream, "apply_profile", json!({"name": name}))?;
        if !resp.is_ok() {
            bail!(
                "{}",
                resp.error.as_deref().unwrap_or("daemon returned error")
            );
        }
        let r = resp.result.unwrap_or_default();
        let backend = r.get("backend").and_then(|v| v.as_str()).unwrap_or("?");
        let n = r
            .get("monitors_set")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let ms = r
            .get("elapsed_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        println!(
            "Applied '{name}' on {n} monitor{s} via {backend} in {ms}ms",
            s = if n == 1 { "" } else { "s" }
        );
        return Ok(());
    }
    // In-process fallback (--no-daemon or daemon not running).
    let cfg = load_config(config_path)?;
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("profile '{name}' not found"))?;
    let mut monitors = detect(None)?;
    cfg.merge_into_monitors(&mut monitors);
    let backend_kind = profile.backend_override.unwrap_or(cfg.backend.prefer);
    let custom_cmd = cfg.backend.custom_command.clone();
    match &profile.body {
        ProfileBody::Span(span) => {
            let image_path = match &span.source {
                SpanSource::Single { path } => path.clone(),
                SpanSource::Slideshow { .. } => {
                    bail!(
                        "slideshow profiles require a running daemon; \
                         start one with `superpanels daemon`"
                    )
                }
            };
            run_span_apply(
                &image_path,
                &monitors,
                &profile.monitor_state,
                Some(span.image_rect_mm),
                backend_kind,
                &custom_cmd,
            )?;
        }
        ProfileBody::PerMonitor(pm) => {
            run_per_monitor_apply(
                &pm.assignments,
                &monitors,
                pm.fit,
                backend_kind,
                &custom_cmd,
            )?;
        }
    }
    info!(name, "profile applied in-process");
    println!("Applied profile '{name}'.");
    Ok(())
}

pub(crate) fn delete_cmd(name: &str, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_config_path(config_path)?;
    let mut cfg = Config::load_from(&cfg_path)?;
    let before = cfg.profiles.len();
    cfg.profiles.retain(|p| p.name != name);
    if cfg.profiles.len() == before {
        bail!("profile '{name}' not found");
    }
    cfg.save_to(&cfg_path)?;
    println!("Deleted '{name}'.");
    Ok(())
}

pub(crate) fn rename_cmd(old: &str, new_name: &str, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_config_path(config_path)?;
    let mut cfg = Config::load_from(&cfg_path)?;
    if cfg.profiles.iter().any(|p| p.name == new_name) {
        bail!("a profile named '{new_name}' already exists");
    }
    match cfg.profiles.iter_mut().find(|p| p.name == old) {
        Some(p) => new_name.clone_into(&mut p.name),
        None => bail!("profile '{old}' not found"),
    }
    cfg.save_to(&cfg_path)?;
    println!("Renamed '{old}' → '{new_name}'.");
    Ok(())
}

pub(crate) fn export_cmd(
    name: &str,
    output: Option<&Path>,
    config_path: Option<&Path>,
) -> Result<()> {
    let cfg = load_config(config_path)?;
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("profile '{name}' not found"))?
        .clone();
    let bundle = ProfileBundle {
        profiles: vec![profile],
    };
    let text = toml::to_string(&bundle).context("serialising profile bundle")?;
    match output {
        Some(path) => {
            std::fs::write(path, &text)
                .with_context(|| format!("writing to {}", path.display()))?;
        }
        None => print!("{text}"),
    }
    Ok(())
}

pub(crate) fn show_cmd(name: &str, config_path: Option<&Path>) -> Result<()> {
    let cfg = load_config(config_path)?;
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("profile '{name}' not found"))?;
    let text = toml::to_string(profile).context("serialising profile")?;
    print!("{text}");
    Ok(())
}

pub(crate) fn duplicate_cmd(name: &str, new_name: &str, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_config_path(config_path)?;
    let mut cfg = Config::load_from(&cfg_path)?;
    let source = cfg
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("profile '{name}' not found"))?
        .clone();
    if cfg.profiles.iter().any(|p| p.name == new_name) {
        bail!("a profile named '{new_name}' already exists");
    }
    let now = superpanels_core::config::now_timestamp();
    let mut copy = source;
    new_name.clone_into(&mut copy.name);
    copy.created_at = now;
    copy.updated_at = now;
    copy.last_applied_at = None;
    cfg.profiles.push(copy);
    cfg.save_to(&cfg_path)?;
    println!("Duplicated '{name}' → '{new_name}'.");
    Ok(())
}

pub(crate) fn import_cmd(file: &Path, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = resolve_config_path(config_path)?;
    let mut cfg = Config::load_from(&cfg_path)?;
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let bundle: ProfileBundle = toml::from_str(&text).context("parsing profile bundle")?;
    let mut added = 0usize;
    for profile in bundle.profiles {
        if cfg.profiles.iter().any(|p| p.name == profile.name) {
            eprintln!(
                "warning: profile '{}' already exists, skipping",
                profile.name
            );
        } else {
            cfg.profiles.push(profile);
            added += 1;
        }
    }
    cfg.save_to(&cfg_path)?;
    println!(
        "Imported {added} profile{s}.",
        s = if added == 1 { "" } else { "s" }
    );
    Ok(())
}

// --- in-process apply helpers ---

fn run_span_apply(
    image_path: &Path,
    monitors: &[Monitor],
    placements: &HashMap<String, MonitorPlacement>,
    image_rect_mm: Option<ImageRectMm>,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<()> {
    let source = load(image_path).with_context(|| format!("loading {}", image_path.display()))?;
    let image_size = (source.width(), source.height());
    let synth;
    let p: &HashMap<String, MonitorPlacement> = if placements.is_empty() {
        synth = synthesise_placements(monitors);
        &synth
    } else {
        placements
    };
    let rect = image_rect_mm.unwrap_or_else(|| cover_image_rect_mm(monitors, image_size));
    let specs = compute_crop_specs(monitors, p, image_size, rect)?;
    let backend = detect_backend(backend_kind, custom_cmd);
    clear_temp_dir()?;
    let token = apply_token();
    let mut assignments: Vec<(MonitorRef, PathBuf)> = Vec::with_capacity(specs.len());
    for spec in &specs {
        let monitor = monitors
            .iter()
            .find(|m| m.id == spec.monitor_id)
            .ok_or_else(|| {
                anyhow::anyhow!("crop spec references unknown monitor {:?}", spec.monitor_id)
            })?;
        // Save at canvas/post-rotation dims; backends paint into the
        // rotated framebuffer themselves (memory: KDE wallpaper orientation).
        let composed = render_slice(&source, spec)?;
        let safe = sanitise_filename(&monitor.name);
        let path = save_temp(&composed, &format!("{safe}-{token}.png"))?;
        assignments.push((monitor_ref(monitor), path));
    }
    backend.apply(&assignments).context("backend apply")?;
    Ok(())
}

fn run_per_monitor_apply(
    assignments: &[PerMonitorAssignment],
    monitors: &[Monitor],
    fit: LayoutFitMode,
    backend_kind: BackendKind,
    custom_cmd: &str,
) -> Result<()> {
    let backend = detect_backend(backend_kind, custom_cmd);
    clear_temp_dir()?;
    let token = apply_token();
    let mut backend_assignments: Vec<(MonitorRef, PathBuf)> = Vec::new();
    for assignment in assignments {
        let monitor = monitors.iter().find(|m| {
            m.stable_id
                .as_deref()
                .is_some_and(|id| id == assignment.monitor.stable_id)
                || m.name == assignment.monitor.name
        });
        if let Some(monitor) = monitor {
            let source = load(&assignment.path)
                .with_context(|| format!("loading {}", assignment.path.display()))?;
            let resized = scale_to_fit(&source, monitor.resolution, layout_fit(fit));
            let safe = sanitise_filename(&monitor.name);
            let path = save_temp(&resized, &format!("{safe}-{token}.png"))?;
            backend_assignments.push((monitor_ref(monitor), path));
        } else {
            warn!(monitor = %assignment.monitor.name, "monitor not found in layout; skipping");
        }
    }
    backend
        .apply(&backend_assignments)
        .context("backend apply")?;
    Ok(())
}

fn layout_fit(f: LayoutFitMode) -> ImageFitMode {
    match f {
        LayoutFitMode::Fill => ImageFitMode::Fill,
        LayoutFitMode::Fit => ImageFitMode::Fit,
        LayoutFitMode::Stretch => ImageFitMode::Stretch,
        LayoutFitMode::Center => ImageFitMode::Center,
    }
}

fn monitor_ref(m: &Monitor) -> MonitorRef {
    MonitorRef {
        stable_id: m.stable_id.clone().unwrap_or_else(|| m.name.clone()),
        name: m.name.clone(),
    }
}

fn sanitise_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn apply_token() -> u128 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn load_config(path: Option<&Path>) -> Result<Config> {
    Ok(match path {
        Some(p) => Config::load_from(p)?,
        None => Config::load_or_default()?,
    })
}

fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    Ok(match path {
        Some(p) => p.to_owned(),
        None => Config::default_path()?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
mod tests {
    use super::*;
    use superpanels_core::TopologyFingerprint;
    use superpanels_core::config::{SpanProfile, SpanSource};
    use tempfile::tempdir;

    fn sample_profile(name: &str) -> Profile {
        let now = superpanels_core::config::now_timestamp();
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Single {
                    path: PathBuf::from("/walls/sample.jpg"),
                },
                image_rect_mm: ImageRectMm::default(),
            }),
            monitor_state: HashMap::new(),
            topology: TopologyFingerprint(String::new()),
            description: None,
            created_at: now,
            updated_at: now,
            last_applied_at: None,
            backend_override: None,
        }
    }

    fn write_config_with_profiles(dir: &Path, profiles: Vec<Profile>) -> PathBuf {
        let cfg_path = dir.join("config.toml");
        let cfg = Config {
            profiles,
            ..Config::default()
        };
        cfg.save_to(&cfg_path).unwrap();
        cfg_path
    }

    #[test]
    fn export_round_trips_through_import() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path = write_config_with_profiles(dir.path(), vec![sample_profile("test")]);
        let bundle_path = dir.path().join("bundle.toml");

        // Act
        export_cmd("test", Some(&bundle_path), Some(&cfg_path)).unwrap();
        let cfg2_path = write_config_with_profiles(dir.path(), vec![]);
        import_cmd(&bundle_path, Some(&cfg2_path)).unwrap();

        // Assert
        let cfg2 = Config::load_from(&cfg2_path).unwrap();
        assert_eq!(cfg2.profiles.len(), 1);
        assert_eq!(cfg2.profiles[0].name, "test");
    }

    #[test]
    fn delete_removes_named_profile() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path = write_config_with_profiles(dir.path(), vec![sample_profile("to-delete")]);

        // Act
        delete_cmd("to-delete", Some(&cfg_path)).unwrap();

        // Assert
        let cfg = Config::load_from(&cfg_path).unwrap();
        assert!(cfg.profiles.is_empty());
    }

    #[test]
    fn delete_missing_profile_returns_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path = write_config_with_profiles(dir.path(), vec![sample_profile("exists")]);

        // Act + Assert
        assert!(delete_cmd("no-such", Some(&cfg_path)).is_err());
    }

    #[test]
    fn rename_updates_profile_name() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path = write_config_with_profiles(dir.path(), vec![sample_profile("old")]);

        // Act
        rename_cmd("old", "new", Some(&cfg_path)).unwrap();

        // Assert
        let cfg = Config::load_from(&cfg_path).unwrap();
        assert_eq!(cfg.profiles[0].name, "new");
    }

    #[test]
    fn rename_to_existing_name_returns_error() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path =
            write_config_with_profiles(dir.path(), vec![sample_profile("a"), sample_profile("b")]);

        // Act + Assert
        assert!(rename_cmd("a", "b", Some(&cfg_path)).is_err());
    }

    #[test]
    fn import_skips_duplicate_names() {
        // Arrange
        let dir = tempdir().unwrap();
        let cfg_path = write_config_with_profiles(dir.path(), vec![sample_profile("shared")]);
        let bundle_path = dir.path().join("bundle.toml");
        export_cmd("shared", Some(&bundle_path), Some(&cfg_path)).unwrap();

        // Act — import into the same config that already has "shared"
        import_cmd(&bundle_path, Some(&cfg_path)).unwrap();

        // Assert — still only one profile
        let cfg = Config::load_from(&cfg_path).unwrap();
        assert_eq!(cfg.profiles.len(), 1);
    }
}
