use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::parse::parse_time_or;
use crate::state;

pub fn run(config: &Config, at: Option<&str>, energy: Option<u8>) -> Result<(), String> {
    if let Some(e) = energy
        && !(1..=5).contains(&e)
    {
        return Err(format!("energy level {e} out of range (use 1-5)"));
    }
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let now = Local::now().naive_local();
    let date = now.date();
    let time = parse_time_or(at, now.time())?;

    let (open, path) = datafile::find_open(&data_dir, date)?.ok_or("nothing is running")?;
    super::validate_close_time(&open, date, time)?;

    let closed = datafile::close_open(&path, time, energy)?.ok_or("failed to close interval")?;
    state::warn_on_err(state::write(config, None, None));

    // Note if the timer crossed midnight
    if closed.date < date {
        eprintln!(
            "note: timer crossed midnight (started {} on {})",
            closed.start.format("%H:%M"),
            closed.date.format("%Y-%m-%d")
        );
    }

    let mut meta = format!("@{}", closed.project);
    for tag in &closed.tags {
        meta.push_str(&format!(" #{tag}"));
    }
    if let Some(e) = closed.energy {
        meta.push_str(&format!(" !{e}"));
    }

    let dur = closed
        .duration()
        .ok_or("internal error: closed interval has no duration")?;
    println!("Stopped: {meta}");
    if !closed.description.is_empty() {
        println!("         {}", closed.description);
    }
    println!(
        "{} - {} ({})",
        closed.start.format("%H:%M"),
        time.format("%H:%M"),
        format::duration_str(dur)
    );

    super::sync::auto_commit(config)?;

    Ok(())
}
