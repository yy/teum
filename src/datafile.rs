use chrono::{Datelike, IsoWeek, NaiveDate};
use fs2::FileExt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::interval::Interval;

/// Acquire an exclusive lock on a lockfile adjacent to the given path.
/// The lock is released when the returned File is dropped.
fn open_lock(path: &Path) -> Result<std::fs::File, String> {
    let lock_path = path.with_extension("lock");
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("failed to open lock file: {e}"))
}

fn lock_file(path: &Path) -> Result<std::fs::File, String> {
    let lock = open_lock(path)?;
    lock.lock_exclusive()
        .map_err(|e| format!("failed to acquire lock: {e}"))?;
    Ok(lock)
}

/// Serialize command-level mutations that may inspect or touch multiple week
/// files. Per-file locks protect bytes; this lock protects invariants such as
/// "at most one open timer" across the whole data directory.
pub(crate) fn lock_data_dir(data_dir: &Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(data_dir)
        .map_err(|e| format!("failed to create data directory: {e}"))?;
    lock_file(&data_dir.join(".teum-operation"))
}

fn read_lock(path: &Path) -> Result<std::fs::File, String> {
    let lock = open_lock(path)?;
    FileExt::lock_shared(&lock).map_err(|e| format!("failed to acquire read lock: {e}"))?;
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
    let _lock = read_lock(path)?;
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

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create data directory: {e}"))?;
    }

    let _lock = lock_file(path)?;
    let line = format!("{}\n", interval.serialize());
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;

    file.write_all(line.as_bytes())
        .map_err(|e| format!("failed to write to {}: {e}", path.display()))?;
    file.sync_data()
        .map_err(|e| format!("failed to sync {}: {e}", path.display()))
}

/// Find the open (running) interval. Returns the interval and its file path.
pub fn find_open(data_dir: &Path, date: NaiveDate) -> Result<Option<(Interval, PathBuf)>, String> {
    let mut open = Vec::new();
    for path in week_filepaths(data_dir)? {
        for iv in read_intervals(&path)? {
            if iv.is_open() {
                if iv.date > date {
                    return Err(format!(
                        "open interval starts in the future ({} {} in {})",
                        iv.date,
                        iv.start,
                        path.display()
                    ));
                }
                open.push((iv, path.clone()));
            }
        }
    }
    open.sort_by_key(|(iv, _)| iv.date.and_time(iv.start));
    match open.len() {
        0 => Ok(None),
        1 => Ok(open.pop()),
        n => Err(format!(
            "found {n} open intervals; run 'teum edit' and leave exactly one open timer"
        )),
    }
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
        crate::fsutil::atomic_write(path, new_content.as_bytes())?;
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
        crate::fsutil::atomic_write(path, new_content.as_bytes())?;
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
        crate::fsutil::atomic_write(path, new_content.as_bytes())?;
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
    let mut last = None;
    for path in week_filepaths(data_dir)? {
        for iv in read_intervals(&path)? {
            if !iv.is_open() && iv.date <= date {
                let replace = last.as_ref().is_none_or(|current: &Interval| {
                    iv.date.and_time(iv.start) > current.date.and_time(current.start)
                });
                if replace {
                    last = Some(iv);
                }
            }
        }
    }
    Ok(last)
}

fn week_filepaths(data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(data_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("failed to read {}: {e}", data_dir.display())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read {}: {e}", data_dir.display()))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_week_filename(name) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub(crate) fn is_week_filename(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".txt") else {
        return false;
    };
    let Some((year, week)) = stem.split_once("-w") else {
        return false;
    };
    year.len() == 4
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && week.len() == 2
        && week
            .parse::<u32>()
            .is_ok_and(|week| (1..=53).contains(&week))
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

    #[test]
    fn finds_open_interval_older_than_previous_week() {
        let dir = TempDir::new().unwrap();
        let old_date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let today = NaiveDate::from_ymd_opt(2030, 2, 4).unwrap();
        let path = week_filepath(dir.path(), old_date);
        let iv = Interval {
            date: old_date,
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: None,
            project: "focus".into(),
            tags: vec![],
            energy: None,
            description: "forgotten".into(),
        };
        append_interval(&path, &iv).unwrap();

        let (found, found_path) = find_open(dir.path(), today).unwrap().unwrap();
        assert_eq!(found, iv);
        assert_eq!(found_path, path);
    }

    #[test]
    fn rejects_multiple_open_intervals() {
        let dir = TempDir::new().unwrap();
        let first_date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let second_date = NaiveDate::from_ymd_opt(2030, 1, 14).unwrap();
        for (date, description) in [(first_date, "first"), (second_date, "second")] {
            append_interval(
                &week_filepath(dir.path(), date),
                &Interval {
                    date,
                    start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                    end: None,
                    project: "focus".into(),
                    tags: vec![],
                    energy: None,
                    description: description.into(),
                },
            )
            .unwrap();
        }

        let err = find_open(dir.path(), second_date).unwrap_err();
        assert!(err.contains("2 open intervals"));
    }

    #[test]
    fn concurrent_append_and_close_preserve_every_entry() {
        let dir = TempDir::new().unwrap();
        let date = NaiveDate::from_ymd_opt(2030, 1, 7).unwrap();
        let path = week_filepath(dir.path(), date);
        let open = Interval {
            date,
            start: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end: None,
            project: "focus".into(),
            tags: vec![],
            energy: None,
            description: "running".into(),
        };
        append_interval(&path, &open).unwrap();

        let append_path = path.clone();
        let append_thread = std::thread::spawn(move || {
            for minute in 0..100 {
                let iv = Interval {
                    date,
                    start: NaiveTime::from_hms_opt(10, minute % 60, 0).unwrap(),
                    end: Some(NaiveTime::from_hms_opt(11, minute % 60, 0).unwrap()),
                    project: "side".into(),
                    tags: vec![],
                    energy: None,
                    description: format!("entry {minute}"),
                };
                append_interval(&append_path, &iv).unwrap();
            }
        });
        let close_path = path.clone();
        let close_thread = std::thread::spawn(move || {
            close_open(&close_path, NaiveTime::from_hms_opt(9, 0, 0).unwrap(), None)
                .unwrap()
                .unwrap();
        });
        append_thread.join().unwrap();
        close_thread.join().unwrap();

        let intervals = read_intervals(&path).unwrap();
        assert_eq!(intervals.len(), 101);
        assert!(intervals.iter().all(|iv| !iv.is_open()));
    }
}
