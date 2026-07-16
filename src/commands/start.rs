use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::interval::Interval;
use crate::parse::{parse_energy_and_desc, parse_start_args, parse_time_or};
use crate::state;

pub fn run(
    config: &Config,
    preset: Option<&str>,
    at: Option<&str>,
    args: &[String],
) -> Result<(), String> {
    // Bare `teum start`: show available presets instead of starting anything
    if preset.is_none() && args.is_empty() {
        return print_presets(config);
    }

    // Validate the replacement before mutating a running timer. A typo in the
    // new project, tag, energy, or preset must leave the current timer intact.
    let (project, tags, energy, description) = if let Some(preset_name) = preset {
        let p = config.resolve_preset(preset_name)?;
        let (energy, desc) = parse_energy_and_desc(args)?;
        (p.project.clone(), p.tags.clone(), energy, desc)
    } else {
        parse_start_args(args)?
    };

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

    let interval = Interval {
        date,
        start: time,
        end: None,
        project,
        tags,
        energy,
        description,
    };

    let path = datafile::week_filepath(&data_dir, date);
    datafile::append_interval(&path, &interval)?;
    state::warn_on_err(state::write(config, Some(&interval)));

    // Display
    let mut meta = format!("@{}", interval.project);
    for tag in &interval.tags {
        meta.push_str(&format!(" #{tag}"));
    }
    println!("Tracking: {meta}");
    if !interval.description.is_empty() {
        println!("          {}", interval.description);
    }
    println!("Started:  {}", time.format("%H:%M"));

    Ok(())
}

fn print_presets(config: &Config) -> Result<(), String> {
    println!("usage: teum start @project [#tags] [description]");
    println!("       teum start -p <preset> [description]");
    if config.presets.is_empty() {
        println!();
        println!(
            "No presets configured. Add some to {}",
            crate::config::config_path()?.display()
        );
        return Ok(());
    }
    println!();
    println!("Presets:");
    let mut names: Vec<_> = config.presets.keys().collect();
    names.sort();
    let width = names.iter().map(|n| n.len()).max().unwrap_or(0);
    for name in names {
        let p = &config.presets[name];
        let mut meta = format!("@{}", p.project);
        for tag in &p.tags {
            meta.push_str(&format!(" #{tag}"));
        }
        println!("  {name:<width$}  {meta}");
    }
    Ok(())
}
