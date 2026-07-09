//! In-process implementations for IPC methods that don't need the daemon.
//!
//! Same surface as the daemon's `dispatch`, but each method calls into
//! `superpanels-core` directly. Used when no daemon socket is reachable so
//! one-shot CLI-equivalent operations still work from the GUI.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{Value, json};
use superpanels_core::config::{Config, ConfigError, write_monitor_block};
use superpanels_core::detect;
use superpanels_core::ipc::validate as v;
use superpanels_core::library::{LibraryFilter as CoreLibraryFilter, apply_library_filter};

use crate::bridge::{CallResult, ok_payload, ok_unit};
use crate::errors::IpcError;

/// Floor when `LibraryConfig::thumbnail_size` is misconfigured.
const THUMBNAIL_MIN_EDGE: u32 = 64;

pub(crate) fn dispatch(method: &str, params: &Value, config_path: Option<&Path>) -> CallResult {
    match method {
        "detect_monitors" | "redetect" => detect_monitors(config_path),
        "list_profiles" => list_profiles(config_path),
        "apply_profile" => apply_profile(),
        "apply_canvas" => apply_canvas(),
        "save_profile" => save_profile(params, config_path),
        "update_profile_source" => update_profile_source(params, config_path),
        "delete_profile" => delete_profile(params, config_path),
        "preview_crop" => preview_crop(params, config_path),
        "library_list" => library_list(params, config_path),
        "library_thumbnail" => library_thumbnail(params, config_path),
        "library_tag" => Err(IpcError::internal(
            "library_tag is not implemented in-process; requires daemon library state",
        )),
        "library_delete" => Err(IpcError::internal(
            "library_delete is not implemented in-process; requires daemon library state",
        )),
        "library_rescan" => library_rescan(config_path),
        "get_config" => get_config(config_path),
        "save_config" => save_config(params, config_path),
        "set_monitor_physical_size" => set_monitor_physical_size(params, config_path),
        "current_state" => Ok(current_state()),
        other => Err(IpcError::invalid(format!(
            "method '{other}' has no in-process implementation"
        ))),
    }
}

fn load_config(path: Option<&Path>) -> Result<Config, IpcError> {
    Ok(match path {
        Some(p) => Config::load_from(p)?,
        None => Config::load_or_default()?,
    })
}

fn config_path_or_default(path: Option<&Path>) -> Result<std::path::PathBuf, IpcError> {
    Ok(match path {
        Some(p) => p.to_owned(),
        None => Config::default_path()?,
    })
}

fn detect_monitors(config_path: Option<&Path>) -> CallResult {
    let mut monitors = detect(None)?;
    let cfg = load_config(config_path)?;
    cfg.merge_into_monitors(&mut monitors);
    Ok(ok_payload(monitors))
}

fn list_profiles(config_path: Option<&Path>) -> CallResult {
    let cfg = load_config(config_path)?;
    // Validity needs detected monitors merged with config-supplied physical
    // sizes. If detection fails (no compositor in scope —
    // e.g. CI, headless dev), fall through with an empty validity list so
    // the rest of the GUI keeps working; the daemon path is the source of
    // truth in production.
    let validity_entries: Vec<Value> = match detect(None) {
        Ok(mut monitors) => {
            cfg.merge_into_monitors(&mut monitors);
            cfg.profiles
                .iter()
                .map(|p| {
                    let v = superpanels_core::ProfileValidity::evaluate(p, &monitors);
                    json!({ "profile": p.name, "validity": v })
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };
    Ok(json!({
        "profiles": cfg.profiles,
        "validity": validity_entries,
    }))
}

fn apply_profile() -> CallResult {
    Err(IpcError::internal(
        "apply_profile in-process requires daemon-equivalent runtime state; \
         start `superpanels-daemon` to apply profiles from the GUI",
    ))
}

fn apply_canvas() -> CallResult {
    Err(IpcError::internal(
        "apply_canvas in-process requires daemon-equivalent runtime state; \
         start `superpanels-daemon` to apply canvas state from the GUI",
    ))
}

fn save_profile(params: &Value, config_path: Option<&Path>) -> CallResult {
    let profile_val = params
        .get("profile")
        .ok_or_else(|| IpcError::invalid("params.profile required"))?;
    let profile: superpanels_core::Profile = serde_json::from_value(profile_val.clone())
        .map_err(|e| IpcError::invalid(format!("profile is malformed: {e}")))?;
    let path = config_path_or_default(config_path)?;
    let mut cfg = Config::load_from(&path)?;
    if let Some(existing) = cfg.profiles.iter_mut().find(|p| p.name == profile.name) {
        *existing = profile;
    } else {
        cfg.profiles.push(profile);
    }
    cfg.save_to(&path)?;
    Ok(ok_unit())
}

fn update_profile_source(params: &Value, config_path: Option<&Path>) -> CallResult {
    let name = params
        .get("profile")
        .and_then(Value::as_str)
        .ok_or_else(|| IpcError::invalid("params.profile required"))?;
    let source: superpanels_core::config::SlideshowSource = params
        .get("source")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| IpcError::invalid("params.source (SlideshowSource) required"))?;
    let path = config_path_or_default(config_path)?;
    let mut cfg = Config::load_from(&path)?;
    cfg.set_slideshow_source(name, source)
        .map_err(|e| match e {
            superpanels_core::config::SlideshowSourceError::ProfileNotFound(_) => {
                IpcError::Config(e.to_string())
            }
            superpanels_core::config::SlideshowSourceError::NotSlideshow => {
                IpcError::invalid(e.to_string())
            }
        })?;
    cfg.save_to(&path)?;
    Ok(ok_unit())
}

fn delete_profile(params: &Value, config_path: Option<&Path>) -> CallResult {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| IpcError::invalid("params.name required"))?;
    let path = config_path_or_default(config_path)?;
    let mut cfg = Config::load_from(&path)?;
    let before = cfg.profiles.len();
    cfg.profiles.retain(|p| p.name != name);
    if cfg.profiles.len() == before {
        return Err(IpcError::Config(format!("profile '{name}' not found")));
    }
    cfg.save_to(&path)?;
    Ok(ok_unit())
}

fn preview_crop(params: &Value, config_path: Option<&Path>) -> CallResult {
    use superpanels_core::layout::{
        ImageRectMm, compute_crop_specs, cover_image_rect_mm, synthesise_placements,
    };

    let image = params
        .get("image")
        .and_then(Value::as_str)
        .ok_or_else(|| IpcError::invalid("params.image required"))?;
    let image_rect_mm: Option<ImageRectMm> = params
        .get("image_rect_mm")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let cfg = load_config(config_path)?;
    let canonical = canonicalise_inside_roots(Path::new(image), &cfg.library.roots)?;
    let dims = superpanels_core::image::read_dimensions(&canonical)
        .map_err(|e| IpcError::Image(e.to_string()))?;
    let mut monitors = detect(None)?;
    cfg.merge_into_monitors(&mut monitors);
    let placements = synthesise_placements(&monitors);
    let rect = image_rect_mm.unwrap_or_else(|| cover_image_rect_mm(&monitors, dims));
    let specs = compute_crop_specs(&monitors, &placements, dims, rect)?;
    Ok(ok_payload(specs))
}

/// In-process mirror of `daemon::handlers::library::canonicalise_inside_roots`.
/// Same fail-deny posture: empty roots reject; failure to
/// canonicalise `requested` rejects; a root that itself fails to canonicalise
/// is *silently skipped* via `is_ok_and`, so misconfigured/unreadable roots
/// shrink the allowlist instead of expanding it.
fn canonicalise_inside_roots(requested: &Path, roots: &[PathBuf]) -> Result<PathBuf, IpcError> {
    if roots.is_empty() {
        return Err(IpcError::invalid("library has no configured roots"));
    }
    let canonical = std::fs::canonicalize(requested)
        .map_err(|e| IpcError::invalid(format!("rejecting path '{}': {e}", requested.display())))?;
    let allowed = roots
        .iter()
        .any(|root| std::fs::canonicalize(root).is_ok_and(|c| canonical.starts_with(&c)));
    if !allowed {
        return Err(IpcError::invalid(format!(
            "path '{}' is outside the configured library roots",
            requested.display()
        )));
    }
    Ok(canonical)
}

fn library_list(params: &Value, config_path: Option<&Path>) -> CallResult {
    let filter: CoreLibraryFilter = params
        .get("filter")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let cfg = load_config(config_path)?;
    let mut all: Vec<superpanels_core::LibraryEntry> = Vec::new();
    for root in &cfg.library.roots {
        let entries = superpanels_core::scan_folder(root, cfg.library.recursive, |_| {});
        all.extend(entries);
    }
    let page = apply_library_filter(&all, &filter);
    Ok(ok_payload(page))
}

fn library_rescan(config_path: Option<&Path>) -> CallResult {
    let cfg = load_config(config_path)?;
    let mut count: usize = 0;
    for root in &cfg.library.roots {
        count += superpanels_core::scan_folder(root, cfg.library.recursive, |_| {}).len();
    }
    Ok(json!({ "count": count }))
}

fn library_thumbnail(params: &Value, config_path: Option<&Path>) -> CallResult {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| IpcError::invalid("params.path required"))?;
    let cfg = load_config(config_path)?;
    let edge = cfg.library.thumbnail_size.max(THUMBNAIL_MIN_EDGE);
    let canonical = canonicalise_inside_roots(Path::new(path), &cfg.library.roots)?;
    let img = superpanels_core::image::load_thumbnail(
        &canonical,
        edge,
        superpanels_core::image::Resample::Fast,
    )
    .map_err(|e| IpcError::Image(e.to_string()))?;
    let bytes =
        superpanels_core::image::encode_png(&img).map_err(|e| IpcError::Image(e.to_string()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(json!({ "data": encoded, "mime": "image/png" }))
}

fn get_config(config_path: Option<&Path>) -> CallResult {
    let cfg = load_config(config_path)?;
    Ok(ok_payload(cfg))
}

fn save_config(params: &Value, config_path: Option<&Path>) -> CallResult {
    let cfg_val = params
        .get("config")
        .ok_or_else(|| IpcError::invalid("params.config required"))?;
    let cfg: Config = serde_json::from_value(cfg_val.clone())
        .map_err(|e| IpcError::Config(format!("config is malformed: {e}")))?;
    let path = config_path_or_default(config_path)?;
    cfg.save_to(&path)
        .map_err(|e: ConfigError| IpcError::Config(e.to_string()))?;
    Ok(ok_unit())
}

fn set_monitor_physical_size(params: &Value, config_path: Option<&Path>) -> CallResult {
    let identifier = v::parse_monitor_identifier(params).map_err(|e| IpcError::invalid(e.0))?;
    let physical_mm = v::parse_physical_mm(params).map_err(|e| IpcError::invalid(e.0))?;
    let path = config_path_or_default(config_path)?;
    write_monitor_block(&path, &identifier, physical_mm)
        .map_err(|e| IpcError::Config(e.to_string()))?;
    Ok(ok_unit())
}

fn current_state() -> Value {
    // No daemon to ask, but the previous run's resume file still describes
    // what's on screen — compositors persist the wallpaper across sessions.
    let resume = superpanels_core::resume::resume_path()
        .and_then(|p| superpanels_core::resume::load(&p).ok().flatten());
    current_state_from(resume)
}

fn current_state_from(resume: Option<superpanels_core::resume::ResumeState>) -> Value {
    let (active, secs, backend) = match resume {
        Some(r) => (
            Value::from(r.active_profile),
            r.last_apply_unix_secs.map_or(Value::Null, Value::from),
            r.last_apply_backend.map_or(Value::Null, Value::from),
        ),
        None => (Value::Null, Value::Null, Value::Null),
    };
    json!({
        "version": superpanels_core::ipc::PROTOCOL_VERSION,
        "active_profile": active,
        "slideshow": Value::Null,
        "last_apply_unix_secs": secs,
        "last_apply_backend": backend,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::expect_used)] // reason: same
#[allow(clippy::panic)] // reason: same — explicit panic on unexpected enum branch
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn dispatch_unknown_method_returns_invalid_argument() {
        let err = dispatch("bogus", &Value::Null, None).unwrap_err();
        assert!(matches!(err, IpcError::InvalidArgument(_)));
    }

    #[test]
    fn list_profiles_returns_profiles_and_validity_object() {
        // Frontend types in `ui/src/lib/api.ts` expect a
        // `{ profiles: Profile[], validity: [...] }` object — same shape the
        // daemon's `cmd_list_profiles` returns. A bare array (the old
        // in-process shape) made `profileStore.refresh()` read undefined for
        // `resp.profiles` and silently break the GUI when no daemon was up.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let v = list_profiles(Some(&path)).unwrap();
        assert!(v.is_object(), "expected object, got {v}");
        assert!(v.get("profiles").is_some_and(Value::is_array));
        assert!(v.get("validity").is_some_and(Value::is_array));
    }

    #[test]
    fn save_profile_rejects_malformed_payload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let err = save_profile(&json!({"profile": "not-an-object"}), Some(&path)).unwrap_err();
        assert!(matches!(err, IpcError::InvalidArgument(_)));
    }

    #[test]
    fn delete_profile_missing_returns_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let err = delete_profile(&json!({"name": "ghost"}), Some(&path)).unwrap_err();
        assert!(matches!(err, IpcError::Config(_)));
    }

    #[test]
    fn current_state_without_resume_returns_null_fields() {
        let v = current_state_from(None);
        assert_eq!(v["active_profile"], Value::Null);
        assert_eq!(v["slideshow"], Value::Null);
        assert_eq!(v["last_apply_backend"], Value::Null);
    }

    #[test]
    fn current_state_reports_resumed_profile() {
        let v = current_state_from(Some(superpanels_core::resume::ResumeState {
            active_profile: "Lofi".to_owned(),
            last_apply_backend: Some("kde".to_owned()),
            last_apply_unix_secs: Some(1_781_213_342),
        }));
        assert_eq!(v["active_profile"], "Lofi");
        assert_eq!(v["last_apply_backend"], "kde");
        assert_eq!(v["last_apply_unix_secs"], 1_781_213_342);
        // No daemon means no live slideshow state, resumed or not.
        assert_eq!(v["slideshow"], Value::Null);
    }

    #[test]
    fn preview_crop_malformed_image_rect_falls_back_to_cover_fit() {
        // A malformed `image_rect_mm` should not surface as an InvalidArgument
        // from the parser — the handler silently falls back to a cover-fit
        // rect over the detected monitors. Failure modes (missing image, no
        // monitors) still surface as their own errors after that point.
        for malformed in [
            json!({"image": "/nope.png", "image_rect_mm": "junk"}),
            json!({"image": "/nope.png", "image_rect_mm": [1, 2, 3]}),
            json!({"image": "/nope.png", "image_rect_mm": null}),
        ] {
            let err = preview_crop(&malformed, None).unwrap_err();
            assert!(
                !matches!(&err, IpcError::InvalidArgument(m) if m.contains("image_rect_mm")),
                "expected silent fall-back to cover-fit but got: {err:?}"
            );
        }
    }

    #[test]
    fn library_thumbnail_rejects_path_outside_roots() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.library.roots = vec![dir.path().to_path_buf()];
        cfg.save_to(&path).unwrap();
        let err = library_thumbnail(&json!({"path": "/etc/passwd"}), Some(&path)).unwrap_err();
        assert!(matches!(err, IpcError::InvalidArgument(_)));
    }

    #[test]
    fn set_monitor_physical_size_rejects_above_cap() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let err = set_monitor_physical_size(
            &json!({
                "stable_id": "abc",
                "physical_mm": [1.0e30, 100.0],
            }),
            Some(&path),
        )
        .unwrap_err();
        let msg = match err {
            IpcError::InvalidArgument(m) => m,
            other => panic!("expected InvalidArgument, got {other:?}"),
        };
        assert!(msg.contains("must be in (0,"), "unexpected message: {msg}");
    }

    #[test]
    fn set_monitor_physical_size_rejects_oversize_stable_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let big_id = "x".repeat(v::MAX_MONITOR_ID_CHARS + 1);
        let err = set_monitor_physical_size(
            &json!({
                "stable_id": big_id,
                "physical_mm": [100.0, 100.0],
            }),
            Some(&path),
        )
        .unwrap_err();
        let msg = match err {
            IpcError::InvalidArgument(m) => m,
            other => panic!("expected InvalidArgument, got {other:?}"),
        };
        assert!(msg.contains("exceeds"), "unexpected message: {msg}");
    }

    #[test]
    fn set_monitor_physical_size_rejects_control_chars_in_name() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let err = set_monitor_physical_size(
            &json!({
                "name": "DP-1\nname=injected",
                "physical_mm": [100.0, 100.0],
            }),
            Some(&path),
        )
        .unwrap_err();
        let msg = match err {
            IpcError::InvalidArgument(m) => m,
            other => panic!("expected InvalidArgument, got {other:?}"),
        };
        assert!(msg.contains("control"), "unexpected message: {msg}");
    }

    #[test]
    fn set_monitor_physical_size_rejects_missing_identifier() {
        // Daemon-side coverage doesn't pin this branch; the in-process
        // mirror should reject when both `stable_id` and `name` are absent
        // (or empty) so the regression "wrote a [[monitor]] block keyed on
        // empty string" can't slip in.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();

        for params in [
            json!({"physical_mm": [100.0, 100.0]}),
            json!({"stable_id": "", "physical_mm": [100.0, 100.0]}),
            json!({"stable_id": "", "name": "", "physical_mm": [100.0, 100.0]}),
        ] {
            let err = set_monitor_physical_size(&params, Some(&path)).unwrap_err();
            assert!(
                matches!(&err, IpcError::InvalidArgument(m) if m.contains("stable_id") || m.contains("name")),
                "params {params}: expected identifier-required InvalidArgument, got {err:?}"
            );
        }
    }

    #[test]
    fn library_list_walks_all_configured_roots() {
        // The in-process `library_list` scans each `config.library.roots`
        // entry independently and concatenates. Pin that with a 2-root
        // fixture so a regression in the for-loop (e.g. "first root only")
        // shows up as a missing entry.
        let root_a = tempdir().unwrap();
        let root_b = tempdir().unwrap();
        let img_a = root_a.path().join("a.png");
        let img_b = root_b.path().join("b.png");
        let pixel = image::RgbImage::from_pixel(8, 8, image::Rgb([0, 0, 0]));
        pixel.save(&img_a).unwrap();
        pixel.save(&img_b).unwrap();

        let cfg_dir = tempdir().unwrap();
        let cfg_path = cfg_dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.library.roots = vec![root_a.path().to_path_buf(), root_b.path().to_path_buf()];
        cfg.save_to(&cfg_path).unwrap();

        let v = library_list(&Value::Null, Some(&cfg_path)).unwrap();
        let arr = v.as_array().expect("library_list returns array");
        let names: Vec<String> = arr
            .iter()
            .filter_map(|e| {
                e.get("path")
                    .and_then(Value::as_str)
                    .and_then(|p| std::path::Path::new(p).file_name()?.to_str())
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            names.contains(&"a.png".to_owned()),
            "missing root_a entry: {names:?}"
        );
        assert!(
            names.contains(&"b.png".to_owned()),
            "missing root_b entry: {names:?}"
        );
    }
}
