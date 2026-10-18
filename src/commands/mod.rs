mod add;
mod cancel;
mod doctor;
mod edit;
mod fill;
mod init;
mod inject;
mod log;
mod report;
mod resume;
mod start;
mod status;
mod stop;
mod summary;
mod sync;

pub use add::run as add;
pub use cancel::run as cancel;
pub use doctor::run as doctor;
pub use edit::run as edit;
pub use fill::run as fill;
pub use init::run as init;
pub use inject::run as inject;
pub use log::run as log;
pub use report::run as report;
pub use resume::run as resume;
pub use start::run as start;
pub use status::run as status;
pub use stop::run as stop;
pub use summary::run as summary;
pub use sync::run as sync;

fn validate_close_time(
    interval: &crate::interval::Interval,
    end_date: chrono::NaiveDate,
    end_time: chrono::NaiveTime,
) -> Result<(), String> {
    let elapsed_days = (end_date - interval.date).num_days();
    let representable = match elapsed_days {
        0 => end_time >= interval.start,
        1 => end_time < interval.start,
        _ => false,
    };
    if representable {
        Ok(())
    } else {
        // A same-day end before the start is a typo to fix by hand. Anything
        // else is a timer left running past 24 hours, which `stop --at` can
        // still close at the time the work really ended.
        let hint = if elapsed_days == 0 {
            "use 'teum edit' to correct it"
        } else {
            "close it with 'teum stop --at HH:MM', the time it should have \
             stopped, or use 'teum edit'"
        };
        Err(format!(
            "end time {} on {} cannot close a timer started at {} on {}; \
             intervals must be shorter than 24 hours ({hint})",
            end_time.format("%H:%M"),
            end_date,
            interval.start.format("%H:%M"),
            interval.date
        ))
    }
}

/// The day an explicit `teum stop --at` ends `interval` on.
///
/// `--at` names a clock time, not a date, and an interval is always shorter
/// than 24 hours, so the time can only mean its first occurrence at or after
/// the timer's start. Reading it that way — rather than as a time today — is
/// what lets a timer forgotten for days be closed at the moment the work
/// really ended. If that occurrence is still ahead (tomorrow), the answer
/// stays on `today`, where [`validate_close_time`] rejects it as backwards.
fn stop_at_date(
    interval: &crate::interval::Interval,
    end_time: chrono::NaiveTime,
    today: chrono::NaiveDate,
) -> chrono::NaiveDate {
    if end_time >= interval.start {
        interval.date
    } else {
        (interval.date + chrono::Duration::days(1)).min(today)
    }
}

/// How soon after a start a repeat of it counts as the same start fired twice
/// (a double-clicked button, a retried script) rather than a new session. The
/// ledger is minute-resolution, so this covers the starting minute and the
/// next one.
const REPEAT_WINDOW_MINUTES: i64 = 2;

/// Whether `open` began within [`REPEAT_WINDOW_MINUTES`] of `date`/`time`.
/// Restarting the same thing that soon would only auto-stop `open` into a
/// zero-length junk entry, so callers leave the running timer alone instead.
fn just_started(
    open: &crate::interval::Interval,
    date: chrono::NaiveDate,
    time: chrono::NaiveTime,
) -> bool {
    let gap = date.and_time(time) - open.date.and_time(open.start);
    gap >= chrono::Duration::zero() && gap < chrono::Duration::minutes(REPEAT_WINDOW_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    fn open(date: NaiveDate, hour: u32) -> crate::interval::Interval {
        crate::interval::Interval {
            date,
            start: NaiveTime::from_hms_opt(hour, 0, 0).unwrap(),
            end: None,
            project: "focus".into(),
            tags: vec![],
            energy: None,
            description: String::new(),
        }
    }

    #[test]
    fn close_time_rejects_same_day_backwards_and_multiday_intervals() {
        let monday = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let timer = open(monday, 15);
        assert!(
            validate_close_time(&timer, monday, NaiveTime::from_hms_opt(14, 0, 0).unwrap())
                .is_err()
        );
        assert!(
            validate_close_time(
                &timer,
                monday + chrono::Duration::days(2),
                NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn close_time_allows_real_cross_midnight_interval() {
        let monday = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let timer = open(monday, 23);
        assert!(
            validate_close_time(
                &timer,
                monday + chrono::Duration::days(1),
                NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
            )
            .is_ok()
        );
    }

    #[test]
    fn stop_at_lands_within_a_day_of_the_start() {
        let monday = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let day = chrono::Duration::days(1);
        let timer = open(monday, 20);
        let at = |hour| NaiveTime::from_hms_opt(hour, 0, 0).unwrap();

        // A timer forgotten for days closes on the day it started...
        assert_eq!(stop_at_date(&timer, at(21), monday + day * 3), monday);
        // ...or early the next morning, if the time is before the start.
        assert_eq!(stop_at_date(&timer, at(1), monday + day * 3), monday + day);
        // The usual overnight stop is unchanged.
        assert_eq!(stop_at_date(&timer, at(1), monday + day), monday + day);
        // Same-day backwards never rolls into tomorrow; it stays rejected.
        assert_eq!(stop_at_date(&timer, at(19), monday), monday);
        assert!(validate_close_time(&timer, monday, at(19)).is_err());
    }

    #[test]
    fn just_started_covers_only_the_first_two_minutes() {
        let monday = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let timer = open(monday, 23);
        let at = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();

        assert!(just_started(&timer, monday, at(23, 0)));
        assert!(just_started(&timer, monday, at(23, 1)));
        assert!(!just_started(&timer, monday, at(23, 2)));
        assert!(!just_started(&timer, monday, at(22, 59)));
        // The clock time alone would match; the date says a day has passed.
        assert!(!just_started(
            &timer,
            monday + chrono::Duration::days(1),
            at(23, 0)
        ));
    }
}
