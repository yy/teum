//! Machine-readable runtime state (`current.json`).
//!
//! teum is a headless CLI: the only signal that a timer is running is a line
//! with no end time buried in a weekly text file. That invisibility is how a
//! timer once stayed open for five days unnoticed. This module mirrors the
//! running timer into a small JSON file that external tools — notably the
//! `dial` desk timer — can watch to surface a live, glanceable indicator.
//!
//! The file stores *facts* (what is running, since when), not a stale elapsed
//! snapshot; consumers compute elapsed live from `start`. It is rewritten on
//! every state change (start/stop/cancel/resume/inject) and refreshed by
//! `teum status`, so it self-heals if it ever drifts.

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::config::Config;
use crate::interval::Interval;

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

    pub fn from_interval(iv: &Interval) -> Self {
        let start_dt = iv.date.and_time(iv.start);
        State {
            tracking: true,
            project: Some(iv.project.clone()),
            tags: iv.tags.clone(),
            description: if iv.description.is_empty() {
                None
            } else {
                Some(iv.description.clone())
            },
            start: Some(start_dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
            elapsed_seconds: None,
        }
    }

    /// Attach a live elapsed-seconds value, computed from the full start
    /// datetime (date included) so multi-day-stale timers report honestly.
    pub fn with_elapsed(mut self, iv: &Interval, now: NaiveDateTime) -> Self {
        let start_dt = iv.date.and_time(iv.start);
        self.elapsed_seconds = Some((now - start_dt).num_seconds().max(0));
        self
    }

    pub fn to_json(&self) -> String {
        // A tiny fixed-shape struct; serialization cannot realistically fail.
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"tracking\":false}".into())
    }
}

/// Persist the current running state (or idle) to `current.json`.
///
/// Best-effort: a failure here must never block time tracking, so callers
/// pass the result through [`warn_on_err`] rather than propagating it.
pub fn write(config: &Config, interval: Option<&Interval>) -> Result<(), String> {
    let state = match interval {
        Some(iv) => State::from_interval(iv),
        None => State::idle(),
    };
    let path = config.state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create state directory: {e}"))?;
    }
    std::fs::write(&path, state.to_json() + "\n")
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Downgrade a state-write error to a stderr warning. State mirroring is a
/// convenience for external tools; the log write already succeeded.
pub fn warn_on_err(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("warning: could not update state file: {e}");
    }
}

/// Warn when an open interval predating `today` is about to be implicitly
/// closed or replaced (auto-stop on `start`/`resume`/`inject`). Closing a
/// days-old timer at today's clock time yields a bogus end time, so flag it.
pub fn warn_if_stale(iv: &Interval, today: chrono::NaiveDate) {
    if iv.date < today {
        let days = (today - iv.date).num_days();
        let plural = if days == 1 { "" } else { "s" };
        eprintln!(
            "⚠  auto-stopping a timer started {days} day{plural} ago ({}) — \
             its end time is likely wrong; run `teum edit` to fix.",
            iv.date.format("%Y-%m-%d")
        );
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

    #[test]
    fn interval_state_has_iso_start_and_no_elapsed() {
        let json = State::from_interval(&sample_open()).to_json();
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
        let state = State::from_interval(&sample_open()).with_elapsed(&sample_open(), now);
        let secs = state.elapsed_seconds.unwrap();
        // 5 days + 1h24m, not 1h24m.
        assert!(
            secs > 5 * 24 * 3600,
            "expected multi-day elapsed, got {secs}"
        );
    }
}
