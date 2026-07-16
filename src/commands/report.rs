use chrono::{Local, NaiveDate};

use crate::config::Config;
use crate::datafile;
use crate::period;
use crate::report;

/// `teum report [period] [--html PATH] [--open]`
///
/// Aggregates tracked time into weekly buckets and prints a plain-text table.
/// With `--html`, also writes a self-contained HTML report (inline-SVG charts).
/// `html` mirrors the `--html` flag: `None` = absent, `Some(None)` = given
/// with no value (use the default path), `Some(Some(p))` = explicit path.
pub fn run(
    config: &Config,
    period_str: &str,
    html: Option<Option<String>>,
    open: bool,
) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let today = Local::now().naive_local().date();

    let (start, end, label) = if period_str == "all" {
        let start = earliest_week_start(&data_dir)?.unwrap_or(today);
        (start, today, "all time".to_string())
    } else {
        let range = period::resolve(period_str, today)?;
        (range.start, range.end, range.label)
    };

    let intervals = datafile::read_range(&data_dir, start, end)?;
    if intervals.is_empty() {
        return Err(format!("no entries in range ({label})"));
    }

    let now = Local::now().naive_local().time();
    let weeks = report::aggregate(&intervals, config, today, now);

    // Warn about stale open timers on past days: they are data errors (a timer
    // left running), silently dropped from the totals above.
    let stale: Vec<&crate::interval::Interval> = intervals
        .iter()
        .filter(|iv| iv.end.is_none() && iv.date < today)
        .collect();
    for iv in &stale {
        eprintln!(
            "warning: unclosed timer {} {} | @{} — skipped (close it to count)",
            iv.date, iv.start, iv.project
        );
    }

    print!("{}", report::text_table(&weeks));

    // Resolve the HTML target: an explicit path wins; `--html` with no value or
    // a bare `--open` falls back to the default report path.
    let target: Option<std::path::PathBuf> = match html {
        Some(Some(p)) => Some(std::path::PathBuf::from(p)),
        Some(None) => Some(config.report_path()?),
        None if open => Some(config.report_path()?),
        None => None,
    };

    if let Some(path) = target {
        let html = report::html_report(&weeks, &label);
        crate::fsutil::atomic_write(&path, html.as_bytes())?;
        println!("\nwrote {}", path.display());
        if open {
            open_in_browser(&path)?;
        }
    }

    Ok(())
}

/// Scan the data directory for `YYYY-wWW.txt` files and return the Monday of the
/// earliest ISO week present.
fn earliest_week_start(data_dir: &std::path::Path) -> Result<Option<NaiveDate>, String> {
    let entries = match std::fs::read_dir(data_dir) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    let mut earliest: Option<NaiveDate> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(monday) = parse_week_filename(&name) {
            earliest = Some(match earliest {
                Some(cur) if cur <= monday => cur,
                _ => monday,
            });
        }
    }
    Ok(earliest)
}

/// Parse `YYYY-wWW.txt` -> Monday of that ISO week.
fn parse_week_filename(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_suffix(".txt")?;
    let (year_str, week_str) = stem.split_once("-w")?;
    let year: i32 = year_str.parse().ok()?;
    let week: u32 = week_str.parse().ok()?;
    NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
}

fn open_in_browser(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(path).status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = std::process::Command::new("xdg-open").arg(path).status();

    let status = status.map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("browser opener exited with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn parses_week_filenames() {
        let d = parse_week_filename("2030-w02.txt").unwrap();
        assert_eq!(d.iso_week().week(), 2);
        assert_eq!(d.weekday(), chrono::Weekday::Mon);
        assert!(parse_week_filename("current.json").is_none());
        assert!(parse_week_filename("2030-w02.lock").is_none());
        assert!(parse_week_filename("notes.txt").is_none());
    }
}
