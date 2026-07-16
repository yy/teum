use crate::config::Config;
use crate::datafile;
use crate::interval::Interval;

pub fn run(config: &Config, line: &str) -> Result<(), String> {
    let iv = Interval::parse(line)?;

    if iv.is_open() {
        return Err("cannot add an open interval (no end time)".into());
    }

    let data_dir = config.data_dir();
    let path = datafile::week_filepath(&data_dir, iv.date);
    datafile::append_interval(&path, &iv)?;

    println!("Added: {}", iv.serialize());
    Ok(())
}
