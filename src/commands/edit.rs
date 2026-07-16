use chrono::Local;

use crate::config::Config;
use crate::datafile;

pub fn run(config: &Config, target: &str) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    let _operation_lock = datafile::lock_data_dir(&data_dir)?;
    let today = Local::now().naive_local().date();

    let path = if target == "current" {
        datafile::week_filepath(&data_dir, today)
    } else {
        // Validate YYYY-wWW format
        let bytes = target.as_bytes();
        let valid = bytes.len() == 8
            && bytes[0..4].iter().all(|b| b.is_ascii_digit())
            && bytes[4] == b'-'
            && bytes[5] == b'w'
            && bytes[6..8].iter().all(|b| b.is_ascii_digit());
        if !valid {
            return Err(format!(
                "invalid target '{target}' — expected 'current' or YYYY-wWW (e.g., 2030-w02)"
            ));
        }
        data_dir.join(format!("{target}.txt"))
    };

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());

    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .map_err(|e| format!("failed to launch {editor}: {e}"))?;

    if !status.success() {
        return Err(format!("{editor} exited with {status}"));
    }

    Ok(())
}
