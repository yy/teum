use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::interval::Interval;
use crate::parse::parse_time_or;
use crate::state;

pub fn run(config: &Config, at: Option<&str>) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let now = Local::now().naive_local();
    let date = now.date();
    let time = parse_time_or(at, now.time())?;

    // Auto-stop if something is running
    if let Some((open, path)) = datafile::find_open(&data_dir, date)? {
        super::validate_close_time(&open, date, time)?;
        datafile::close_open(&path, time, None)?;
        eprintln!("(auto-stopped previous timer)");
    }

    let last =
        datafile::find_last_closed(&data_dir, date)?.ok_or("no previous interval to resume")?;

    let interval = Interval {
        date,
        start: time,
        end: None,
        project: last.project,
        tags: last.tags,
        energy: None,
        description: last.description.clone(),
    };

    let path = datafile::week_filepath(&data_dir, date);
    datafile::append_interval(&path, &interval)?;
    // Same as `start`: the mirror keeps the second, the ledger keeps the minute.
    state::warn_on_err(state::write(
        config,
        Some(&interval),
        at.is_none().then_some(now),
    ));

    let mut meta = format!("@{}", interval.project);
    for tag in &interval.tags {
        meta.push_str(&format!(" #{tag}"));
    }
    println!("Resumed: {meta}");
    if !interval.description.is_empty() {
        println!("         {}", interval.description);
    }
    println!("Started: {}", time.format("%H:%M"));

    Ok(())
}
