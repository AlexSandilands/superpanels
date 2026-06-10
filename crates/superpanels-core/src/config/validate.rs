//! Field-level validation for [`super::Config`].

use std::collections::HashSet;
use std::path::Path;

use crate::ipc::validate as v;

use super::{Config, ConfigError};

/// Bails on the first failure for a focused error message.
pub(super) fn validate(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    validate_monitors(cfg, path)?;
    validate_profiles(cfg, path)?;
    validate_general(cfg, path)?;
    Ok(())
}

fn invalid(path: &Path, field: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        path: path.to_owned(),
        field: field.into(),
        message: message.into(),
    }
}

fn validate_monitors(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    if cfg.monitors.len() > v::MAX_MONITORS {
        return Err(invalid(
            path,
            "monitor",
            format!(
                "{} entries exceeds {} cap",
                cfg.monitors.len(),
                v::MAX_MONITORS
            ),
        ));
    }
    for (i, m) in cfg.monitors.iter().enumerate() {
        if m.stable_id.is_none() && m.name.is_none() {
            return Err(invalid(
                path,
                format!("monitor[{i}]"),
                "at least one of `stable_id` or `name` must be set",
            ));
        }
        if let Some(id) = &m.stable_id {
            v::validate_monitor_id_string(id, "stable_id")
                .map_err(|e| invalid(path, format!("monitor[{i}].stable_id"), e.0))?;
        }
        if let Some(name) = &m.name {
            v::validate_monitor_id_string(name, "name")
                .map_err(|e| invalid(path, format!("monitor[{i}].name"), e.0))?;
        }
        v::validate_physical_mm(m.physical_mm)
            .map_err(|e| invalid(path, format!("monitor[{i}].physical_mm"), e.0))?;
    }
    Ok(())
}

fn validate_profiles(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    if cfg.profiles.len() > v::MAX_PROFILES {
        return Err(invalid(
            path,
            "profile",
            format!(
                "{} entries exceeds {} cap",
                cfg.profiles.len(),
                v::MAX_PROFILES
            ),
        ));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for (i, p) in cfg.profiles.iter().enumerate() {
        v::validate_profile_name(&p.name)
            .map_err(|e| invalid(path, format!("profile[{i}].name"), e.0))?;
        if !seen.insert(p.name.as_str()) {
            return Err(invalid(
                path,
                format!("profile[{i}].name"),
                format!("duplicate profile name `{}`", p.name),
            ));
        }
        for (key, placement) in &p.monitor_state {
            if !placement.x_mm.is_finite() || !placement.y_mm.is_finite() {
                return Err(invalid(
                    path,
                    format!("profile[{i}].monitor_state.{key}"),
                    "x_mm and y_mm must be finite",
                ));
            }
        }
        validate_profile_body(path, i, &p.body)?;
    }
    Ok(())
}

fn validate_profile_body(
    path: &Path,
    i: usize,
    body: &super::ProfileBody,
) -> Result<(), ConfigError> {
    if let super::ProfileBody::Span(span) = body {
        if let super::SpanSource::Slideshow {
            images, overrides, ..
        } = &span.source
        {
            if images.sources.len() > v::MAX_SLIDESHOW_IMAGES {
                return Err(invalid(
                    path,
                    format!("profile[{i}].body.source.images.sources"),
                    format!(
                        "{} entries exceeds {} cap",
                        images.sources.len(),
                        v::MAX_SLIDESHOW_IMAGES
                    ),
                ));
            }
            if overrides.len() > v::MAX_IMAGE_OVERRIDES {
                return Err(invalid(
                    path,
                    format!("profile[{i}].body.source.overrides"),
                    format!(
                        "{} entries exceeds {} cap",
                        overrides.len(),
                        v::MAX_IMAGE_OVERRIDES
                    ),
                ));
            }
            for (image, o) in overrides {
                let finite_placements = o
                    .monitor_state
                    .values()
                    .all(|p| p.x_mm.is_finite() && p.y_mm.is_finite());
                let r = o.image_rect_mm;
                let finite_rect = [r.x_mm, r.y_mm, r.w_mm, r.h_mm]
                    .iter()
                    .all(|v| v.is_finite());
                if !finite_placements || !finite_rect {
                    return Err(invalid(
                        path,
                        format!("profile[{i}].body.source.overrides.{}", image.display()),
                        "placements and image_rect_mm must be finite",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_general(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    match cfg.general.notifications.as_str() {
        "off" | "errors" | "all" => {}
        other => {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: "general.notifications".to_owned(),
                message: format!("expected `off`, `errors`, or `all`, got `{other}`"),
            });
        }
    }
    match cfg.general.theme.as_str() {
        "auto" | "light" | "dark" => {}
        other => {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: "general.theme".to_owned(),
                message: format!("expected `auto`, `light`, or `dark`, got `{other}`"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::config::{
        ImageOverride, ImageSet, Profile, ProfileBody, SlideshowConfig, SlideshowSort,
        SlideshowStart, SpanProfile, SpanSource,
    };
    use crate::layout::ImageRectMm;
    use crate::schedule::{MonitorPlacement, TopologyFingerprint};

    use super::*;

    fn slideshow_profile_with_overrides(
        overrides: HashMap<PathBuf, ImageOverride>,
    ) -> super::super::Config {
        let now = crate::config::now_timestamp();
        let profile = Profile {
            name: "show".to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Slideshow {
                    images: ImageSet::from_folder(PathBuf::from("/walls"), false),
                    config: SlideshowConfig {
                        interval: std::time::Duration::from_secs(600),
                        sort: SlideshowSort::Shuffle,
                        recent_history_size: 10,
                        on_start: SlideshowStart::Resume,
                        pause_when_active: false,
                        skip_on_unavailable: true,
                    },
                    overrides,
                    uniform_layout: false,
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
        };
        Config {
            profiles: vec![profile],
            ..Config::default()
        }
    }

    fn finite_override() -> ImageOverride {
        ImageOverride {
            monitor_state: HashMap::from([(
                "uuid-a".to_owned(),
                MonitorPlacement {
                    x_mm: 0.0,
                    y_mm: 0.0,
                },
            )]),
            image_rect_mm: ImageRectMm {
                x_mm: 0.0,
                y_mm: 0.0,
                w_mm: 100.0,
                h_mm: 100.0,
            },
        }
    }

    #[test]
    fn overrides_within_cap_pass_validation() {
        let overrides = HashMap::from([(PathBuf::from("/walls/a.png"), finite_override())]);
        let cfg = slideshow_profile_with_overrides(overrides);
        assert!(validate(&cfg, Path::new("/x/config.toml")).is_ok());
    }

    #[test]
    fn overrides_above_cap_are_rejected() {
        let overrides: HashMap<PathBuf, ImageOverride> = (0..=v::MAX_IMAGE_OVERRIDES)
            .map(|i| (PathBuf::from(format!("/walls/{i}.png")), finite_override()))
            .collect();
        let cfg = slideshow_profile_with_overrides(overrides);
        let err = validate(&cfg, Path::new("/x/config.toml")).unwrap_err();
        assert!(err.to_string().contains("exceeds"), "got: {err}");
    }

    #[test]
    fn non_finite_override_rect_is_rejected() {
        let mut bad = finite_override();
        bad.image_rect_mm.w_mm = f32::NAN;
        let overrides = HashMap::from([(PathBuf::from("/walls/a.png"), bad)]);
        let cfg = slideshow_profile_with_overrides(overrides);
        let err = validate(&cfg, Path::new("/x/config.toml")).unwrap_err();
        assert!(err.to_string().contains("finite"), "got: {err}");
    }

    #[test]
    fn non_finite_override_placement_is_rejected() {
        let mut bad = finite_override();
        bad.monitor_state.insert(
            "uuid-b".to_owned(),
            MonitorPlacement {
                x_mm: f32::INFINITY,
                y_mm: 0.0,
            },
        );
        let overrides = HashMap::from([(PathBuf::from("/walls/a.png"), bad)]);
        let cfg = slideshow_profile_with_overrides(overrides);
        let err = validate(&cfg, Path::new("/x/config.toml")).unwrap_err();
        assert!(err.to_string().contains("finite"), "got: {err}");
    }
}
