use chrono::{Datelike, Duration, NaiveDate};

pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub label: String,
}

pub fn resolve(period: &str, today: NaiveDate) -> Result<DateRange, String> {
    match period {
        "today" => Ok(DateRange {
            start: today,
            end: today,
            label: format!("Today ({})", today.format("%Y-%m-%d, %a")),
        }),
        "yesterday" => {
            let d = today - Duration::days(1);
            Ok(DateRange {
                start: d,
                end: d,
                label: format!("Yesterday ({})", d.format("%Y-%m-%d, %a")),
            })
        }
        "week" => {
            let monday = week_start(today);
            Ok(DateRange {
                start: monday,
                end: today,
                label: format!(
                    "Week {} ({} to {})",
                    today.iso_week().week(),
                    monday.format("%b %d"),
                    today.format("%b %d")
                ),
            })
        }
        "last-week" => {
            let monday = week_start(today) - Duration::weeks(1);
            let sunday = monday + Duration::days(6);
            Ok(DateRange {
                start: monday,
                end: sunday,
                label: format!(
                    "Week {} ({} to {})",
                    monday.iso_week().week(),
                    monday.format("%b %d"),
                    sunday.format("%b %d")
                ),
            })
        }
        "month" => {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            Ok(DateRange {
                start: first,
                end: today,
                label: format!("{}", today.format("%B %Y")),
            })
        }
        "year" => {
            let first = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            Ok(DateRange {
                start: first,
                end: today,
                label: format!("{}", today.year()),
            })
        }
        _ => Err(format!(
            "unknown period '{period}'. Use: today, yesterday, week, last-week, month, year"
        )),
    }
}

fn week_start(date: NaiveDate) -> NaiveDate {
    let days_from_monday = date.weekday().num_days_from_monday();
    date - Duration::days(days_from_monday as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn today_range() {
        let r = resolve("today", date(2030, 1, 9)).unwrap();
        assert_eq!(r.start, date(2030, 1, 9));
        assert_eq!(r.end, date(2030, 1, 9));
    }

    #[test]
    fn week_range() {
        // January 9, 2030 is Wednesday.
        let r = resolve("week", date(2030, 1, 9)).unwrap();
        assert_eq!(r.start, date(2030, 1, 7)); // Monday
        assert_eq!(r.end, date(2030, 1, 9));
    }

    #[test]
    fn last_week_range() {
        let r = resolve("last-week", date(2030, 1, 9)).unwrap();
        assert_eq!(r.start, date(2029, 12, 31)); // Previous Monday
        assert_eq!(r.end, date(2030, 1, 6)); // Previous Sunday
    }

    #[test]
    fn month_range() {
        let r = resolve("month", date(2030, 1, 9)).unwrap();
        assert_eq!(r.start, date(2030, 1, 1));
        assert_eq!(r.end, date(2030, 1, 9));
    }

    #[test]
    fn unknown_period() {
        assert!(resolve("biweekly", date(2030, 1, 9)).is_err());
    }
}
