use chrono::Local;
use std::process::Command;

use crate::config::Config;

pub fn run(config: &Config) -> Result<(), String> {
    let data_dir = config.data_dir()?;
    ensure_git_sync(config, &data_dir)?;

    if let Some(output) = commit_changes(&data_dir)?
        && !output.is_empty()
    {
        println!("{output}");
    }

    let auto_push = config.auto_push.unwrap_or(true);
    let branch = run_git(&data_dir, &["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err("cannot sync from a detached HEAD".into());
    }
    let has_upstream = has_upstream(&data_dir)?;
    if has_upstream {
        let output = run_git(&data_dir, &["pull", "--rebase"])?;
        if !output.is_empty() {
            println!("{output}");
        }
    } else {
        run_git(&data_dir, &["remote", "get-url", "origin"])
            .map_err(|_| "no upstream or 'origin' remote is configured".to_string())?;
        if remote_branch_exists(&data_dir, &branch)? {
            let output = run_git(&data_dir, &["pull", "--rebase", "origin", &branch])?;
            if !output.is_empty() {
                println!("{output}");
            }
        }
    }

    if auto_push && has_upstream {
        let output = run_git(&data_dir, &["push"])?;
        if !output.is_empty() {
            println!("{output}");
        }
    } else if auto_push {
        let output = run_git(&data_dir, &["push", "--set-upstream", "origin", &branch])?;
        if !output.is_empty() {
            println!("{output}");
        }
    }

    Ok(())
}

pub(crate) fn auto_commit(config: &Config) -> Result<(), String> {
    if config.sync.as_deref() != Some("git") || !config.auto_commit.unwrap_or(false) {
        return Ok(());
    }
    let data_dir = config.data_dir()?;
    ensure_git_sync(config, &data_dir)?;
    if let Some(output) = commit_changes(&data_dir)?
        && !output.is_empty()
    {
        println!("{output}");
    }
    Ok(())
}

fn ensure_git_sync(config: &Config, data_dir: &std::path::Path) -> Result<(), String> {
    if config.sync.as_deref() != Some("git") {
        return Err("git sync is disabled; set sync = \"git\" in config.toml".into());
    }
    if !data_dir.join(".git").exists() {
        return Err(format!(
            "no git repo in {}. Run 'teum init' with sync = \"git\"",
            data_dir.display()
        ));
    }
    Ok(())
}

fn commit_changes(data_dir: &std::path::Path) -> Result<Option<String>, String> {
    // Lock files coordinate local writers and are runtime state, never user data.
    run_git(
        data_dir,
        &[
            "add",
            "-A",
            "--",
            ".",
            ":(exclude)*.lock",
            ":(glob,exclude)**/*.lock",
        ],
    )?;
    if !has_staged_changes(data_dir)? {
        return Ok(None);
    }
    let now = Local::now().format("%Y-%m-%d %H:%M");
    run_git(data_dir, &["commit", "-m", &format!("teum: sync {now}")]).map(Some)
}

fn has_staged_changes(dir: &std::path::Path) -> Result<bool, String> {
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet", "--exit-code"])
        .current_dir(dir)
        .status()
        .map_err(|e| format!("failed to inspect staged changes: {e}"))?;
    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(format!("git diff --cached failed with {status}")),
    }
}

fn has_upstream(dir: &std::path::Path) -> Result<bool, String> {
    let status = Command::new("git")
        .args([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("failed to inspect git upstream: {e}"))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(128) => Ok(false),
        _ => Err(format!("git upstream inspection failed with {status}")),
    }
}

fn remote_branch_exists(dir: &std::path::Path, branch: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args(["ls-remote", "--exit-code", "--heads", "origin", branch])
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("failed to inspect origin: {e}"))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(2) => Ok(false),
        _ => Err(format!("failed to inspect origin branch '{branch}'")),
    }
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
