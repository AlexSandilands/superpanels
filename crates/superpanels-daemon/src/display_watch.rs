//! OS-rotation push: subscribe to the KDE Plasma kscreen kded D-Bus signal
//! on KDE sessions and re-detect on emission. Best-effort — on Plasma 6
//! Wayland the kded module is often unloaded and no signal arrives; users
//! drive a manual re-detect from Settings > Monitors (F5) in that case.
//!
//! `publish` updates `DaemonState.monitors` and broadcasts a `()` tick on
//! `monitors_tx` so the GUI's `wait_for_monitor_change` long-poll can
//! deliver a Tauri event.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use superpanels_core::display::Monitor;
use tokio::sync::{Mutex, broadcast};
use tokio::time::{Instant, sleep_until};
use tracing::{debug, info, warn};
use zbus::{Connection, MatchRule, MessageStream, message::Type as MessageType};

use crate::state::DaemonState;

/// Match rule for any signal on `org.kde.KScreen`. We deliberately don't pin
/// a member name — KDE has shipped both `configChanged` and other property-
/// change-style signals across Plasma versions; matching the interface is
/// stable and the cost of a spurious wake-up is one re-detect.
fn build_match_rule() -> Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface("org.kde.KScreen")?
        .build())
}

const DEBOUNCE: Duration = Duration::from_millis(250);

/// True when the current session looks like KDE Plasma. Mirrors the env
/// check in `KscreenDoctorDetector::availability`; we don't depend on the
/// detector type because that lives in a private module in -core.
pub(crate) fn kde_session_present() -> bool {
    if std::env::var_os("KDE_FULL_SESSION").is_some() {
        return true;
    }
    std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|d| d.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
}

/// Spawn the OS-rotation watch task. On KDE sessions, subscribes to the
/// kscreen D-Bus signal as a best-effort push path. On non-KDE sessions or
/// stacks where the signal doesn't fire, the GUI falls back to manual
/// re-detect (Settings > Monitors, F5).
pub(crate) fn spawn(state: Arc<Mutex<DaemonState>>, monitors_tx: broadcast::Sender<()>) {
    if !kde_session_present() {
        debug!("non-KDE session; OS-rotation push disabled, use manual refresh");
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = run_dbus(state, monitors_tx).await {
            warn!(error = %e, "KDE display-watch exited; OS-rotation push disabled");
        }
    });
}

async fn run_dbus(
    state: Arc<Mutex<DaemonState>>,
    monitors_tx: broadcast::Sender<()>,
) -> Result<()> {
    let conn = Connection::session().await?;
    let rule = build_match_rule()?;
    let mut stream = MessageStream::for_match_rule(rule, &conn, Some(8)).await?;
    info!("subscribed to org.kde.KScreen for OS-rotation push");

    drive_stream(&mut stream, &state, &monitors_tx).await;
    Ok(())
}

async fn drive_stream(
    stream: &mut MessageStream,
    state: &Arc<Mutex<DaemonState>>,
    monitors_tx: &broadcast::Sender<()>,
) {
    while let Some(item) = stream.next().await {
        match item {
            Ok(_) => debounce_and_redetect(stream, state, monitors_tx).await,
            Err(e) => {
                warn!(error = %e, "kscreen signal stream error; stopping watch");
                return;
            }
        }
    }
}

async fn debounce_and_redetect(
    stream: &mut MessageStream,
    state: &Arc<Mutex<DaemonState>>,
    monitors_tx: &broadcast::Sender<()>,
) {
    let mut deadline = Instant::now() + DEBOUNCE;
    loop {
        tokio::select! {
            biased;
            item = stream.next() => match item {
                Some(Ok(_)) => deadline = Instant::now() + DEBOUNCE,
                Some(Err(e)) => {
                    warn!(error = %e, "kscreen signal during debounce; aborting redetect");
                    return;
                }
                None => break,
            },
            () = sleep_until(deadline) => break,
        }
    }
    redetect_and_publish(state, monitors_tx).await;
}

async fn redetect_and_publish(
    state: &Arc<Mutex<DaemonState>>,
    monitors_tx: &broadcast::Sender<()>,
) {
    let detected = match superpanels_core::detect(None) {
        Ok(monitors) => monitors,
        Err(e) => {
            warn!(error = %e, "OS-driven redetect failed");
            return;
        }
    };
    publish(detected, state, monitors_tx).await;
}

/// Swap `state.monitors` for `detected` (after merging per-monitor config) and
/// broadcast a tick. Split out so tests can drive the publish path with a
/// synthetic monitor list, without spawning real detector subprocesses.
async fn publish(
    mut detected: Vec<Monitor>,
    state: &Arc<Mutex<DaemonState>>,
    monitors_tx: &broadcast::Sender<()>,
) {
    let count = {
        let mut guard = state.lock().await;
        guard.config.merge_into_monitors(&mut detected);
        let n = detected.len();
        guard.monitors = detected;
        n
    };
    debug!(monitors = count, "OS-driven monitor snapshot updated");
    // `send` errors when no subscribers exist — that's fine.
    let _ = monitors_tx.send(());
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
mod tests {
    use super::*;
    use superpanels_core::config::Config;
    use superpanels_core::display::{MonitorId, Rotation};

    fn synth_monitor(name: &str) -> Monitor {
        Monitor {
            id: MonitorId(0),
            name: name.to_owned(),
            stable_id: Some(format!("synth-{name}")),
            position: (0, 0),
            resolution: (1920, 1080),
            physical_size_mm: Some((527.0, 296.0)),
            scale: 1.0,
            rotation: Rotation::None,
            refresh_hz: None,
            primary: true,
            ppi: None,
        }
    }

    #[test]
    fn build_match_rule_targets_kscreen_signals() {
        let rule = build_match_rule().unwrap();
        let s = rule.to_string();
        assert!(s.contains("type='signal'"), "rule must filter signals: {s}");
        assert!(
            s.contains("interface='org.kde.KScreen'"),
            "rule must target KScreen interface: {s}"
        );
    }

    #[tokio::test]
    async fn publish_delivers_tick_to_live_subscriber() {
        let state = Arc::new(Mutex::new(DaemonState::for_tests(Config::default())));
        let (tx, mut rx) = broadcast::channel::<()>(4);

        publish(vec![synth_monitor("DP-1")], &state, &tx).await;

        assert!(
            rx.try_recv().is_ok(),
            "live subscriber must receive a tick after publish"
        );
        let guard = state.lock().await;
        assert_eq!(guard.monitors.len(), 1);
        assert_eq!(guard.monitors[0].name, "DP-1");
    }

    #[tokio::test]
    async fn publish_is_a_noop_when_no_subscribers() {
        let state = Arc::new(Mutex::new(DaemonState::for_tests(Config::default())));
        let (tx, rx) = broadcast::channel::<()>(4);
        drop(rx);

        // Must not panic when send returns the no-subscribers error.
        publish(vec![synth_monitor("HDMI-A-1")], &state, &tx).await;

        let guard = state.lock().await;
        assert_eq!(
            guard.monitors.len(),
            1,
            "monitors snapshot is updated regardless of subscriber count"
        );
    }
}
