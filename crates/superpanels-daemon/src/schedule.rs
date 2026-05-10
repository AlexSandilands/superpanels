//! Schedule evaluation. The daemon ticks once per minute, checks
//! `Config.schedules` against wall time, and dispatches `apply_profile` for
//! any rule that matches the current minute. Schedules are top-level
//! (`docs/spec/09-profiles-schedules.md`) and reference profiles by name.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use chrono::NaiveDate;
use chrono::{DateTime, Local, TimeZone, Timelike, Utc};
use superpanels_core::config::Config;
use superpanels_core::ipc::{IpcRequest, PROTOCOL_VERSION};
use superpanels_core::schedule::{Schedule, Trigger};
use tokio::sync::{Mutex, watch};
use tracing::{debug, warn};

use crate::server;
use crate::state::DaemonState;

/// Tracks per-rule (keyed by `display_name` ∥ index) last-fire timestamps so
/// each rule fires at most once per trigger window.
pub(crate) struct ScheduleChecker {
    last_fire: HashMap<String, i64>,
}

impl ScheduleChecker {
    pub(crate) fn new() -> Self {
        Self {
            last_fire: HashMap::new(),
        }
    }

    pub(crate) fn check(&mut self, cfg: &Config) -> Vec<String> {
        if cfg.schedules_paused {
            return Vec::new();
        }
        self.check_at(Local::now(), cfg)
    }

    pub(crate) fn check_at(&mut self, now: DateTime<Local>, cfg: &Config) -> Vec<String> {
        if cfg.schedules_paused {
            return Vec::new();
        }
        let mut to_apply = Vec::new();
        for (i, rule) in cfg.schedules.iter().enumerate() {
            if !rule.enabled {
                continue;
            }
            let key = rule_key(rule, i);
            let last = self.last_fire.get(&key).copied();
            if rule_should_fire(rule, now, last) {
                debug!(profile = %rule.profile, "schedule firing");
                to_apply.push(rule.profile.clone());
                self.last_fire.insert(key, now.timestamp());
            }
        }
        to_apply
    }

    /// Boot catch-up: find the most recent past fire across all enabled rules
    /// today and return the corresponding profile name. If none found, returns
    /// `None`. The caller should compare to the active profile and apply only
    /// if different. Methodised on `ScheduleChecker` for symmetry with
    /// `check`; doesn't touch the last-fire map (that's a runtime concern).
    #[must_use]
    #[allow(clippy::unused_self)] // reason: kept on `Self` for symmetry with `check`
    pub(crate) fn boot_catch_up(&self, cfg: &Config) -> Option<String> {
        if cfg.schedules_paused {
            return None;
        }
        let now = Local::now();
        let mut best: Option<(DateTime<Local>, String)> = None;
        for rule in &cfg.schedules {
            if !rule.enabled {
                continue;
            }
            let Some(fire_at) = last_fire_today(rule, now) else {
                continue;
            };
            if fire_at > now {
                continue;
            }
            match &best {
                Some((t, _)) if *t >= fire_at => {}
                _ => best = Some((fire_at, rule.profile.clone())),
            }
        }
        best.map(|(_, p)| p)
    }
}

fn rule_key(rule: &Schedule, idx: usize) -> String {
    rule.display_name
        .clone()
        .unwrap_or_else(|| format!("schedule[{idx}]"))
}

pub(crate) async fn run_schedule_checker(
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) {
    loop {
        tokio::time::sleep(sleep_to_next_minute(&Local::now())).await;
        let to_apply = {
            let mut guard = state.lock().await;
            let cfg = guard.config.clone();
            guard.schedule_checker.check(&cfg)
        };
        for profile_name in to_apply {
            let req = IpcRequest {
                v: PROTOCOL_VERSION,
                method: "apply_profile".to_owned(),
                params: serde_json::json!({"name": profile_name}),
            };
            let resp = server::dispatch_for_tests(req, Arc::clone(&state), timer_tx.clone()).await;
            if !resp.is_ok() {
                warn!(
                    profile = %profile_name,
                    error = ?resp.error,
                    "scheduled profile apply failed"
                );
            }
        }
    }
}

/// Time until the next wall-clock minute boundary, plus a 250 ms grace so
/// `Local::now()` after the sleep is reliably *past* the boundary (otherwise
/// scheduling jitter could leave us at HH:MM:59.998 and miss the rule).
/// Aligning each iteration to the wall clock keeps `Daily { HH, MM }` rules
/// firing at HH:MM:00.x rather than at the daemon's startup phase.
fn sleep_to_next_minute<Tz: TimeZone>(now: &DateTime<Tz>) -> Duration {
    let secs_to_next: u32 = 60 - now.second();
    let ms_to_next: u64 = u64::from(secs_to_next) * 1000 - u64::from(now.timestamp_subsec_millis());
    Duration::from_millis(ms_to_next + 250)
}

// --- predicates ---

fn rule_should_fire(rule: &Schedule, now: DateTime<Local>, last_secs: Option<i64>) -> bool {
    match &rule.trigger {
        Trigger::Daily { hour, minute } => daily_should_fire(*hour, *minute, now, last_secs),
        Trigger::Cron { expr } => cron_should_fire(expr, now.with_timezone(&Utc), last_secs),
    }
}

fn last_fire_today(rule: &Schedule, now: DateTime<Local>) -> Option<DateTime<Local>> {
    if !rule.enabled {
        return None;
    }
    let today = now.date_naive();
    match &rule.trigger {
        Trigger::Daily { hour, minute } => {
            let naive = today.and_hms_opt(u32::from(*hour), u32::from(*minute), 0)?;
            Local.from_local_datetime(&naive).single()
        }
        Trigger::Cron { expr } => {
            let sched = cron::Schedule::from_str(expr).ok()?;
            let day_start_local = Local
                .from_local_datetime(&today.and_hms_opt(0, 0, 0)?)
                .single()?;
            let day_start_utc = day_start_local.with_timezone(&Utc);
            let now_utc = now.with_timezone(&Utc);
            sched
                .after(&day_start_utc)
                .take_while(|t| *t <= now_utc)
                .last()
                .map(|t| t.with_timezone(&Local))
        }
    }
}

fn daily_should_fire(hour: u8, minute: u8, now: DateTime<Local>, last_secs: Option<i64>) -> bool {
    if u32::from(hour) != now.hour() || u32::from(minute) != now.minute() {
        return false;
    }
    match last_secs {
        None => true,
        Some(t) => now.timestamp() - t > 60,
    }
}

fn cron_should_fire(expr: &str, now: DateTime<Utc>, last_secs: Option<i64>) -> bool {
    let Ok(sched) = cron::Schedule::from_str(expr) else {
        warn!(%expr, "invalid cron expression; schedule will not fire");
        return false;
    };
    let since = match last_secs {
        Some(t) => DateTime::from_timestamp(t, 0).unwrap_or(now - chrono::TimeDelta::seconds(65)),
        None => now - chrono::TimeDelta::seconds(65),
    };
    sched.after(&since).next().is_some_and(|t| t <= now)
}

// Conflict detection lives in `superpanels_core::schedule`. Re-exported as a
// crate-private alias so handlers can keep their existing call site shape.
pub(crate) fn detect_same_minute_collision(cfg: &Config) -> Option<(usize, usize)> {
    superpanels_core::schedule::detect_same_minute_collision(&cfg.schedules)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use superpanels_core::schedule::Trigger;

    fn rule(profile: &str, trigger: Trigger) -> Schedule {
        Schedule {
            display_name: None,
            profile: profile.to_owned(),
            trigger,
            enabled: true,
        }
    }

    fn fixed_local(hour: u32, minute: u32) -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2025, 6, 21)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        Local.from_local_datetime(&naive).single().unwrap()
    }

    fn local_at(hour: u32, minute: u32, second: u32, milli: u32) -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2025, 6, 21)
            .unwrap()
            .and_hms_milli_opt(hour, minute, second, milli)
            .unwrap();
        Local.from_local_datetime(&naive).single().unwrap()
    }

    #[test]
    fn sleep_to_next_minute_lands_past_the_boundary() {
        // Mid-minute: ~30s + 250ms grace.
        let d = sleep_to_next_minute(&local_at(11, 1, 30, 0));
        assert_eq!(d, Duration::from_millis(30_000 + 250));
        // Just before the boundary: short sleep + 250ms grace.
        let d = sleep_to_next_minute(&local_at(11, 1, 59, 750));
        assert_eq!(d, Duration::from_millis(250 + 250));
        // Right on the boundary: full minute (we still wait, then re-check).
        let d = sleep_to_next_minute(&local_at(11, 1, 0, 0));
        assert_eq!(d, Duration::from_millis(60_000 + 250));
    }

    #[test]
    fn daily_fires_at_matching_minute() {
        let mut checker = ScheduleChecker::new();
        let mut cfg = Config::default();
        cfg.schedules.push(rule(
            "dark",
            Trigger::Daily {
                hour: 18,
                minute: 0,
            },
        ));
        let result = checker.check_at(fixed_local(18, 0), &cfg);
        assert_eq!(result, vec!["dark".to_owned()]);
    }

    #[test]
    fn schedules_paused_blocks_fire() {
        let mut checker = ScheduleChecker::new();
        let mut cfg = Config {
            schedules_paused: true,
            ..Config::default()
        };
        cfg.schedules.push(rule(
            "dark",
            Trigger::Daily {
                hour: 18,
                minute: 0,
            },
        ));
        let result = checker.check_at(fixed_local(18, 0), &cfg);
        assert!(result.is_empty());
    }

    #[test]
    fn detect_same_minute_collision_finds_conflict() {
        let mut cfg = Config::default();
        cfg.schedules
            .push(rule("a", Trigger::Daily { hour: 8, minute: 0 }));
        cfg.schedules
            .push(rule("b", Trigger::Daily { hour: 8, minute: 0 }));
        assert_eq!(detect_same_minute_collision(&cfg), Some((0, 1)));
    }
}
