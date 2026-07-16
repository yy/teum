use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::interval::Interval;
use crate::parse::{parse_energy_and_desc, parse_start_args, truncate_to_minutes};
use crate::state;

pub fn run(
    config: &Config,
    preset: Option<&str>,
    cont: bool,
    args: &[String],
) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let now = Local::now().naive_local();
    let date = now.date();
    let now_time = truncate_to_minutes(now.time());

    if datafile::find_open(&data_dir, date)?.is_some() {
        return Err("a timer is running — there is no gap to fill (use 'teum stop' first)".into());
    }

    let last = datafile::find_last_closed(&data_dir, date)?
        .ok_or("no previous interval — use 'teum add' instead")?;
    if last.date != date {
        return Err(format!(
            "last interval ended on {} — filling across days is not supported, use 'teum add'",
            last.date
        ));
    }
    let start_time = last
        .end
        .ok_or("internal error: last closed interval has no end")?;
    if start_time > now_time {
        return Err(format!(
            "last interval ends at {}, which is in the future",
            start_time.format("%H:%M")
        ));
    }
    if !cont && start_time == now_time {
        return Err("last interval ended just now — nothing to fill".into());
    }

    let end_time = if cont { None } else { Some(now_time) };

    // Resolve project, tags, energy, description
    let (project, tags, energy, description) = if let Some(preset_name) = preset {
        let p = config.resolve_preset(preset_name)?;
        let (energy, desc) = parse_energy_and_desc(args)?;
        (p.project.clone(), p.tags.clone(), energy, desc)
    } else {
        parse_start_args(args)?
    };

    let interval = Interval {
        date,
        start: start_time,
        end: end_time,
        project,
        tags,
        energy,
        description,
    };

    let path = datafile::week_filepath(&data_dir, date);
    datafile::append_interval(&path, &interval)?;
    state::warn_on_err(state::write(
        config,
        if cont { Some(&interval) } else { None },
    ));

    // Display
    let mut meta = format!("@{}", interval.project);
    for tag in &interval.tags {
        meta.push_str(&format!(" #{tag}"));
    }

    println!("Filled:  {meta}");
    if !interval.description.is_empty() {
        println!("         {}", interval.description);
    }
    if cont {
        println!("Running: {} -", start_time.format("%H:%M"));
    } else {
        let dur = interval
            .duration()
            .ok_or("internal error: filled interval has no duration")?;
        println!(
            "{} - {} ({})",
            start_time.format("%H:%M"),
            now_time.format("%H:%M"),
            format::duration_str(dur)
        );
    }

    Ok(())
}
