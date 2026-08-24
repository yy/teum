use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::period;

pub fn run(config: &Config, period_str: &str) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let today = Local::now().naive_local().date();
    let range = period::resolve(period_str, today)?;

    let intervals = datafile::read_range(&data_dir, range.start, range.end)?;

    if intervals.is_empty() {
        println!("{}", range.label);
        println!("  (no entries)");
        return Ok(());
    }

    println!("{}", range.label);
    println!();

    let now = Local::now().naive_local().time();
    let mut total_minutes: i64 = 0;

    for iv in &intervals {
        let start = iv.start.format("%H:%M");
        let end_str = match iv.end {
            Some(end) => format!("{}", end.format("%H:%M")),
            None => "...  ".into(),
        };

        let mut meta = format!("@{}", iv.project);
        for tag in &iv.tags {
            meta.push_str(&format!(" #{tag}"));
        }
        if let Some(e) = iv.energy {
            meta.push_str(&format!(" !{e}"));
        }

        let (dur_str, running) = match iv.report_duration(today, now) {
            Some(dur) => {
                total_minutes += dur.num_minutes();
                (
                    format::duration_str(dur),
                    if iv.is_open() { " (running)" } else { "" },
                )
            }
            None => ("--".into(), " (stale; not counted)"),
        };

        let desc = if iv.description.is_empty() {
            String::new()
        } else {
            format!("  {}", iv.description)
        };

        println!("  {start} - {end_str}  {meta:<30}{desc:<30} {dur_str:>5}{running}");
    }

    println!("  {:>76}", "─────");
    println!(
        "  {:>76}",
        format::duration_str(chrono::Duration::minutes(total_minutes))
    );

    Ok(())
}
