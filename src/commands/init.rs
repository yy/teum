use crate::config::{self, Config};

pub fn run(config: &Config) -> Result<(), String> {
    let data_dir = config.data_dir()?;

    // Create data directory
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("failed to create {}: {e}", data_dir.display()))?;
    println!("Data directory: {}", data_dir.display());

    // Create config if it doesn't exist
    let config_path = config::config_path()?;
    if !config_path.exists() {
        config::write_default_config(&config_path)?;
        println!("Config created: {}", config_path.display());
    } else {
        println!("Config exists:  {}", config_path.display());
    }

    // Initialize git if sync = "git"
    if config.sync.as_deref() == Some("git") {
        ensure_lockfiles_ignored(&data_dir)?;
        let git_dir = data_dir.join(".git");
        if !git_dir.exists() {
            let output = std::process::Command::new("git")
                .args(["init"])
                .current_dir(&data_dir)
                .output()
                .map_err(|e| format!("failed to run git init: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "git init failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            println!("Git initialized in {}", data_dir.display());
        } else {
            println!("Git already initialized");
        }
    }

    println!("Ready.");
    Ok(())
}

fn ensure_lockfiles_ignored(data_dir: &std::path::Path) -> Result<(), String> {
    let path = data_dir.join(".gitignore");
    let existing = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("failed to read {}: {e}", path.display())),
    };
    if existing.lines().any(|line| line.trim() == "*.lock") {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str("# teum runtime lock files\n*.lock\n");
    crate::fsutil::atomic_write(&path, updated.as_bytes())
}
