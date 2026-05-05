//! `superpanels set` subcommand: end-to-end apply pipeline (`SPEC.md` §11.1).

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
use superpanels_core::image::{
    FitMode as ImageFitMode, clear_temp_dir, crop, load, rotate, save_temp, scale_to_fit,
};
use superpanels_core::layout::{
    BezelConfig, CropSpec, FitMode as LayoutFitMode, compute_crop_specs_with_offset,
};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub(crate) struct SetArgs {
    pub image: PathBuf,
    /// Forward-compat with multi-image; rejected in Phase 1.
    pub extra_images: Vec<PathBuf>,
    pub bezel_h: Option<f32>,
    pub bezel_v: Option<f32>,
    pub fit: LayoutFitMode,
    /// Accepted for spec compat; not yet honoured.
    pub offset: Option<(i32, i32)>,
    pub backend: Option<String>,
    pub monitors: Option<String>,
    /// `--monitor DP-1=PATH` pins. Phase 2 feature; rejected here.
    pub pins: Vec<String>,
    pub dry_run: bool,
    /// Write the current args as a named profile before applying.
    pub save_as: Option<String>,
}

/// Forward `set` args to the daemon via `stream` and print the result.
/// `config_path` is used only when `--save-as` is provided.
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
        "bezel_h": args.bezel_h.unwrap_or(0.0_f32),
        "bezel_v": args.bezel_v.unwrap_or(0.0_f32),
        "fit": args.fit,
        "offset_px": args.offset.map_or([0, 0], |(x, y)| [x, y]),
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

    let bezels = resolve_bezels(args);

    debug!(image = %args.image.display(), "set: loading source image");
    let source = load(&args.image)?;
    let image_size = (source.width(), source.height());

    let offset = args.offset.map_or([0, 0], |(x, y)| [x, y]);
    let specs = compute_crop_specs_with_offset(&monitors, &bezels, args.fit, image_size, offset)?;

    if args.dry_run {
        return print_dry_run(&specs, &monitors, bezels, image_size);
    }

    let backend: Box<dyn WallpaperBackend> = match backend_override {
        Some(b) => b,
        None => pick_backend(args.backend.as_deref(), &cfg)?,
    };

    clear_temp_dir()?;
    let assignments = render_per_monitor(&source, &specs, &monitors, apply_token())?;

    let report = backend.apply(&assignments)?;
    let elapsed_ms = report.duration.as_millis();
    // u64::MAX ms ≈ 584 million years; saturation is lossless for any real apply.
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

fn resolve_bezels(args: &SetArgs) -> BezelConfig {
    // Phase 1: no profile machinery yet — CLI flags are the only source.
    BezelConfig {
        horizontal_mm: args.bezel_h.unwrap_or(0.0),
        vertical_mm: args.bezel_v.unwrap_or(0.0),
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

/// Write (or overwrite) a profile named `name` built from `args` into the
/// config file. The profile uses a `Single` span source pointing at the image.
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

    let profile = Profile {
        name: name.to_owned(),
        body: ProfileBody::Span(SpanProfile {
            source: SpanSource::Single {
                path: args.image.clone(),
            },
            fit: args.fit,
            offset: args.offset.map_or([0, 0], |(x, y)| [x, y]),
            image_size_px: None,
        }),
        bezels: BezelConfig {
            horizontal_mm: args.bezel_h.unwrap_or(0.0),
            vertical_mm: args.bezel_v.unwrap_or(0.0),
        },
        backend_override,
        schedule: None,
    };

    // Upsert: replace existing profile with the same name, or append.
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

fn print_dry_run(
    specs: &[CropSpec],
    monitors: &[Monitor],
    bezels: BezelConfig,
    image_size: (u32, u32),
) -> Result<()> {
    let payload = dry_run_payload(specs, monitors, bezels, image_size);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer_pretty(&mut out, &payload)?;
    writeln!(out).ok();
    Ok(())
}

fn dry_run_payload(
    specs: &[CropSpec],
    monitors: &[Monitor],
    bezels: BezelConfig,
    image_size: (u32, u32),
) -> Value {
    json!({
        "image_size": [image_size.0, image_size.1],
        "bezels": bezels,
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
        let cropped = crop(source, spec.src_rect)?;
        // Crop already matches the monitor's physical aspect — Stretch to dst_size
        // avoids re-introducing letterboxing.
        let resized = scale_to_fit(&cropped, spec.dst_size, ImageFitMode::Stretch);
        let rotated = rotate(&resized, spec.rotation);
        let safe = sanitise_filename(&monitor.name);
        // Per-apply cache-buster: Plasma's org.kde.image plugin caches by URL,
        // so a unique filename per apply forces a redraw. `clear_temp_dir()`
        // ran above so tokens don't accumulate.
        let filename = format!("{safe}-{token}.png");
        let path = save_temp(&rotated, &filename)?;
        debug!(monitor = %monitor.name, file = %path.display(), "set: wrote temp slice");
        out.push((to_monitor_ref(monitor), path));
    }
    Ok(out)
}

/// Per-apply cache-buster token; wall-clock nanos.
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

/// Replace any non-`[A-Za-z0-9._-]` with `_`. Defence in depth — names come
/// from compositor output, not user input.
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
    use tempfile::tempdir;

    #[test]
    fn sanitise_filename_replaces_path_separators() {
        // Arrange / Act
        let got = sanitise_filename("DP-1/../etc/passwd");

        // Assert
        assert_eq!(got, "DP-1_.._etc_passwd");
    }

    #[test]
    fn sanitise_filename_keeps_dot_dash_underscore() {
        // Arrange / Act
        let got = sanitise_filename("HDMI-A_1.0");

        // Assert
        assert_eq!(got, "HDMI-A_1.0");
    }

    #[test]
    fn resolve_bezels_uses_flag_values_when_provided() {
        // Arrange
        let args = SetArgs {
            image: PathBuf::from("x.jpg"),
            extra_images: vec![],
            bezel_h: Some(8.0),
            bezel_v: Some(5.0),
            fit: LayoutFitMode::Fill,
            offset: None,
            backend: None,
            monitors: None,
            pins: vec![],
            dry_run: false,
            save_as: None,
        };

        // Act
        let bezels = resolve_bezels(&args);

        // Assert
        assert!((bezels.horizontal_mm - 8.0).abs() < f32::EPSILON);
        assert!((bezels.vertical_mm - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn resolve_bezels_defaults_to_zero_when_no_flag() {
        // Arrange
        let args = SetArgs {
            image: PathBuf::from("x.jpg"),
            extra_images: vec![],
            bezel_h: None,
            bezel_v: None,
            fit: LayoutFitMode::Fill,
            offset: None,
            backend: None,
            monitors: None,
            pins: vec![],
            dry_run: false,
            save_as: None,
        };

        // Act
        let bezels = resolve_bezels(&args);

        // Assert
        assert!(bezels.horizontal_mm.abs() < f32::EPSILON);
        assert!(bezels.vertical_mm.abs() < f32::EPSILON);
    }

    #[test]
    fn dry_run_payload_serialises_both_physical_mm_branches() {
        // Arrange — pins both `[w, h]` and `null` branches of physical_mm.
        use superpanels_core::display::{MonitorId, Rotation};
        use superpanels_core::layout::{CropSpec, Rect};

        let monitors = vec![
            Monitor {
                id: MonitorId(0),
                name: "DP-1".to_owned(),
                stable_id: Some("uuid-a".to_owned()),
                position: (0, 0),
                resolution: (2560, 1440),
                physical_size_mm: Some((597, 336)),
                scale: 1.0,
                rotation: Rotation::None,
                refresh_hz: None,
                primary: true,
                ppi: Some(108.79),
            },
            Monitor {
                id: MonitorId(1),
                name: "DP-2".to_owned(),
                stable_id: None,
                position: (2560, 0),
                resolution: (1920, 1080),
                physical_size_mm: None,
                scale: 1.0,
                rotation: Rotation::None,
                refresh_hz: None,
                primary: false,
                ppi: None,
            },
        ];
        let specs = vec![CropSpec {
            monitor_id: MonitorId(0),
            src_rect: Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            dst_size: (2560, 1440),
            rotation: Rotation::None,
            fit: LayoutFitMode::Fill,
        }];
        let bezels = BezelConfig {
            horizontal_mm: 8.0,
            vertical_mm: 5.0,
        };

        // Act
        let payload = dry_run_payload(&specs, &monitors, bezels, (3840, 2160));

        // Assert
        let expected = serde_json::json!({
            "image_size": [3840, 2160],
            "bezels": { "horizontal_mm": 8.0, "vertical_mm": 5.0 },
            "monitors": [
                {
                    "id": 0,
                    "name": "DP-1",
                    "stable_id": "uuid-a",
                    "resolution": [2560, 1440],
                    "physical_mm": [597, 336],
                },
                {
                    "id": 1,
                    "name": "DP-2",
                    "stable_id": null,
                    "resolution": [1920, 1080],
                    "physical_mm": null,
                },
            ],
            "crops": [
                {
                    "monitor_id": 0,
                    "src_rect": { "x": 0, "y": 0, "w": 100, "h": 100 },
                    "dst_size": [2560, 1440],
                    "rotation": "none",
                    "fit": "fill",
                }
            ],
        });
        assert_eq!(payload, expected);
    }

    #[test]
    fn set_pipeline_with_mock_backend_runs_end_to_end() {
        // Arrange
        use image::{DynamicImage, RgbaImage};
        use superpanels_core::backends::MockBackend;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let img_path = dir.path().join("pano.png");
        DynamicImage::ImageRgba8(RgbaImage::new(1920, 1080))
            .save(&img_path)
            .unwrap();
        let cfg_path = dir.path().join("config.toml");
        std::fs::write(&cfg_path, "").unwrap();

        let args = SetArgs {
            image: img_path,
            extra_images: vec![],
            bezel_h: None,
            bezel_v: None,
            fit: LayoutFitMode::Fill,
            offset: None,
            backend: None,
            monitors: Some("1920x1080+0+0?480x270".to_owned()),
            pins: vec![],
            dry_run: false,
            save_as: None,
        };

        // Act
        let result = run(&args, Some(&cfg_path), Some(Box::new(MockBackend::new())));

        // Assert
        if let Err(e) = result {
            panic!("pipeline failed: {e:#}");
        }
    }

    #[test]
    fn save_as_writes_profile_to_config_then_removes_it() {
        // Arrange
        use superpanels_core::config::Config;

        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");
        std::fs::write(&cfg_path, "").unwrap();
        let args = SetArgs {
            image: dir.path().join("w.jpg"),
            extra_images: vec![],
            bezel_h: Some(8.0),
            bezel_v: Some(5.0),
            fit: LayoutFitMode::Fill,
            offset: None,
            backend: None,
            monitors: None,
            pins: vec![],
            dry_run: false,
            save_as: Some("my-profile".to_owned()),
        };

        // Act
        save_profile(&args, "my-profile", Some(&cfg_path)).unwrap();

        // Assert — profile was persisted
        let cfg = Config::load_from(&cfg_path).unwrap();
        assert_eq!(cfg.profiles.len(), 1);
        let p = &cfg.profiles[0];
        assert_eq!(p.name, "my-profile");
        assert!((p.bezels.horizontal_mm - 8.0).abs() < f32::EPSILON);

        // Act again — upsert overwrites
        save_profile(&args, "my-profile", Some(&cfg_path)).unwrap();
        let cfg2 = Config::load_from(&cfg_path).unwrap();
        assert_eq!(cfg2.profiles.len(), 1, "upsert must not duplicate");
    }

    #[test]
    fn pick_backend_rejects_unknown_name() {
        // Arrange
        let cfg = superpanels_core::config::Config::default();

        // Act
        let result = pick_backend(Some("unicorn"), &cfg);

        // Assert
        let Err(err) = result else {
            panic!("expected Err for unknown backend, got Ok");
        };
        assert!(err.to_string().contains("unknown backend"), "msg was {err}");
    }
}
