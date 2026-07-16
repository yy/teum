mod add;
mod cancel;
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
        Err(format!(
            "end time {} on {} cannot close a timer started at {} on {}; \
             intervals must be shorter than 24 hours (use 'teum edit' to correct it)",
            end_time.format("%H:%M"),
            end_date,
            interval.start.format("%H:%M"),
            interval.date
        ))
    }
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
}
