use chrono::Local;

use crate::config::Config;
use crate::datafile;
use crate::state;

pub fn run(config: &Config) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let now = Local::now().naive_local();
    let date = now.date();

    let (_open, path) = datafile::find_open(&data_dir, date)?.ok_or("nothing is running")?;

    let removed = datafile::remove_open(&path)?.ok_or("failed to remove interval")?;
    state::warn_on_err(state::write(config, None, None));

    let mut meta = format!("@{}", removed.project);
    for tag in &removed.tags {
        meta.push_str(&format!(" #{tag}"));
    }
    println!("Cancelled: {meta}");
    if !removed.description.is_empty() {
        println!("           {}", removed.description);
    }

    Ok(())
}
