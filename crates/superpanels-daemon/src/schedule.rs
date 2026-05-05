//! Schedule evaluation: 60-second tick that checks per-profile schedules (`SPEC.md` §9.3).

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Utc};
use superpanels_core::config::{Profile, Schedule};
use superpanels_core::ipc::{IpcRequest, PROTOCOL_VERSION};
use tokio::sync::{Mutex, watch};
use tokio::time::MissedTickBehavior;
use tracing::{debug, warn};

use crate::server;
use crate::state::DaemonState;

/// Tracks per-profile last-fire timestamps so each schedule fires at most once
/// per trigger window.
pub(crate) struct ScheduleChecker {
    /// Profile name → unix seconds of last schedule fire.
    last_fire: HashMap<String, i64>,
}

impl ScheduleChecker {
    pub(crate) fn new() -> Self {
        Self {
            last_fire: HashMap::new(),
        }
    }

    /// Check schedules against the current wall time. Returns profile names to apply.
    pub(crate) fn check(&mut self, profiles: &[Profile], lat_lon: Option<[f64; 2]>) -> Vec<String> {
        self.check_at(Local::now(), profiles, lat_lon)
    }

    /// Testable variant that accepts an injected `now`.
    pub(crate) fn check_at(
        &mut self,
        now: DateTime<Local>,
        profiles: &[Profile],
        lat_lon: Option<[f64; 2]>,
    ) -> Vec<String> {
        let mut to_apply = Vec::new();

        for profile in profiles {
            let Some(sched) = &profile.schedule else {
                continue;
            };
            let last = self.last_fire.get(&profile.name).copied();

            let (target, fire) = match sched {
                Schedule::Daily {
                    hour,
                    minute,
                    profile: target,
                } => {
                    let fire = daily_should_fire(*hour, *minute, now, last);
                    (target.as_str(), fire)
                }
                Schedule::Sunset {
                    offset_minutes,
                    profile: target,
                } => {
                    let fire = if let Some([slat, slon]) = lat_lon {
                        sunset_should_fire(*offset_minutes, slat, slon, now, last)
                    } else {
                        warn!(
                            profile = %profile.name,
                            "sunset schedule skipped: no [general].lat_lon configured"
                        );
                        false
                    };
                    (target.as_str(), fire)
                }
                Schedule::Cron { expr } => {
                    let fire = cron_should_fire(expr, now.with_timezone(&Utc), last);
                    (profile.name.as_str(), fire)
                }
            };

            if fire {
                debug!(profile = target, "schedule firing");
                to_apply.push(target.to_owned());
                self.last_fire.insert(profile.name.clone(), now.timestamp());
            }
        }

        to_apply
    }
}

/// Runs forever; checks all profile schedules every 60 seconds.
pub(crate) async fn run_schedule_checker(
    state: Arc<Mutex<DaemonState>>,
    timer_tx: watch::Sender<Option<Duration>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Skip the immediate first tick so we don't fire on daemon start-up.
    interval.tick().await;

    loop {
        interval.tick().await;

        let to_apply = {
            let mut guard = state.lock().await;
            let lat_lon = guard.config.general.lat_lon;
            // Clone profiles so the mutable borrow of schedule_checker and the
            // immutable borrow of config don't alias.
            let profiles = guard.config.profiles.clone();
            guard.schedule_checker.check(&profiles, lat_lon)
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

// --- schedule predicates ---

fn daily_should_fire(hour: u8, minute: u8, now: DateTime<Local>, last_secs: Option<i64>) -> bool {
    if u32::from(hour) != now.hour() || u32::from(minute) != now.minute() {
        return false;
    }
    // Re-firing guard: don't fire again within the same minute.
    match last_secs {
        None => true,
        Some(t) => now.timestamp() - t > 60,
    }
}

fn sunset_should_fire(
    offset_min: i32,
    lat: f64,
    lon: f64,
    now: DateTime<Local>,
    last_secs: Option<i64>,
) -> bool {
    let d = now.date_naive();
    let Some(sunset_utc_min) = compute_sunset_utc_minutes(lat, lon, d.year(), d.month(), d.day())
    else {
        return false;
    };
    let fire_utc_min = i64::from(sunset_utc_min) + i64::from(offset_min);
    let Some(fire_naive_utc) = chrono::NaiveDate::from_ymd_opt(d.year(), d.month(), d.day())
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|dt| dt + chrono::TimeDelta::minutes(fire_utc_min))
    else {
        return false;
    };
    let fire_local = Utc.from_utc_datetime(&fire_naive_utc).with_timezone(&Local);

    if fire_local.hour() != now.hour() || fire_local.minute() != now.minute() {
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
    // Find the first scheduled time after the previous check window.
    let since = match last_secs {
        Some(t) => DateTime::from_timestamp(t, 0).unwrap_or(now - chrono::TimeDelta::seconds(65)),
        None => now - chrono::TimeDelta::seconds(65),
    };
    sched.after(&since).next().is_some_and(|t| t <= now)
}

// --- astronomical helpers ---

/// Compute sunset time as minutes past midnight UTC using the USNO simplified
/// algorithm (accurate within ~2 min for latitudes ±65°). Returns `None` when
/// the sun does not set on the given date (polar day/night).
fn compute_sunset_utc_minutes(lat: f64, lon: f64, year: i32, month: u32, day: u32) -> Option<u32> {
    let doy = f64::from(day_of_year(year, month, day));
    let lng_hour = lon / 15.0;
    let t = doy + (18.0 - lng_hour) / 24.0; // approximate time for sunset

    let mean_anomaly_deg = 0.9856 * t - 3.289;
    let mean_anomaly_rad = mean_anomaly_deg.to_radians();

    let true_lon_deg = (mean_anomaly_deg
        + 1.916 * mean_anomaly_rad.sin()
        + 0.020 * (2.0 * mean_anomaly_rad).sin()
        + 282.634)
        .rem_euclid(360.0);
    let true_lon_rad = true_lon_deg.to_radians();

    // Right ascension (hours)
    let ra_deg = f64::atan(0.91764 * true_lon_rad.tan())
        .to_degrees()
        .rem_euclid(360.0);
    let l_quad = (true_lon_deg / 90.0).floor() * 90.0;
    let ra_quad = (ra_deg / 90.0).floor() * 90.0;
    let ra_hours = (ra_deg + l_quad - ra_quad) / 15.0;

    // Declination
    let sin_dec = 0.39782 * true_lon_rad.sin();
    let cos_dec = f64::asin(sin_dec).cos();

    // Hour angle (for civil twilight / sunset: zenith = 90.833°)
    let lat_rad = lat.to_radians();
    let cos_h = (-0.014_54 - lat_rad.sin() * sin_dec) / (lat_rad.cos() * cos_dec);
    if !(-1.0..=1.0).contains(&cos_h) {
        return None; // polar day or polar night
    }
    // USNO algorithm: for sunset use H = acos(cos_h); for sunrise use 360 - acos.
    let h_hours = f64::acos(cos_h).to_degrees() / 15.0;

    let local_mean_time = h_hours + ra_hours - 0.06571 * t - 6.622;
    let utc_hours = (local_mean_time - lng_hour).rem_euclid(24.0);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // reason: utc_hours ∈ [0, 24) after rem_euclid; result ∈ [0, 1440), fits u32.
    Some((utc_hours * 60.0).round() as u32)
}

fn day_of_year(year: i32, month: u32, day: u32) -> u32 {
    const DAYS_BEFORE: [u32; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap_bonus = u32::from(month > 2 && is_leap_year(year));
    DAYS_BEFORE[month as usize] + day + leap_bonus
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

// --- tests ---

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: test failures are bugs, not runtime errors
#[allow(clippy::expect_used)] // reason: same as above
mod tests {
    use super::*;
    use std::path::PathBuf;
    use superpanels_core::config::{Profile, ProfileBody, SpanProfile, SpanSource};
    use superpanels_core::layout::{BezelConfig, FitMode};

    fn profile_with_schedule(name: &str, sched: Schedule) -> Profile {
        Profile {
            name: name.to_owned(),
            body: ProfileBody::Span(SpanProfile {
                source: SpanSource::Single {
                    path: PathBuf::from("/walls/x.jpg"),
                },
                fit: FitMode::Fill,
                offset: [0, 0],
                image_size_px: None,
            }),
            bezels: BezelConfig {
                horizontal_mm: 0.0,
                vertical_mm: 0.0,
            },
            backend_override: None,
            schedule: Some(sched),
        }
    }

    fn fixed_local(hour: u32, minute: u32) -> DateTime<Local> {
        // Build a fixed local datetime for testing (2025-06-21, given hour/minute).
        let naive = chrono::NaiveDate::from_ymd_opt(2025, 6, 21)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        Local.from_local_datetime(&naive).unwrap()
    }

    #[test]
    fn daily_fires_at_matching_hour_and_minute() {
        // Arrange
        let profile = profile_with_schedule(
            "p",
            Schedule::Daily {
                hour: 18,
                minute: 0,
                profile: "target".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();

        // Act
        let result = checker.check_at(fixed_local(18, 0), &[profile], None);

        // Assert
        assert_eq!(result, vec!["target".to_owned()]);
    }

    #[test]
    fn daily_does_not_fire_at_wrong_time() {
        // Arrange
        let profile = profile_with_schedule(
            "p",
            Schedule::Daily {
                hour: 18,
                minute: 0,
                profile: "target".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();

        // Act
        let result = checker.check_at(fixed_local(17, 59), &[profile], None);

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn daily_does_not_refire_within_same_minute() {
        // Arrange
        let profile = profile_with_schedule(
            "p",
            Schedule::Daily {
                hour: 18,
                minute: 0,
                profile: "target".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();
        let now = fixed_local(18, 0);
        checker.check_at(now, std::slice::from_ref(&profile), None);

        // Act — second check in the same minute
        let result = checker.check_at(now, &[profile], None);

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn sunset_fires_without_lat_lon_does_nothing() {
        // Arrange
        let profile = profile_with_schedule(
            "p",
            Schedule::Sunset {
                offset_minutes: 0,
                profile: "target".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();

        // Act — no lat_lon configured; warning is expected and intentional
        let result = checker.check_at(fixed_local(18, 0), &[profile], None);

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn sunset_fires_at_computed_local_sunset_and_does_not_refire_within_minute() {
        // Arrange — London, midsummer. The test builds `now` by mirroring the
        // exact sequence sunset_should_fire performs internally so the
        // assertion holds regardless of the host's timezone (TZ=UTC and
        // TZ=America/Los_Angeles must both pass).
        let lat = 51.5074_f64;
        let lon = -0.1278_f64;

        // Pick a `local_date` from the host's Local now to keep the same
        // conversion semantics; the actual day of year drives the sunset.
        let local_today = Local::now().date_naive();
        let sunset_utc_min = compute_sunset_utc_minutes(
            lat,
            lon,
            local_today.year(),
            local_today.month(),
            local_today.day(),
        )
        .expect("london is below the polar circle");
        let fire_naive_utc = local_today.and_hms_opt(0, 0, 0).unwrap()
            + chrono::TimeDelta::minutes(i64::from(sunset_utc_min));
        let fire_local: DateTime<Local> =
            Utc.from_utc_datetime(&fire_naive_utc).with_timezone(&Local);

        // Build `now` so that now.date_naive() == local_today AND hour/minute
        // match fire_local's hour/minute — this is what sunset_should_fire
        // compares against. We use `local_today` plus fire_local's h/m.
        let now_naive = local_today
            .and_hms_opt(fire_local.hour(), fire_local.minute(), 0)
            .unwrap();
        let now: DateTime<Local> = Local
            .from_local_datetime(&now_naive)
            .single()
            .unwrap_or_else(|| Local.from_local_datetime(&now_naive).earliest().unwrap());

        let profile = profile_with_schedule(
            "p",
            Schedule::Sunset {
                offset_minutes: 0,
                profile: "target".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();

        // Act + Assert — first call at the firing minute must fire.
        let result = checker.check_at(now, std::slice::from_ref(&profile), Some([lat, lon]));
        assert_eq!(result, vec!["target".to_owned()]);

        // Re-fire guard: same `now` must not fire again.
        let again = checker.check_at(now, &[profile], Some([lat, lon]));
        assert!(
            again.is_empty(),
            "expected re-fire suppression, got {again:?}"
        );
    }

    #[test]
    fn cron_fires_when_expression_matches_window() {
        // Arrange — fires every minute at :00 seconds; use a time 5s past the boundary.
        let naive = chrono::NaiveDate::from_ymd_opt(2025, 6, 21)
            .unwrap()
            .and_hms_opt(12, 30, 5)
            .unwrap();
        let now_utc = Utc.from_utc_datetime(&naive);

        // Act
        let fires = cron_should_fire("0 * * * * *", now_utc, None);

        // Assert — there is a :00 second in the 65-second window before now
        assert!(fires);
    }

    #[test]
    fn cron_invalid_expression_does_not_panic() {
        // Arrange
        let profile = profile_with_schedule(
            "p",
            Schedule::Cron {
                expr: "not a cron expr".to_owned(),
            },
        );
        let mut checker = ScheduleChecker::new();

        // Act
        let result = checker.check_at(fixed_local(18, 0), &[profile], None);

        // Assert — no panic, no fire
        assert!(result.is_empty());
    }

    #[test]
    fn compute_sunset_returns_reasonable_time_for_london() {
        // London, midsummer: sunset ≈ 21:20 UTC
        let minutes = compute_sunset_utc_minutes(51.5, -0.1, 2025, 6, 21);
        let minutes = minutes.unwrap();
        // 20:00–22:00 UTC window
        assert!(minutes > 20 * 60 && minutes < 22 * 60, "minutes={minutes}");
    }

    #[test]
    fn day_of_year_leap_vs_non_leap() {
        // March 1 of a leap year should be day 61 (Jan 31 + Feb 29 + 1).
        assert_eq!(day_of_year(2024, 3, 1), 61);
        // March 1 of a non-leap year should be day 60.
        assert_eq!(day_of_year(2025, 3, 1), 60);
    }
}
