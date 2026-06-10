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
        if let super::SpanSource::Slideshow { images, .. } = &span.source {
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
