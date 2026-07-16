use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::format;
use crate::state::{self, State};

pub fn run(config: &Config, json: bool) -> Result<(), String> {
    let data_dir = config.data_dir();
    let now = Local::now().naive_local();
    let date = now.date();

    let open = datafile::find_open(&data_dir, date)?;

    // Reconcile the runtime state file with reality on every status call, so it
    // self-heals if a crash or manual edit ever left it out of sync.
    state::warn_on_err(state::write(config, open.as_ref().map(|(iv, _)| iv)));

    if json {
        let out = match &open {
            Some((iv, _)) => State::from_interval(iv).with_elapsed(iv, now).to_json(),
            None => State::idle().to_json(),
        };
        println!("{out}");
        return Ok(());
    }

    match open {
        Some((iv, _path)) => {
            let mut meta = format!("@{}", iv.project);
            for tag in &iv.tags {
                meta.push_str(&format!(" #{tag}"));
            }
            println!("Tracking: {meta}");
            if !iv.description.is_empty() {
                println!("          {}", iv.description);
            }
            // Elapsed from the full start datetime (date included), not the
            // clock alone — otherwise a days-old timer reads as minutes.
            let start_dt = iv.date.and_time(iv.start);
            let elapsed = now - start_dt;
            println!(
                "Started:  {} ({} ago)",
                iv.start.format("%H:%M"),
                format::duration_ago(elapsed)
            );
            // A timer whose start predates today is almost always a forgotten
            // runaway, not a genuine multi-day session. Flag it loudly.
            if iv.date < date {
                let days = (date - iv.date).num_days();
                let plural = if days == 1 { "" } else { "s" };
                eprintln!(
                    "⚠  open timer started {days} day{plural} ago ({}). \
                     Likely a runaway — `teum stop` or `teum cancel`.",
                    iv.date.format("%Y-%m-%d")
                );
            }
        }
        None => {
            println!("No active tracking.");
            // Show last completed interval
            if let Some(last) = datafile::find_last_closed(&data_dir, date)? {
                let mut meta = format!("@{}", last.project);
                for tag in &last.tags {
                    meta.push_str(&format!(" #{tag}"));
                }
                let end = last
                    .end
                    .map(|e| e.format("%H:%M").to_string())
                    .unwrap_or_default();
                println!(
                    "Last:     {} - {} | {meta}",
                    last.start.format("%H:%M"),
                    end
                );
            }
        }
    }

    Ok(())
}
