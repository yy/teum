use chrono::{Duration, Local, Timelike};

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::interval::Interval;
use crate::parse::{parse_energy_and_desc, parse_start_args, truncate_to_minutes};
use crate::state;

pub fn run(
    config: &Config,
    preset: Option<&str>,
    duration_str: &str,
    cont: bool,
    args: &[String],
) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let now = Local::now().naive_local();
    let date = now.date();
    let now_time = truncate_to_minutes(now.time());

    let duration = parse_duration(duration_str)?;

    // Calculate start_time = now - duration (using minute arithmetic to detect underflow)
    let now_minutes = now_time.hour() as i64 * 60 + now_time.minute() as i64;
    let start_minutes = now_minutes - duration.num_minutes();
    if start_minutes < 0 {
        return Err("injection would cross midnight — use 'teum add' instead".into());
    }
    let start_time = chrono::NaiveTime::from_hms_opt(
        (start_minutes / 60) as u32,
        (start_minutes % 60) as u32,
        0,
    )
    .unwrap();

    let end_time = if cont { None } else { Some(now_time) };

    // Resolve project, tags, energy, description
    let (project, tags, energy, description) = if let Some(preset_name) = preset {
        let p = config.resolve_preset(preset_name)?;
        let (energy, desc) = parse_energy_and_desc(args)?;
        (p.project.clone(), p.tags.clone(), energy, desc)
    } else {
        parse_start_args(args)?
    };

    // Trim the previous entry:
    // - If there's a running timer, close it at start_time
    // - Otherwise, if the last closed entry overlaps, trim its end
    if let Some((open, path)) = datafile::find_open(&data_dir, date)? {
        super::validate_close_time(&open, date, start_time)?;
        datafile::close_open(&path, start_time, None)?;
        eprintln!("(trimmed running timer to {})", start_time.format("%H:%M"));
    } else if let Some(last) = datafile::find_last_closed(&data_dir, date)?
        && let Some(end) = last.end
        && end > start_time
    {
        let path = datafile::week_filepath(&data_dir, last.date);
        datafile::trim_last_end(&path, start_time)?;
        eprintln!("(trimmed previous entry to {})", start_time.format("%H:%M"));
    }

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
    // With --continue the injected entry is now the running timer; otherwise
    // any prior timer was trimmed closed and nothing is running.
    state::warn_on_err(state::write(
        config,
        if cont { Some(&interval) } else { None },
        None,
    ));

    // Display
    let mut meta = format!("@{}", interval.project);
    for tag in &interval.tags {
        meta.push_str(&format!(" #{tag}"));
    }

    if cont {
        println!("Injected: {meta}");
        if !interval.description.is_empty() {
            println!("          {}", interval.description);
        }
        println!("Running:  {} -", start_time.format("%H:%M"));
    } else {
        let dur = interval
            .duration()
            .ok_or("internal error: injected interval has no duration")?;
        println!("Injected: {meta}");
        if !interval.description.is_empty() {
            println!("          {}", interval.description);
        }
        println!(
            "{} - {} ({})",
            start_time.format("%H:%M"),
            now_time.format("%H:%M"),
            format::duration_str(dur)
        );
    }

    Ok(())
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim().to_lowercase();
    let (hours, minutes) = if let Some((hours, rest)) = s.split_once('h') {
        let hours: i64 = hours
            .parse()
            .map_err(|_| format!("invalid duration '{s}'"))?;
        let minutes = if rest.is_empty() {
            0
        } else {
            let minutes = rest
                .strip_suffix('m')
                .ok_or_else(|| format!("invalid duration '{s}' — expected format like 1h30m"))?;
            minutes
                .parse()
                .map_err(|_| format!("invalid duration '{s}'"))?
        };
        (hours, minutes)
    } else {
        let minutes = s
            .strip_suffix('m')
            .ok_or_else(|| format!("invalid duration '{s}' — use format like 30m or 1h30m"))?;
        let minutes = minutes
            .parse()
            .map_err(|_| format!("invalid duration '{s}'"))?;
        (0, minutes)
    };

    let total = hours
        .checked_mul(60)
        .and_then(|hours| hours.checked_add(minutes))
        .ok_or("duration is too large")?;
    if total <= 0 {
        return Err("duration must be positive".into());
    }

    Ok(Duration::minutes(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minutes() {
        assert_eq!(parse_duration("30m").unwrap(), Duration::minutes(30));
        assert_eq!(parse_duration("90m").unwrap(), Duration::minutes(90));
    }

    #[test]
    fn parse_hours() {
        assert_eq!(parse_duration("1h").unwrap(), Duration::minutes(60));
        assert_eq!(parse_duration("2h").unwrap(), Duration::minutes(120));
    }

    #[test]
    fn parse_hours_and_minutes() {
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::minutes(90));
        assert_eq!(parse_duration("2h15m").unwrap(), Duration::minutes(135));
    }

    #[test]
    fn reject_invalid_durations() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("h30m").is_err());
        assert!(parse_duration("30mjunk").is_err());
        assert!(parse_duration("1h30mgarbage").is_err());
    }
}
