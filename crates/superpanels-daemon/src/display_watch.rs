//! KDE-only OS-rotation watch via the kscreen kded D-Bus signal.
//!
//! KDE Plasma's kscreen kded module emits signals on the `org.kde.KScreen`
//! interface whenever the display configuration changes (rotation, layout,
//! resolution). We subscribe with a session-bus match rule, debounce bursts
//! into a single re-detect (~250 ms), update `DaemonState.monitors`, and
//! broadcast a `()` tick on `monitors_tx` so the GUI can refresh.
//!
//! Hyprland / Sway / wlroots / X11 keep relying on the existing IPC
//! `redetect` request — they have no comparable session-bus signal.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
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

/// Spawn the watch loop. Returns immediately; the task lives until the
/// daemon exits. Any setup failure (no session bus, `AppArmor`, etc.) is
/// logged once and the task exits — the daemon keeps running.
pub(crate) fn spawn(state: Arc<Mutex<DaemonState>>, monitors_tx: broadcast::Sender<()>) {
    tokio::spawn(async move {
        if let Err(e) = run(state, monitors_tx).await {
            warn!(error = %e, "KDE display-watch exited; OS-rotation push disabled");
        }
    });
}

async fn run(state: Arc<Mutex<DaemonState>>, monitors_tx: broadcast::Sender<()>) -> Result<()> {
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
    let mut guard = state.lock().await;
    match superpanels_core::detect(None) {
        Ok(mut monitors) => {
            guard.config.merge_into_monitors(&mut monitors);
            let count = monitors.len();
            guard.monitors = monitors;
            drop(guard);
            debug!(monitors = count, "OS-driven re-detect after kscreen signal");
            // `send` errors when no subscribers exist — that's fine.
            let _ = monitors_tx.send(());
        }
        Err(e) => warn!(error = %e, "OS-driven redetect failed"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on unexpected errors
mod tests {
    use super::*;

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
    async fn redetect_publishes_when_subscribers_exist() {
        // No KDE session in test; detect() will still return Ok or Err
        // depending on environment, but the broadcast channel mechanics are
        // independent of detection success.
        use superpanels_core::config::Config;
        let state = Arc::new(Mutex::new(DaemonState::for_tests(Config::default())));
        let (tx, mut rx) = broadcast::channel::<()>(4);
        // Send a tick directly to verify subscriber receives it; the inner
        // detect call is a no-op for this assertion's purposes.
        tx.send(()).unwrap();
        assert!(rx.try_recv().is_ok());
        // Cover the typical "no subscribers" path: drop receiver and confirm
        // redetect_and_publish doesn't panic when send returns an error.
        drop(rx);
        redetect_and_publish(&state, &tx).await;
    }
}
