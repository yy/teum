//! Machine-readable runtime state (`current.json`).
//!
//! teum is a headless CLI: the only signal that a timer is running is a line
//! with no end time buried in a weekly text file. An unnoticed timer can
//! therefore remain open indefinitely. This module mirrors the running timer
//! into a small JSON file that external tools — notably the
//! `dial` desk timer — can watch to surface a live, glanceable indicator.
//!
//! The file stores *facts* (what is running, since when), not a stale elapsed
//! snapshot; consumers compute elapsed live from `start`. It is rewritten on
//! every state change (start/stop/cancel/resume/inject) and refreshed by
//! `teum status`, so it self-heals if it ever drifts.
//!
//! Unlike the ledger, `start` here carries seconds: this is runtime state, not
//! the permanent record, and a live readout that opens at 0:50 because the
//! ledger rounded the minute down is simply wrong. See [`resolve_start`].

use chrono::{Duration, NaiveDateTime};
use serde::Serialize;
use std::path::Path;

use crate::config::Config;
use crate::interval::Interval;

/// Serialization format for `start` — ISO-8601 local, seconds included.
const STAMP: &str = "%Y-%m-%dT%H:%M:%S";

#[derive(Debug, Serialize, PartialEq)]
pub struct State {
    /// Whether a timer is currently running.
    pub tracking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// ISO-8601 local start datetime, e.g. `2030-01-08T09:00:00`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Seconds since `start`. Only populated for live queries (`status --json`);
    /// omitted in the persisted file so a stale snapshot can't mislead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<i64>,
}

impl State {
    pub fn idle() -> Self {
        State {
            tracking: false,
            project: None,
            tags: Vec::new(),
            description: None,
            start: None,
            elapsed_seconds: None,
        }
    }

    /// Mirror a running interval. `start_dt` is the interval's start instant,
    /// which may carry seconds the minute-resolution ledger cannot — see
    /// [`resolve_start`].
    pub fn running(iv: &Interval, start_dt: NaiveDateTime) -> Self {
        State {
            tracking: true,
            project: Some(iv.project.clone()),
            tags: iv.tags.clone(),
            description: if iv.description.is_empty() {
                None
            } else {
                Some(iv.description.clone())
            },
            start: Some(start_dt.format(STAMP).to_string()),
            elapsed_seconds: None,
        }
    }

    /// Attach a live elapsed-seconds value, computed from the full start
    /// datetime (date included) so multi-day-stale timers report honestly.
    pub fn with_elapsed(mut self, start_dt: NaiveDateTime, now: NaiveDateTime) -> Self {
        self.elapsed_seconds = Some((now - start_dt).num_seconds().max(0));
        self
    }

    pub fn to_json(&self) -> String {
        // A tiny fixed-shape struct; serialization cannot realistically fail.
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"tracking\":false}".into())
    }
}

/// The instant a running interval actually began, seconds included.
///
/// The ledger is minute-resolution by design, so a timer started at 10:58:50 is
/// logged as `10:58` and any reader computing `now - start` would show 50s the
/// moment the timer begins. `current.json` is machine-local runtime state
/// rather than the permanent record, so it can hold the honest instant:
/// `start`/`resume` pass the wall clock they stamped, and every other writer —
/// notably `status`, which restates the mirror from the ledger — keeps the
/// seconds already on file for as long as they still describe this interval.
pub fn resolve_start(
    config: &Config,
    iv: &Interval,
    stamped: Option<NaiveDateTime>,
) -> NaiveDateTime {
    let floor = iv.date.and_time(iv.start);
    let carried = || {
        config
            .state_path()
            .ok()
            .and_then(|path| carried_start(&path, &iv.project))
    };
    refine(floor, stamped.or_else(carried))
}

/// Accept a sub-minute start only if it falls inside the logged minute —
/// anything else belongs to some other interval and the ledger wins.
fn refine(floor: NaiveDateTime, candidate: Option<NaiveDateTime>) -> NaiveDateTime {
    match candidate {
        Some(c) if c >= floor && c < floor + Duration::minutes(1) => c,
        _ => floor,
    }
}

/// The start already recorded in `current.json`, if it is tracking the same
/// project. Read tolerantly: a missing, empty, or hand-mangled file simply
/// means we have no sub-minute knowledge to carry forward.
fn carried_start(path: &Path, project: &str) -> Option<NaiveDateTime> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("tracking")?.as_bool() != Some(true) {
        return None;
    }
    if value.get("project")?.as_str() != Some(project) {
        return None;
    }
    NaiveDateTime::parse_from_str(value.get("start")?.as_str()?, STAMP).ok()
}

/// Persist the current running state (or idle) to `current.json`.
///
/// `stamped` is the precise wall clock of a start just issued, if the caller
/// has one; see [`resolve_start`].
///
/// Best-effort: a failure here must never block time tracking, so callers
/// pass the result through [`warn_on_err`] rather than propagating it.
pub fn write(
    config: &Config,
    interval: Option<&Interval>,
    stamped: Option<NaiveDateTime>,
) -> Result<(), String> {
    let state = match interval {
        Some(iv) => State::running(iv, resolve_start(config, iv, stamped)),
        None => State::idle(),
    };
    let path = config.state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create state directory: {e}"))?;
    }
    crate::fsutil::atomic_write(&path, (state.to_json() + "\n").as_bytes())
}

/// Downgrade a state-write error to a stderr warning. State mirroring is a
/// convenience for external tools; the log write already succeeded.
pub fn warn_on_err(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("warning: could not update state file: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    fn sample_open() -> Interval {
        Interval {
            date: NaiveDate::from_ymd_opt(2030, 1, 8).unwrap(),
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: None,
            project: "focus".into(),
            tags: vec!["build".into()],
            energy: None,
            description: "prototype".into(),
        }
    }

    #[test]
    fn idle_state_serializes_minimally() {
        let json = State::idle().to_json();
        assert!(json.contains("\"tracking\": false"));
        assert!(!json.contains("project"));
        assert!(!json.contains("elapsed"));
    }

    fn floor_of(iv: &Interval) -> NaiveDateTime {
        iv.date.and_time(iv.start)
    }

    #[test]
    fn interval_state_has_iso_start_and_no_elapsed() {
        let iv = sample_open();
        let json = State::running(&iv, floor_of(&iv)).to_json();
        assert!(json.contains("\"tracking\": true"));
        assert!(json.contains("\"project\": \"focus\""));
        assert!(json.contains("\"start\": \"2030-01-08T09:00:00\""));
        // Persisted file omits elapsed so it cannot go stale.
        assert!(!json.contains("elapsed"));
    }

    #[test]
    fn elapsed_counts_full_days_not_just_clock() {
        // A synthetic multi-day interval catches time-only subtraction bugs.
        let now = NaiveDate::from_ymd_opt(2030, 1, 13)
            .unwrap()
            .and_hms_opt(10, 24, 0)
            .unwrap();
        let iv = sample_open();
        let state = State::running(&iv, floor_of(&iv)).with_elapsed(floor_of(&iv), now);
        let secs = state.elapsed_seconds.unwrap();
        // 5 days + 1h24m, not 1h24m.
        assert!(
            secs > 5 * 24 * 3600,
            "expected multi-day elapsed, got {secs}"
        );
    }

    #[test]
    fn sub_minute_start_survives_inside_the_logged_minute() {
        let iv = sample_open();
        let stamped = floor_of(&iv) + Duration::seconds(50);
        assert_eq!(refine(floor_of(&iv), Some(stamped)), stamped);
    }

    #[test]
    fn start_from_another_minute_is_ignored() {
        let iv = sample_open();
        let floor = floor_of(&iv);
        // Stale mirror (an earlier interval) and an out-of-range later stamp
        // both lose to the ledger.
        assert_eq!(refine(floor, Some(floor - Duration::seconds(1))), floor);
        assert_eq!(refine(floor, Some(floor + Duration::seconds(60))), floor);
        assert_eq!(refine(floor, None), floor);
    }

    #[test]
    fn carried_start_reads_only_a_matching_running_mirror() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("current.json");
        let write = |json: &str| std::fs::write(&path, json).unwrap();

        assert_eq!(carried_start(&path, "focus"), None); // no file yet
        write("{\"tracking\": false}");
        assert_eq!(carried_start(&path, "focus"), None);
        write("not json at all");
        assert_eq!(carried_start(&path, "focus"), None);
        write("{\"tracking\": true, \"project\": \"other\", \"start\": \"2030-01-08T09:00:50\"}");
        assert_eq!(carried_start(&path, "focus"), None); // different interval
        write("{\"tracking\": true, \"project\": \"focus\", \"start\": \"2030-01-08T09:00:50\"}");
        assert_eq!(
            carried_start(&path, "focus"),
            Some(
                NaiveDate::from_ymd_opt(2030, 1, 8)
                    .unwrap()
                    .and_hms_opt(9, 0, 50)
                    .unwrap()
            )
        );
    }
}
