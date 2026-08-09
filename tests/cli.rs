use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Sandbox {
    _temp: tempfile::TempDir,
    config_home: PathBuf,
    data_home: PathBuf,
    data_dir: PathBuf,
}

impl Sandbox {
    fn new(config: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let config_home = temp.path().join("config");
        let data_home = temp.path().join("data-home");
        let data_dir = temp.path().join("records");
        let config_dir = config_home.join("teum");
        std::fs::create_dir_all(&config_dir).unwrap();
        let data_dir_toml = format!("{:?}", data_dir.to_string_lossy());
        std::fs::write(
            config_dir.join("config.toml"),
            format!("data_dir = {data_dir_toml}\n{config}"),
        )
        .unwrap();
        Self {
            _temp: temp,
            config_home,
            data_home,
            data_dir,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_teum"));
        command
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_DATA_HOME", &self.data_home);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn configure_git_identity(dir: &Path) {
    assert_success(&git(dir, &["config", "user.name", "teum test"]));
    assert_success(&git(
        dir,
        &["config", "user.email", "teum-test@example.invalid"],
    ));
}

#[test]
fn malformed_config_aborts_before_writing_default_data() {
    let sandbox = Sandbox::new("");
    std::fs::write(
        sandbox.config_home.join("teum/config.toml"),
        "data_dir = [\n",
    )
    .unwrap();

    let output = sandbox.run(&["start", "@focus", "test"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to parse"));
    assert!(!sandbox.data_home.join("teum").exists());
}

#[test]
fn same_day_backwards_stop_is_rejected_without_closing_timer() {
    let sandbox = Sandbox::new("");
    assert_success(&sandbox.run(&["start", "--at", "15:00", "@focus", "retroactive"]));

    let output = sandbox.run(&["stop", "--at", "14:00"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot close"));
    let status = sandbox.run(&["status"]);
    assert_success(&status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Tracking: @focus"));
}

#[test]
fn invalid_start_does_not_stop_running_timer() {
    let sandbox = Sandbox::new("");
    assert_success(&sandbox.run(&["start", "--at", "09:00", "@focus", "keep-running"]));

    let output = sandbox.run(&["start", "--at", "10:00", "@Focus", "typo"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid project name"));
    let status = sandbox.run(&["status"]);
    assert_success(&status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Tracking: @focus"));
}

#[test]
fn edit_rejects_week_files_that_data_scans_ignore() {
    let sandbox = Sandbox::new("");
    let editor = std::env::current_exe().unwrap();

    let output = sandbox
        .command()
        .env("EDITOR", editor)
        .args(["edit", "2030-w99"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid target"));
}

#[test]
fn concurrent_starts_preserve_single_open_timer_invariant() {
    let sandbox = Sandbox::new("");
    let outputs = std::thread::scope(|scope| {
        let first = scope.spawn(|| sandbox.run(&["start", "@focus", "first"]));
        let second = scope.spawn(|| sandbox.run(&["start", "@side", "second"]));
        [first.join().unwrap(), second.join().unwrap()]
    });
    for output in &outputs {
        assert_success(output);
    }

    let status = sandbox.run(&["status"]);
    assert_success(&status);
    assert!(!String::from_utf8_lossy(&status.stderr).contains("multiple open intervals"));
    let log = sandbox.run(&["log"]);
    assert_success(&log);
    let log = String::from_utf8_lossy(&log.stdout);
    assert_eq!(
        log.matches("@focus").count() + log.matches("@side").count(),
        2
    );
    assert_eq!(log.matches("(running)").count(), 1);
}

#[test]
fn sync_failure_is_nonzero_and_lockfiles_are_not_committed() {
    let sandbox = Sandbox::new("sync = \"git\"\nauto_push = true\n");
    assert_success(&sandbox.run(&["init"]));
    configure_git_identity(&sandbox.data_dir);
    assert_success(&sandbox.run(&["start", "@focus", "sync-test"]));
    assert_success(&sandbox.run(&["stop"]));

    let output = sandbox.run(&["sync"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no upstream"));
    let tracked = git(&sandbox.data_dir, &["ls-files"]);
    assert_success(&tracked);
    let tracked = String::from_utf8_lossy(&tracked.stdout);
    assert!(tracked.lines().any(|line| line.ends_with(".txt")));
    assert!(!tracked.lines().any(|line| line.ends_with(".lock")));
}

#[test]
fn auto_commit_commits_on_stop_without_pushing() {
    let sandbox = Sandbox::new("sync = \"git\"\nauto_commit = true\nauto_push = false\n");
    assert_success(&sandbox.run(&["init"]));
    configure_git_identity(&sandbox.data_dir);
    assert_success(&sandbox.run(&["start", "@focus", "auto-commit"]));

    let output = sandbox.run(&["stop"]);

    assert_success(&output);
    let log = git(&sandbox.data_dir, &["log", "--oneline"]);
    assert_success(&log);
    assert_eq!(String::from_utf8_lossy(&log.stdout).lines().count(), 1);
    let tracked = git(&sandbox.data_dir, &["ls-files"]);
    assert_success(&tracked);
    assert!(
        !String::from_utf8_lossy(&tracked.stdout)
            .lines()
            .any(|line| line.ends_with(".lock"))
    );
}

#[test]
fn first_sync_pushes_and_sets_upstream() {
    let sandbox = Sandbox::new("sync = \"git\"\nauto_push = true\n");
    assert_success(&sandbox.run(&["init"]));
    configure_git_identity(&sandbox.data_dir);
    let remote = sandbox.data_home.join("remote.git");
    assert_success(
        &Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .unwrap(),
    );
    assert_success(&git(
        &sandbox.data_dir,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    ));
    assert_success(&sandbox.run(&["start", "@focus", "first-sync"]));
    assert_success(&sandbox.run(&["stop"]));
    let branch = git(&sandbox.data_dir, &["branch", "--show-current"]);
    assert_success(&branch);
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_string();

    let output = sandbox.run(&["sync"]);

    assert_success(&output);
    let upstream = git(
        &sandbox.data_dir,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    );
    assert_success(&upstream);
    assert_eq!(
        String::from_utf8_lossy(&upstream.stdout).trim(),
        format!("origin/{branch}")
    );
}

#[test]
fn running_state_keeps_seconds_the_ledger_rounds_away() {
    let sandbox = Sandbox::new("");
    assert_success(&sandbox.run(&["start", "@focus", "prototype"]));

    let state_path = sandbox.config_home.join("teum").join("current.json");
    let started = start_field(&state_path);
    let logged: Vec<_> = std::fs::read_dir(&sandbox.data_dir)
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    // The permanent record stays minute-resolution...
    assert!(
        logged
            .iter()
            .any(|line| line.contains(&format!(" {} -       |", &started[11..16]))),
        "ledger should log HH:MM, got {logged:?}"
    );
    // ...while the mirror keeps the second we actually started. Restating it
    // from the ledger — what `teum status` does, and what dial triggers on every
    // panel open — must not round that back down. Plant a known second so the
    // assertion holds whatever second the test happens to run on.
    let planted = format!("{}37", &started[..17]);
    let text = std::fs::read_to_string(&state_path).unwrap();
    std::fs::write(&state_path, text.replace(&started, &planted)).unwrap();
    assert_success(&sandbox.run(&["status"]));
    assert_eq!(start_field(&state_path), planted);

    // `status --json` reports elapsed against that same start, not the minute.
    let json = sandbox.run(&["status", "--json"]);
    assert_success(&json);
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&json.stdout)).unwrap();
    assert_eq!(value["start"].as_str().unwrap(), planted);
}

/// The `start` string inside `current.json`, e.g. `2030-01-08T09:00:50`.
fn start_field(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["start"].as_str().unwrap().to_string()
}
