//! `superpanels set` subcommand: end-to-end transient apply pipeline.
//!
//! Profiles-as-modes (`docs/spec/03-core-concepts.md` §3.4) doesn't change
//! `set` — it stays a one-shot apply that synthesises monitor placements
//! from the live OS layout (`layout::synthesise_placements`). Use the
//! profile manager or `superpanels profile` for persistent state.

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use superpanels_core::backends::{WallpaperBackend, detect_backend};
use superpanels_core::config::{
    BackendKind, Config, Profile, ProfileBody, SpanProfile, SpanSource,
};
use superpanels_core::detect;
use superpanels_core::display::{Monitor, MonitorRef};
use superpanels_core::image::{clear_temp_dir, load, render_slice, rotate, save_temp};
use superpanels_core::layout::{
    CropSpec, compute_crop_specs, cover_image_rect_mm, synthesise_placements,
};
use superpanels_core::schedule::TopologyFingerprint;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub(crate) struct SetArgs {
    pub image: PathBuf,
    pub extra_images: Vec<PathBuf>,
    pub backend: Option<String>,
    pub monitors: Option<String>,
    pub pins: Vec<String>,
    pub dry_run: bool,
    pub save_as: Option<String>,
}

pub(crate) fn run_via_ipc(
    args: &SetArgs,
    config_path: Option<&Path>,
    stream: &mut UnixStream,
) -> Result<()> {
    if let Some(ref name) = args.save_as {
        save_profile(args, name, config_path)?;
    }
    let params = json!({
        "image": args.image,
    });
    let resp = crate::ipc_client::call(stream, "set", params)?;
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
        "Set wallpaper on {n} monitor{s} via {backend} in {ms}ms",
        s = if n == 1 { "" } else { "s" },
    );
    Ok(())
}

pub(crate) fn run(
    args: &SetArgs,
    config_path: Option<&Path>,
    backend_override: Option<Box<dyn WallpaperBackend>>,
) -> Result<()> {
    if let Some(ref name) = args.save_as {
        save_profile(args, name, config_path)?;
    }

    if !args.extra_images.is_empty() {
        bail!(
            "multiple-image `set` (one per monitor) is not yet supported; \
             see PLAN.md §2"
        );
    }
    if !args.pins.is_empty() {
        bail!(
            "`--monitor NAME=PATH` per-monitor pinning is not yet supported; \
             see PLAN.md §2"
        );
    }
    let cfg = load_config(config_path)?;
    let mut monitors = detect(args.monitors.as_deref())?;
    cfg.merge_into_monitors(&mut monitors);

    let placements = synthesise_placements(&monitors);

    debug!(image = %args.image.display(), "set: loading source image");
    let source = load(&args.image)?;
    let image_size = (source.width(), source.height());

    let image_rect_mm = cover_image_rect_mm(&monitors, image_size);
    let specs = compute_crop_specs(&monitors, &placements, image_size, image_rect_mm)?;

    if args.dry_run {
        return print_dry_run(&specs, &monitors, image_size);
    }

    let backend: Box<dyn WallpaperBackend> = match backend_override {
        Some(b) => b,
        None => pick_backend(args.backend.as_deref(), &cfg)?,
    };

    clear_temp_dir()?;
    let assignments = render_per_monitor(&source, &specs, &monitors, apply_token())?;

    let report = backend.apply(&assignments)?;
    let elapsed_ms = report.duration.as_millis();
    let elapsed_ms_log = u64::try_from(elapsed_ms).unwrap_or(u64::MAX);
    info!(
        backend = report.backend,
        monitors = report.monitors_set,
        elapsed_ms = elapsed_ms_log,
        "set: applied"
    );
    println!(
        "Set wallpaper on {n} monitor{s} via {backend} in {ms}ms",
        n = report.monitors_set,
        s = if report.monitors_set == 1 { "" } else { "s" },
        backend = report.backend,
        ms = elapsed_ms,
    );
    Ok(())
}

fn load_config(config_path: Option<&Path>) -> Result<Config> {
    match config_path {
        Some(p) => Ok(Config::load_from(p)?),
        None => Ok(Config::load_or_default()?),
    }
}

fn pick_backend(requested: Option<&str>, cfg: &Config) -> Result<Box<dyn WallpaperBackend>> {
    let kind = match requested {
        None => cfg.backend.prefer,
        Some(s) => s
            .parse::<BackendKind>()
            .map_err(|e| anyhow::anyhow!("unknown backend `{s}`: {e}"))?,
    };
    Ok(detect_backend(kind, &cfg.backend.custom_command))
}

fn save_profile(args: &SetArgs, name: &str, config_path: Option<&Path>) -> Result<()> {
    let cfg_path = match config_path {
        Some(p) => p.to_owned(),
        None => Config::default_path()?,
    };
    let mut cfg = Config::load_or_default_from(&cfg_path)?;

    let backend_override = args
        .backend
        .as_deref()
        .map(|s| {
            s.parse::<BackendKind>()
                .map_err(|e| anyhow::anyhow!("unknown backend `{s}`: {e}"))
        })
        .transpose()?;

    let monitors = detect(args.monitors.as_deref()).unwrap_or_default();
    let cfg_for_merge = Config::load_or_default_from(&cfg_path).unwrap_or_default();
    let mut merged = monitors.clone();
    cfg_for_merge.merge_into_monitors(&mut merged);
    let placements = synthesise_placements(&merged);
    let topology = TopologyFingerprint::from_monitors(&merged);

    let image_size = superpanels_core::image::read_dimensions(&args.image).unwrap_or((1, 1));
    let image_rect_mm = cover_image_rect_mm(&merged, image_size);

    let now = superpanels_core::config::now_timestamp();
    let profile = Profile {
        name: name.to_owned(),
        body: ProfileBody::Span(SpanProfile {
            source: SpanSource::Single {
                path: args.image.clone(),
            },
            image_rect_mm,
        }),
        monitor_state: placements,
        topology,
        description: None,
        created_at: now,
        updated_at: now,
        last_applied_at: None,
        backend_override,
    };

    if let Some(existing) = cfg.profiles.iter_mut().find(|p| p.name == name) {
        *existing = profile;
    } else {
        cfg.profiles.push(profile);
    }
    cfg.save_to(&cfg_path)
        .with_context(|| format!("saving profile '{name}' to {}", cfg_path.display()))?;
    info!(name, path = %cfg_path.display(), "saved profile");
    println!("Saved profile '{name}'.");
    Ok(())
}

fn print_dry_run(specs: &[CropSpec], monitors: &[Monitor], image_size: (u32, u32)) -> Result<()> {
    let payload = dry_run_payload(specs, monitors, image_size);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer_pretty(&mut out, &payload)?;
    writeln!(out).ok();
    Ok(())
}

fn dry_run_payload(specs: &[CropSpec], monitors: &[Monitor], image_size: (u32, u32)) -> Value {
    json!({
        "image_size": [image_size.0, image_size.1],
        "monitors": monitors.iter().map(|m| json!({
            "id": m.id,
            "name": m.name,
            "stable_id": m.stable_id,
            "resolution": [m.resolution.0, m.resolution.1],
            "physical_mm": m.physical_size_mm.map(|(w, h)| [w, h]),
        })).collect::<Vec<_>>(),
        "crops": specs,
    })
}

fn render_per_monitor(
    source: &image::DynamicImage,
    specs: &[CropSpec],
    monitors: &[Monitor],
    token: u128,
) -> Result<Vec<(MonitorRef, PathBuf)>> {
    let mut out = Vec::with_capacity(specs.len());
    for spec in specs {
        let monitor = monitor_for_spec(monitors, spec)?;
        let composed = render_slice(source, spec)?;
        let rotated = rotate(&composed, spec.rotation);
        let safe = sanitise_filename(&monitor.name);
        let filename = format!("{safe}-{token}.png");
        let path = save_temp(&rotated, &filename)?;
        debug!(monitor = %monitor.name, file = %path.display(), "set: wrote temp slice");
        out.push((to_monitor_ref(monitor), path));
    }
    Ok(out)
}

fn apply_token() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn monitor_for_spec<'a>(monitors: &'a [Monitor], spec: &CropSpec) -> Result<&'a Monitor> {
    monitors
        .iter()
        .find(|m| m.id == spec.monitor_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "crop spec references unknown monitor id {:?}",
                spec.monitor_id
            )
        })
}

fn to_monitor_ref(m: &Monitor) -> MonitorRef {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on Err; no recovery is meaningful
#[allow(clippy::panic)] // reason: panic on unexpected Result variant is the test failure
mod tests {
    use super::*;

    #[test]
    fn sanitise_filename_replaces_path_separators() {
        let got = sanitise_filename("DP-1/../etc/passwd");
        assert_eq!(got, "DP-1_.._etc_passwd");
    }

    #[test]
    fn sanitise_filename_keeps_dot_dash_underscore() {
        let got = sanitise_filename("HDMI-A_1.0");
        assert_eq!(got, "HDMI-A_1.0");
    }

    #[test]
    fn pick_backend_rejects_unknown_name() {
        let cfg = superpanels_core::config::Config::default();
        let result = pick_backend(Some("unicorn"), &cfg);
        let Err(err) = result else {
            panic!("expected Err for unknown backend, got Ok");
        };
        assert!(err.to_string().contains("unknown backend"), "msg was {err}");
    }
}
