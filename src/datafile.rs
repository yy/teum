use chrono::{Datelike, IsoWeek, NaiveDate};
use fs2::FileExt;
use std::path::{Path, PathBuf};

use crate::interval::Interval;

/// Acquire an exclusive lock on a lockfile adjacent to the given path.
/// The lock is released when the returned File is dropped.
fn lock_file(path: &Path) -> Result<std::fs::File, String> {
    let lock_path = path.with_extension("lock");
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("failed to open lock file: {e}"))?;
    lock.lock_exclusive()
        .map_err(|e| format!("failed to acquire lock: {e}"))?;
    Ok(lock)
}

pub fn week_filename(date: NaiveDate) -> String {
    let week = date.iso_week();
    format!("{}-w{:02}.txt", week.year(), week.week())
}

pub fn week_filepath(data_dir: &Path, date: NaiveDate) -> PathBuf {
    data_dir.join(week_filename(date))
}

pub fn read_intervals(path: &Path) -> Result<Vec<Interval>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut intervals = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue; // skip blank lines and comments
        }
        match Interval::parse(line) {
            Ok(iv) => intervals.push(iv),
            Err(e) => return Err(format!("{}:{}: {e}", path.display(), i + 1)),
        }
    }
    Ok(intervals)
}

pub fn append_interval(path: &Path, interval: &Interval) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create data directory: {e}"))?;
    }

    let line = format!("{}\n", interval.serialize());
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;

    file.write_all(line.as_bytes())
        .map_err(|e| format!("failed to write to {}: {e}", path.display()))
}

/// Find the open (running) interval. Returns the interval and its file path.
pub fn find_open(data_dir: &Path, date: NaiveDate) -> Result<Option<(Interval, PathBuf)>, String> {
    // Check current week's file first
    let path = week_filepath(data_dir, date);
    let intervals = read_intervals(&path)?;
    if let Some(iv) = intervals.into_iter().rev().find(|iv| iv.is_open()) {
        return Ok(Some((iv, path)));
    }

    // Check previous week in case timer was started last week
    let prev_date = date - chrono::Duration::days(7);
    let prev_path = week_filepath(data_dir, prev_date);
    if prev_path.exists() {
        let intervals = read_intervals(&prev_path)?;
        if let Some(iv) = intervals.into_iter().rev().find(|iv| iv.is_open()) {
            return Ok(Some((iv, prev_path)));
        }
    }

    Ok(None)
}

/// Close the open interval by filling in the end time (and optional energy).
/// Rewrites the file, replacing the open interval's line.
pub fn close_open(
    path: &Path,
    end_time: chrono::NaiveTime,
    energy: Option<u8>,
) -> Result<Option<Interval>, String> {
    let _lock = lock_file(path)?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut closed = None;

    // Find the last open interval (search from end)
    for line in lines.iter_mut().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Ok(mut iv) = Interval::parse(trimmed)
            && iv.is_open()
        {
            iv.end = Some(end_time);
            if energy.is_some() {
                iv.energy = energy;
            }
            *line = iv.serialize();
            closed = Some(iv);
            break;
        }
    }

    if closed.is_some() {
        let new_content = lines.join("\n") + "\n";
        std::fs::write(path, new_content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }

    Ok(closed)
}

/// Remove the last open interval (for cancel).
pub fn remove_open(path: &Path) -> Result<Option<Interval>, String> {
    let _lock = lock_file(path)?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut removed = None;

    // Find and remove the last open interval
    for i in (0..lines.len()).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Ok(iv) = Interval::parse(trimmed)
            && iv.is_open()
        {
            removed = Some(iv);
            lines.remove(i);
            break;
        }
    }

    if removed.is_some() {
        let new_content = if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        };
        std::fs::write(path, new_content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }

    Ok(removed)
}

/// Trim the end time of the last closed interval (for inject).
pub fn trim_last_end(path: &Path, new_end: chrono::NaiveTime) -> Result<Option<Interval>, String> {
    let _lock = lock_file(path)?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut trimmed = None;

    for line in lines.iter_mut().rev() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Ok(mut iv) = Interval::parse(t)
            && !iv.is_open()
        {
            if new_end < iv.start {
                return Err(
                    "inject would make previous entry negative; use 'teum add' instead".into(),
                );
            }
            iv.end = Some(new_end);
            *line = iv.serialize();
            trimmed = Some(iv);
            break;
        }
    }

    if trimmed.is_some() {
        let new_content = lines.join("\n") + "\n";
        std::fs::write(path, new_content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }

    Ok(trimmed)
}

/// Read intervals from all weekly files within a date range.
pub fn read_range(
    data_dir: &Path,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Interval>, String> {
    // Collect unique week files that overlap the range
    let mut weeks_seen: Vec<IsoWeek> = Vec::new();
    let mut date = start;
    while date <= end {
        let week = date.iso_week();
        if !weeks_seen.contains(&week) {
            weeks_seen.push(week);
        }
        date += chrono::Duration::days(1);
    }

    let mut all = Vec::new();
    for week in &weeks_seen {
        let filename = format!("{}-w{:02}.txt", week.year(), week.week());
        let path = data_dir.join(&filename);
        let intervals = read_intervals(&path)?;
        for iv in intervals {
            if iv.date >= start && iv.date <= end {
                all.push(iv);
            }
        }
    }

    Ok(all)
}

/// Find the last completed interval (for resume).
pub fn find_last_closed(data_dir: &Path, date: NaiveDate) -> Result<Option<Interval>, String> {
    let path = week_filepath(data_dir, date);
    let intervals = read_intervals(&path)?;
    if let Some(iv) = intervals.into_iter().rev().find(|iv| !iv.is_open()) {
        return Ok(Some(iv));
    }

    // Check previous week
    let prev_date = date - chrono::Duration::days(7);
    let prev_path = week_filepath(data_dir, prev_date);
    if prev_path.exists() {
        let intervals = read_intervals(&prev_path)?;
        if let Some(iv) = intervals.into_iter().rev().find(|iv| !iv.is_open()) {
            return Ok(Some(iv));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use tempfile::TempDir;

    #[test]
    fn week_filename_format() {
        let date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap(); // Monday of week 2
        assert_eq!(week_filename(date), "2030-w02.txt");
    }

    #[test]
    fn append_and_read() {
        let dir = TempDir::new().unwrap();
        let date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let path = week_filepath(dir.path(), date);

        let iv = Interval {
            date,
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            project: "work".into(),
            tags: vec!["coding".into()],
            energy: None,
            description: "test".into(),
        };

        append_interval(&path, &iv).unwrap();
        let intervals = read_intervals(&path).unwrap();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0], iv);
    }

    #[test]
    fn find_and_close_open() {
        let dir = TempDir::new().unwrap();
        let date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let path = week_filepath(dir.path(), date);

        let iv = Interval {
            date,
            start: NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            end: None,
            project: "work".into(),
            tags: vec!["coding".into()],
            energy: None,
            description: "wip".into(),
        };

        append_interval(&path, &iv).unwrap();

        let found = find_open(dir.path(), date).unwrap();
        assert!(found.is_some());

        let end = NaiveTime::from_hms_opt(16, 30, 0).unwrap();
        let closed = close_open(&path, end, None).unwrap().unwrap();
        assert_eq!(closed.end, Some(end));

        // Verify it's now closed on disk
        let found = find_open(dir.path(), date).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn remove_open_interval() {
        let dir = TempDir::new().unwrap();
        let date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let path = week_filepath(dir.path(), date);

        let closed_iv = Interval {
            date,
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            project: "work".into(),
            tags: vec![],
            energy: None,
            description: "done".into(),
        };
        let open_iv = Interval {
            date,
            start: NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            end: None,
            project: "work".into(),
            tags: vec!["coding".into()],
            energy: None,
            description: "cancel me".into(),
        };

        append_interval(&path, &closed_iv).unwrap();
        append_interval(&path, &open_iv).unwrap();

        let removed = remove_open(&path).unwrap().unwrap();
        assert_eq!(removed.description, "cancel me");

        let intervals = read_intervals(&path).unwrap();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].description, "done");
    }
}
