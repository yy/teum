use chrono::Local;
use std::collections::BTreeMap;

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::period;

pub fn run(config: &Config, period_str: &str, group: Option<&str>) -> Result<(), String> {
    let data_dir = config.data_dir();
    let today = Local::now().naive_local().date();
    let range = period::resolve(period_str, today)?;

    let intervals = datafile::read_range(&data_dir, range.start, range.end)?;

    if intervals.is_empty() {
        println!("{}", range.label);
        println!("  (no entries)");
        return Ok(());
    }

    // Filter by report group if specified
    let filter_projects: Option<Vec<String>> = match group {
        Some(g) => Some(
            config
                .report_groups
                .get(g)
                .ok_or_else(|| format!("unknown report group '{g}'"))?
                .clone(),
        ),
        None => None,
    };

    // Aggregate by project: minutes and energy
    let now = Local::now().naive_local().time();
    let mut by_project: BTreeMap<String, (i64, Vec<u8>)> = BTreeMap::new();

    for iv in &intervals {
        if let Some(ref filter) = filter_projects
            && !filter.contains(&iv.project)
        {
            continue;
        }
        let dur = iv.duration().unwrap_or_else(|| iv.duration_until(now));
        let entry = by_project.entry(iv.project.clone()).or_default();
        entry.0 += dur.num_minutes();
        if let Some(e) = iv.energy {
            entry.1.push(e);
        }
    }

    let max_minutes = by_project.values().map(|(m, _)| *m).max().unwrap_or(0);
    let total_minutes: i64 = by_project.values().map(|(m, _)| m).sum();
    let has_energy = by_project.values().any(|(_, e)| !e.is_empty());

    println!("{}", range.label);
    println!();

    for (project, (minutes, energies)) in &by_project {
        let dur = format::duration_long(chrono::Duration::minutes(*minutes));
        let bar = format::bar(*minutes, max_minutes, 20);
        let energy_str = if energies.is_empty() {
            String::new()
        } else {
            let avg = energies.iter().map(|&e| e as f64).sum::<f64>() / energies.len() as f64;
            format!("  !{avg:.1}")
        };
        if has_energy {
            println!("  {project:<14} {dur:>8} {energy_str:>6}  {bar}");
        } else {
            println!("  {project:<14} {dur:>8}  {bar}");
        }
    }

    println!("  {:>22}", "───────");
    println!(
        "  {:<14} {:>8}",
        "total",
        format::duration_long(chrono::Duration::minutes(total_minutes))
    );

    Ok(())
}
