use chrono::Local;
use std::process::Command;

use crate::config::Config;

pub fn run(config: &Config) -> Result<(), String> {
    let data_dir = config.data_dir();

    if !data_dir.join(".git").exists() {
        return Err(format!(
            "no git repo in {}. Run 'teum init' with sync = \"git\"",
            data_dir.display()
        ));
    }

    let now = Local::now().format("%Y-%m-%d %H:%M");

    // git add -A
    run_git(&data_dir, &["add", "-A"])?;

    // git commit (may fail if nothing to commit — that's ok)
    let commit_result = run_git(&data_dir, &["commit", "-m", &format!("teum: sync {now}")]);
    match commit_result {
        Ok(output) => println!("{output}"),
        Err(_) => println!("Nothing to commit."),
    }

    // git pull --rebase
    match run_git(&data_dir, &["pull", "--rebase"]) {
        Ok(output) => println!("{output}"),
        Err(e) => eprintln!("Pull failed: {e}"),
    }

    // git push
    let auto_push = config.auto_push.unwrap_or(true);
    if auto_push {
        match run_git(&data_dir, &["push"]) {
            Ok(output) => println!("{output}"),
            Err(e) => eprintln!("Push failed (no remote?): {e}"),
        }
    }

    Ok(())
}

fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
