//! `current_state` `#[tauri::command]` plus the helper that parses the
//! daemon's `current_state` payload into our [`RuntimeSnapshot`].

#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::bridge;
use crate::errors::IpcError;
use crate::state::{AppState, RuntimeSnapshot};

#[tauri::command]
pub(crate) async fn current_state(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let v = bridge::call_off_main("current_state", json!({}), state.config_path()).await?;
    state.set_snapshot(parse_runtime_snapshot(&v));
    Ok(v)
}

fn parse_runtime_snapshot(v: &Value) -> RuntimeSnapshot {
    let active_profile = v
        .get("active_profile")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let paused = v
        .get("slideshow")
        .and_then(|s| s.get("paused"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current_filename = v
        .get("current_filename")
        .and_then(Value::as_str)
        .map(str::to_owned);
    RuntimeSnapshot {
        active_profile,
        current_filename,
        paused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_snapshot_handles_minimal_payload() {
        let v = json!({"active_profile": "home", "slideshow": {"paused": true}});
        let snap = parse_runtime_snapshot(&v);
        assert_eq!(snap.active_profile.as_deref(), Some("home"));
        assert!(snap.paused);
    }

    #[test]
    fn parse_runtime_snapshot_defaults_when_fields_missing() {
        let v = json!({});
        let snap = parse_runtime_snapshot(&v);
        assert!(snap.active_profile.is_none());
        assert!(!snap.paused);
    }
}
