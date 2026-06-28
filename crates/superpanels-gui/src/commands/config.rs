//! Config get/save `#[tauri::command]`s.

#![allow(clippy::needless_pass_by_value)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use superpanels_core::Config;

use crate::bridge;
use crate::errors::IpcError;
use crate::state::AppState;

const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

#[tauri::command]
pub(crate) async fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, IpcError> {
    bridge::call_off_main("get_config", json!({}), state.config_path()).await
}

#[tauri::command]
pub(crate) async fn save_config(
    config: Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    bridge::call_off_main(
        "save_config",
        json!({ "config": config }),
        state.config_path(),
    )
    .await
}

/// Open the active `config.toml` in the user's default handler. The file is
/// materialised with defaults first when a fresh install hasn't saved one yet,
/// so there's always something to edit.
#[tauri::command]
pub(crate) async fn open_config_file(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value, IpcError> {
    let override_path = state.config_path();
    crate::commands::run_off_main(move || {
        let path = match override_path {
            Some(p) => p,
            None => Config::default_path()?,
        };
        if !path.exists() {
            Config::load_or_default_from(&path)?.save_to(&path)?;
        }
        open_in_default_app(&path)?;
        Ok(json!({}))
    })
    .await
}

/// Spawn `xdg-open <path>`, bounded by [`OPEN_TIMEOUT`], capturing stderr.
fn open_in_default_app(path: &Path) -> Result<(), IpcError> {
    let mut child = Command::new("xdg-open")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| IpcError::internal(format!("could not launch xdg-open: {e}")))?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let stderr = child
                    .wait_with_output()
                    .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_owned())
                    .unwrap_or_default();
                return Err(IpcError::internal(format!(
                    "xdg-open exited with {}: {stderr}",
                    status.code().unwrap_or(-1)
                )));
            }
            Ok(None) => {
                if started.elapsed() >= OPEN_TIMEOUT {
                    let _ = child.kill();
                    return Err(IpcError::internal("xdg-open timed out"));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(IpcError::internal(format!("waiting on xdg-open: {e}"))),
        }
    }
}
