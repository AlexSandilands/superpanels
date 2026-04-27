//! `superpanels set` subcommand: end-to-end apply pipeline (`SPEC.md` §11.1).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use superpanels_core::backends::{KdeBackend, WallpaperBackend};
use superpanels_core::config::Config;
use superpanels_core::detect;
use superpanels_core::display::{Availability, Monitor, MonitorRef};
use superpanels_core::image::{
    FitMode as ImageFitMode, clear_temp_dir, crop, load, rotate, save_temp, scale_to_fit,
};
use superpanels_core::layout::{
    BezelConfig, CropSpec, FitMode as LayoutFitMode, compute_crop_specs,
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
}

pub(crate) fn run(
    args: &SetArgs,
    config_path: Option<&Path>,
    backend_override: Option<Box<dyn WallpaperBackend>>,
) -> Result<()> {
    if !args.extra_images.is_empty() {
        bail!(
            "multiple-image `set` (one per monitor) is not yet supported in Phase 1; \
             see PLAN.md §2"
        );
    }
    if !args.pins.is_empty() {
        bail!(
            "`--monitor NAME=PATH` per-monitor pinning is not yet supported in Phase 1; \
             see PLAN.md §2"
        );
    }
    if args.offset.is_some() {
        info!("--offset is accepted but not yet honoured in Phase 1");
    }

    let cfg = load_config(config_path)?;
    let mut monitors = detect(args.monitors.as_deref())?;
    cfg.merge_into_monitors(&mut monitors);

    let bezels = resolve_bezels(args);

    debug!(image = %args.image.display(), "set: loading source image");
    let source = load(&args.image)?;
    let image_size = (source.width(), source.height());

    let specs = compute_crop_specs(&monitors, &bezels, args.fit, image_size)?;

    if args.dry_run {
        return print_dry_run(&specs, &monitors, bezels, image_size);
    }

    let backend: Box<dyn WallpaperBackend> = match backend_override {
        Some(b) => b,
        None => pick_backend(args.backend.as_deref())?,
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

fn pick_backend(requested: Option<&str>) -> Result<Box<dyn WallpaperBackend>> {
    match requested {
        None | Some("kde" | "auto") => {
            let kde = KdeBackend::new();
            match kde.availability() {
                Availability::Available => Ok(Box::new(kde)),
                other => Err(anyhow!(BackendUnavailable {
                    backend: "kde",
                    detail: format!("{other:?}"),
                })),
            }
        }
        Some(other) => bail!(
            "backend `{other}` is not implemented in Phase 1 (only `kde`); \
             see PLAN.md §2.1"
        ),
    }
}

/// `main` downcasts on this to map to exit code 4.
#[derive(Debug, thiserror::Error)]
#[error("backend `{backend}` is not available: {detail}")]
pub(crate) struct BackendUnavailable {
    pub(crate) backend: &'static str,
    pub(crate) detail: String,
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
            anyhow!(
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
                    "rotation": "None",
                    "fit": "Fill",
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
        };

        // Act
        let result = run(&args, Some(&cfg_path), Some(Box::new(MockBackend::new())));

        // Assert
        if let Err(e) = result {
            panic!("pipeline failed: {e:#}");
        }
    }

    #[test]
    fn pick_backend_rejects_unknown_name() {
        // Act
        let result = pick_backend(Some("gnome"));

        // Assert
        let Err(err) = result else {
            panic!("expected Err for unknown backend, got Ok");
        };
        let msg = err.to_string();
        assert!(msg.contains("not implemented"), "msg was {msg}");
    }
}
