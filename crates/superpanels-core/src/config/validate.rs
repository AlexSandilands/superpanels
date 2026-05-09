//! Field-level validation for [`super::Config`] (`SPEC.md` §14.2).

use std::collections::HashSet;
use std::path::Path;

use super::{Config, ConfigError};

fn is_valid_mm(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// Bails on the first failure for a focused error message.
pub(super) fn validate(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    validate_monitors(cfg, path)?;
    validate_profiles(cfg, path)?;
    validate_general(cfg, path)?;
    Ok(())
}

fn validate_monitors(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    for (i, m) in cfg.monitors.iter().enumerate() {
        if m.stable_id.is_none() && m.name.is_none() {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: format!("monitor[{i}]"),
                message: "at least one of `stable_id` or `name` must be set".to_owned(),
            });
        }
        if !is_valid_mm(m.physical_mm[0]) || !is_valid_mm(m.physical_mm[1]) {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: format!("monitor[{i}].physical_mm"),
                message: "values must be finite and > 0".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_profiles(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    let mut seen: HashSet<&str> = HashSet::new();
    for (i, p) in cfg.profiles.iter().enumerate() {
        if p.name.trim().is_empty() {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: format!("profile[{i}].name"),
                message: "profile name must be non-empty".to_owned(),
            });
        }
        if !seen.insert(p.name.as_str()) {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: format!("profile[{i}].name"),
                message: format!("duplicate profile name `{}`", p.name),
            });
        }
        if p.bezels.horizontal_mm < 0.0 || p.bezels.vertical_mm < 0.0 {
            return Err(ConfigError::Invalid {
                path: path.to_owned(),
                field: format!("profile[{i}].bezels"),
                message: "bezel values must be non-negative".to_owned(),
            });
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
