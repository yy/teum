//! Check the data directory for entries that look like mistakes rather than
//! records: forgotten timers, overlaps, misfiled weeks, unreadable lines.
//!
//! Nothing here rewrites data. The point is to surface a bad entry in the week
//! it happens, while you still remember what you were actually doing, instead
//! of months later when a report turns out to be built on phantom hours.

use chrono::{Duration, Local, NaiveDate};

use crate::config::Config;
use crate::datafile;
use crate::interval::Interval;

/// A same-day interval longer than this is almost certainly a timer left
/// running: a real single session that long would have been split by a break.
const LONG_SAME_DAY: i64 = 12;

/// Crossing midnight is a supported case, but a genuine one is short — the
/// late session that runs past twelve. Beyond this it reads as a runaway.
const LONG_OVERNIGHT: i64 = 6;

struct Finding {
    file: String,
    entry: String,
    note: String,
}

impl Finding {
    fn new(file: &str, entry: String, note: String) -> Self {
        Finding {
            file: file.to_string(),
            entry,
            note,
        }
    }
}

/// Describe an interval compactly enough to locate it by eye in the week file.
fn describe(iv: &Interval) -> String {
    let end = match iv.end {
        Some(end) => end.format("%H:%M").to_string(),
        None => "     ".to_string(),
    };
    format!(
        "{} {} - {} @{}",
        iv.date,
        iv.start.format("%H:%M"),
        end,
        iv.project
    )
}

fn hm(d: Duration) -> String {
    format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
}

pub fn run(config: &Config) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let today = Local::now().naive_local().date();
    let mut findings: Vec<Finding> = Vec::new();
    let mut open_timers: Vec<(String, Interval)> = Vec::new();
    let mut checked = 0usize;
    let mut files = 0usize;

    for path in datafile::week_filepaths(&data_dir)? {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        files += 1;

        // A parse error names its own line and aborts that file; report it and
        // keep going, so one bad line cannot hide every other week's problems.
        let intervals = match datafile::read_intervals(&path) {
            Ok(intervals) => intervals,
            Err(e) => {
                findings.push(Finding::new(&name, "unreadable".into(), e));
                continue;
            }
        };
        checked += intervals.len();

        for iv in &intervals {
            check_entry(&name, iv, today, &mut findings);
            if iv.is_open() {
                open_timers.push((name.clone(), iv.clone()));
            }
        }
        check_sequence(&name, &intervals, &mut findings);
    }

    // At most one timer may be running; more means a start was recorded twice.
    if open_timers.len() > 1 {
        for (name, iv) in &open_timers {
            findings.push(Finding::new(
                name,
                describe(iv),
                format!("one of {} open timers; only one may run", open_timers.len()),
            ));
        }
    }

    report(&findings, checked, files)
}

fn check_entry(name: &str, iv: &Interval, today: NaiveDate, findings: &mut Vec<Finding>) {
    let expected = datafile::week_filename(iv.date);
    if expected != name {
        findings.push(Finding::new(
            name,
            describe(iv),
            format!("belongs in {expected}"),
        ));
    }

    if iv.is_open() && iv.date < today {
        findings.push(Finding::new(
            name,
            describe(iv),
            "open timer on a past day; it counts as no time at all".into(),
        ));
        return;
    }

    let Some(dur) = iv.duration() else {
        return;
    };

    if dur.is_zero() {
        findings.push(Finding::new(
            name,
            describe(iv),
            "starts and ends at the same minute".into(),
        ));
        return;
    }

    let overnight = iv.end.is_some_and(|end| end < iv.start);
    let limit = if overnight {
        LONG_OVERNIGHT
    } else {
        LONG_SAME_DAY
    };
    if dur.num_hours() >= limit {
        let kind = if overnight { "overnight " } else { "" };
        findings.push(Finding::new(
            name,
            describe(iv),
            format!("{kind}run of {} — likely a forgotten timer", hm(dur)),
        ));
    }
}

/// Check consecutive entries within one file for ordering and overlap.
fn check_sequence(name: &str, intervals: &[Interval], findings: &mut Vec<Finding>) {
    for pair in intervals.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        let prev_start = prev.date.and_time(prev.start);
        let cur_start = cur.date.and_time(cur.start);

        if cur_start < prev_start {
            findings.push(Finding::new(
                name,
                describe(cur),
                "recorded after a later entry; the file is out of order".into(),
            ));
            continue;
        }

        let prev_end = prev_start + prev.duration().unwrap_or_else(Duration::zero);
        if cur_start < prev_end {
            let overlap = prev_end - cur_start;
            findings.push(Finding::new(
                name,
                describe(cur),
                format!("overlaps the previous entry by {}", hm(overlap)),
            ));
        }
    }
}

fn report(findings: &[Finding], checked: usize, files: usize) -> Result<(), String> {
    if findings.is_empty() {
        println!("{checked} entries in {files} files, nothing to report.");
        return Ok(());
    }

    let mut current = "";
    for f in findings {
        if f.file != current {
            if !current.is_empty() {
                println!();
            }
            println!("{}", f.file);
            current = &f.file;
        }
        println!("  {:<34} {}", f.entry, f.note);
    }

    println!();
    Err(format!(
        "{} issue(s) across {checked} entries in {files} files",
        findings.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn iv(date: &str, start: &str, end: Option<&str>) -> Interval {
        Interval {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            start: NaiveTime::parse_from_str(start, "%H:%M").unwrap(),
            end: end.map(|e| NaiveTime::parse_from_str(e, "%H:%M").unwrap()),
            project: "focus".into(),
            tags: vec![],
            energy: None,
            description: String::new(),
        }
    }

    fn notes(findings: &[Finding]) -> String {
        findings
            .iter()
            .map(|f| f.note.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    const TODAY: &str = "2030-01-09";

    fn today() -> NaiveDate {
        NaiveDate::parse_from_str(TODAY, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn flags_the_overnight_runaway_but_not_a_short_late_session() {
        let mut found = Vec::new();
        // The real failure mode: a timer started in the evening and noticed
        // the next morning.
        check_entry(
            "2030-w02.txt",
            &iv("2030-01-07", "17:18", Some("09:58")),
            today(),
            &mut found,
        );
        assert!(
            notes(&found).contains("forgotten timer"),
            "{}",
            notes(&found)
        );

        let mut ok = Vec::new();
        check_entry(
            "2030-w02.txt",
            &iv("2030-01-07", "23:30", Some("00:15")),
            today(),
            &mut ok,
        );
        assert!(ok.is_empty(), "{}", notes(&ok));
    }

    #[test]
    fn a_long_workday_block_is_not_a_runaway() {
        let mut found = Vec::new();
        check_entry(
            "2030-w02.txt",
            &iv("2030-01-07", "09:00", Some("17:00")),
            today(),
            &mut found,
        );
        assert!(found.is_empty(), "{}", notes(&found));
    }

    #[test]
    fn flags_zero_duration_and_misfiled_and_stale_open() {
        let mut found = Vec::new();
        check_entry(
            "2030-w02.txt",
            &iv("2030-01-07", "15:14", Some("15:14")),
            today(),
            &mut found,
        );
        assert!(notes(&found).contains("same minute"));

        let mut misfiled = Vec::new();
        check_entry(
            "2030-w01.txt",
            &iv("2030-01-07", "09:00", Some("10:00")),
            today(),
            &mut misfiled,
        );
        assert!(notes(&misfiled).contains("2030-w02.txt"));

        let mut stale = Vec::new();
        check_entry(
            "2030-w02.txt",
            &iv("2030-01-07", "09:00", None),
            today(),
            &mut stale,
        );
        assert!(notes(&stale).contains("past day"));

        // The timer running right now is not stale.
        let mut live = Vec::new();
        check_entry(
            "2030-w02.txt",
            &iv(TODAY, "09:00", None),
            today(),
            &mut live,
        );
        assert!(live.is_empty(), "{}", notes(&live));
    }

    #[test]
    fn flags_overlap_and_disorder_but_accepts_touching_entries() {
        let mut overlap = Vec::new();
        check_sequence(
            "2030-w02.txt",
            &[
                iv("2030-01-07", "09:00", Some("10:00")),
                iv("2030-01-07", "09:30", Some("10:30")),
            ],
            &mut overlap,
        );
        assert!(notes(&overlap).contains("overlaps"), "{}", notes(&overlap));

        let mut disorder = Vec::new();
        check_sequence(
            "2030-w02.txt",
            &[
                iv("2030-01-07", "10:07", Some("10:30")),
                iv("2030-01-07", "09:30", Some("10:00")),
            ],
            &mut disorder,
        );
        assert!(notes(&disorder).contains("out of order"));

        // Back-to-back entries share an instant and must stay quiet.
        let mut touching = Vec::new();
        check_sequence(
            "2030-w02.txt",
            &[
                iv("2030-01-07", "09:00", Some("10:00")),
                iv("2030-01-07", "10:00", Some("11:00")),
            ],
            &mut touching,
        );
        assert!(touching.is_empty(), "{}", notes(&touching));
    }
}
