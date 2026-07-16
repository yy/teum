use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub data_dir: Option<String>,
    pub sync: Option<String>,
    #[serde(alias = "auto_commit")]
    pub auto_push: Option<bool>,
    #[serde(default)]
    pub presets: HashMap<String, Preset>,
    #[serde(default)]
    pub report_groups: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Preset {
    pub project: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("warning: failed to parse config: {e}"),
                },
                Err(e) => eprintln!("warning: failed to read config: {e}"),
            }
        }
        Config::default()
    }

    pub fn data_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.data_dir {
            let expanded = shellexpand(dir);
            PathBuf::from(expanded)
        } else {
            default_data_dir()
        }
    }

    /// Path to the machine-local runtime state file (`current.json`).
    ///
    /// Lives next to the config (not in `data_dir`, which may be a synced
    /// folder like iCloud) so it stays local to this machine — a running
    /// timer is a per-machine fact, and `dial` reads it here.
    pub fn state_path(&self) -> PathBuf {
        config_dir().join("current.json")
    }

    /// Default output path for `teum report --html` / `--open` when none is
    /// given. Machine-local, next to the config (not in a synced `data_dir`).
    pub fn report_path(&self) -> PathBuf {
        config_dir().join("report.html")
    }

    pub fn resolve_preset(&self, name: &str) -> Result<&Preset, String> {
        // Exact match first
        if let Some(preset) = self.presets.get(name) {
            return Ok(preset);
        }
        // Prefix match: find all presets whose name starts with the input
        let matches: Vec<_> = self
            .presets
            .iter()
            .filter(|(k, _)| k.starts_with(name))
            .collect();
        match matches.len() {
            0 => Err(format!("unknown preset '{name}'")),
            1 => Ok(matches[0].1),
            _ => {
                let mut names: Vec<_> = matches.iter().map(|(k, _)| k.as_str()).collect();
                names.sort();
                Err(format!(
                    "ambiguous preset '{name}': matches {}",
                    names.join(", ")
                ))
            }
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("HOME directory not found")
                .join(".config")
        });
    base.join("teum")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn default_data_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("HOME directory not found")
                .join(".local")
                .join("share")
        });
    base.join("teum")
}

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    s.to_string()
}

pub fn write_default_config(path: &Path) -> Result<(), String> {
    let content = r#"# teum configuration
# data_dir = "~/.local/share/teum"

# Sync method: "git" or "none" (iCloud users just point data_dir to iCloud)
# sync = "none"
# auto_push = false

[presets]
# dev = { project = "work", tags = ["coding"] }
# design = { project = "work", tags = ["design"] }
# plan = { project = "work", tags = ["planning"] }
# oss = { project = "side-project" }
# errand = { project = "personal" }

[report_groups]
# focus = ["focus"]
# support = ["support"]
# side = ["side-project"]
# excluded = ["personal"]
"#;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create config directory: {e}"))?;
    }
    std::fs::write(path, content).map_err(|e| format!("failed to write config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config() {
        let toml_str = r#"
data_dir = "~/my-data/teum"
sync = "git"

[presets]
dev = { project = "work", tags = ["coding"] }
oss = { project = "side-project" }

[report_groups]
billable = ["work", "consulting"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.data_dir, Some("~/my-data/teum".into()));
        assert_eq!(config.sync, Some("git".into()));
        assert_eq!(config.presets.len(), 2);
        assert_eq!(config.presets["dev"].project, "work");
        assert_eq!(config.presets["dev"].tags, vec!["coding"]);
        assert_eq!(config.report_groups["billable"], vec!["work", "consulting"]);
    }

    #[test]
    fn empty_config() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.data_dir.is_none());
        assert!(config.presets.is_empty());
    }

    #[test]
    fn preset_resolution() {
        let toml_str = r#"
[presets]
dev = { project = "work", tags = ["coding"] }
design = { project = "work", tags = ["design"] }
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Exact match
        let preset = config.resolve_preset("dev").unwrap();
        assert_eq!(preset.project, "work");
        assert_eq!(preset.tags, vec!["coding"]);
        // Prefix match (unique)
        let preset = config.resolve_preset("des").unwrap();
        assert_eq!(preset.tags, vec!["design"]);
        // Ambiguous prefix
        assert!(
            config
                .resolve_preset("de")
                .unwrap_err()
                .contains("ambiguous")
        );
        // No match
        assert!(config.resolve_preset("nonexistent").is_err());
    }
}
