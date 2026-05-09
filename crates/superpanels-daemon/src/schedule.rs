//! Schedule evaluation. The daemon ticks once per minute, checks
//! `Config.schedules` against wall time, and dispatches `apply_profile` for
//! any rule that matches the current minute. Schedules are top-level
//! (`docs/spec/09-profiles-schedules.md`) and reference profiles by name.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Timelike, Utc};
use superpanels_core::config::Config;
use superpanels_core::ipc::{IpcRequest, PROTOCOL_VERSION};
use superpanels_core::schedule::{LatLong, Schedule, Trigger, sun_event_utc_minutes};
use tokio::sync::{Mutex, watch};
use tokio::time::MissedTickBehavior;
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
            if rule_should_fire(rule, cfg.location, now, last) {
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
            let Some(fire_at) = last_fire_today(rule, cfg.location, now) else {
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
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval.tick().await;

    loop {
        interval.tick().await;
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

// --- predicates ---

fn rule_should_fire(
    rule: &Schedule,
    location: Option<LatLong>,
    now: DateTime<Local>,
    last_secs: Option<i64>,
) -> bool {
    match &rule.trigger {
        Trigger::Daily { hour, minute } => daily_should_fire(*hour, *minute, now, last_secs),
        Trigger::Sunset { offset_minutes } => sun_should_fire(
            *offset_minutes,
            location,
            false,
            now,
            last_secs,
            &rule.profile,
        ),
        Trigger::Sunrise { offset_minutes } => sun_should_fire(
            *offset_minutes,
            location,
            true,
            now,
            last_secs,
            &rule.profile,
        ),
        Trigger::Cron { expr } => cron_should_fire(expr, now.with_timezone(&Utc), last_secs),
    }
}

fn last_fire_today(
    rule: &Schedule,
    location: Option<LatLong>,
    now: DateTime<Local>,
) -> Option<DateTime<Local>> {
    if !rule.enabled {
        return None;
    }
    let today = now.date_naive();
    match &rule.trigger {
        Trigger::Daily { hour, minute } => {
            let naive = today.and_hms_opt(u32::from(*hour), u32::from(*minute), 0)?;
            Local.from_local_datetime(&naive).single()
        }
        Trigger::Sunset { offset_minutes } | Trigger::Sunrise { offset_minutes } => sun_fire_local(
            *offset_minutes,
            location,
            matches!(rule.trigger, Trigger::Sunrise { .. }),
            today,
        ),
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

fn sun_should_fire(
    offset_min: i32,
    location: Option<LatLong>,
    is_sunrise: bool,
    now: DateTime<Local>,
    last_secs: Option<i64>,
    profile: &str,
) -> bool {
    let Some(loc) = location else {
        warn!(
            profile,
            "sun-event schedule skipped: no `location` configured at top level"
        );
        return false;
    };
    let today = now.date_naive();
    let Some(fire_local) = sun_fire_local(offset_min, Some(loc), is_sunrise, today) else {
        return false;
    };
    if fire_local.hour() != now.hour() || fire_local.minute() != now.minute() {
        return false;
    }
    match last_secs {
        None => true,
        Some(t) => now.timestamp() - t > 60,
    }
}

fn sun_fire_local(
    offset_min: i32,
    location: Option<LatLong>,
    is_sunrise: bool,
    today: NaiveDate,
) -> Option<DateTime<Local>> {
    let loc = location?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    let date_days = today.signed_duration_since(epoch).num_days();
    let event_min = sun_event_utc_minutes(loc, date_days, is_sunrise)?;
    let fire_min = i64::from(event_min) + i64::from(offset_min);
    let fire_naive_utc = today.and_hms_opt(0, 0, 0)? + chrono::TimeDelta::minutes(fire_min);
    Some(Utc.from_utc_datetime(&fire_naive_utc).with_timezone(&Local))
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

// --- conflict detection ---

/// `Some((idx_a, idx_b))` for the first pair of enabled schedules that fire at
/// the same wall-clock minute on a representative day. Used by the settings
/// UI to block save when two rules collide.
#[must_use]
pub(crate) fn detect_same_minute_collision(cfg: &Config) -> Option<(usize, usize)> {
    let probe = Local::now().date_naive();
    let mut seen: HashMap<(u8, u8), usize> = HashMap::new();
    for (i, rule) in cfg.schedules.iter().enumerate() {
        if !rule.enabled {
            continue;
        }
        let Some(hm) = representative_minute(rule, cfg.location, probe) else {
            continue;
        };
        if let Some(&j) = seen.get(&hm) {
            return Some((j, i));
        }
        seen.insert(hm, i);
    }
    None
}

fn representative_minute(
    rule: &Schedule,
    location: Option<LatLong>,
    today: NaiveDate,
) -> Option<(u8, u8)> {
    match &rule.trigger {
        Trigger::Daily { hour, minute } => Some((*hour, *minute)),
        Trigger::Sunset { offset_minutes } | Trigger::Sunrise { offset_minutes } => {
            let fire = sun_fire_local(
                *offset_minutes,
                location,
                matches!(rule.trigger, Trigger::Sunrise { .. }),
                today,
            )?;
            #[allow(clippy::cast_possible_truncation)]
            let h = fire.hour() as u8;
            #[allow(clippy::cast_possible_truncation)]
            let m = fire.minute() as u8;
            Some((h, m))
        }
        Trigger::Cron { expr } => {
            let sched = cron::Schedule::from_str(expr).ok()?;
            let day_start_local = Local
                .from_local_datetime(&today.and_hms_opt(0, 0, 0)?)
                .single()?;
            let next = sched.after(&day_start_local.with_timezone(&Utc)).next()?;
            let local = next.with_timezone(&Local);
            #[allow(clippy::cast_possible_truncation)]
            let h = local.hour() as u8;
            #[allow(clippy::cast_possible_truncation)]
            let m = local.minute() as u8;
            Some((h, m))
        }
    }
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
