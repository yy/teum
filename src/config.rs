use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub data_dir: Option<String>,
    pub sync: Option<String>,
    pub auto_commit: Option<bool>,
    pub auto_push: Option<bool>,
    #[serde(default)]
    pub presets: HashMap<String, Preset>,
    #[serde(default)]
    pub report_groups: HashMap<String, Vec<String>>,
    pub highlight_tags: Option<Vec<String>>,
}

/// Tags that mark an interval as highlighted when none are configured.
const DEFAULT_HIGHLIGHT_TAGS: &[&str] = &["highlight"];

#[derive(Debug, Deserialize, Clone)]
pub struct Preset {
    pub project: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Config, String> {
        let path = config_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
            let config: Config = toml::from_str(&contents)
                .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
            config.validate()?;
            return Ok(config);
        }
        Ok(Config::default())
    }

    pub fn data_dir(&self) -> Result<PathBuf, String> {
        if let Some(ref dir) = self.data_dir {
            let expanded = shellexpand(dir)?;
            Ok(PathBuf::from(expanded))
        } else {
            default_data_dir()
        }
    }

    /// Path to the machine-local runtime state file (`current.json`).
    ///
    /// Lives next to the config (not in `data_dir`, which may be a synced
    /// folder like iCloud) so it stays local to this machine — a running
    /// timer is a per-machine fact, and `dial` reads it here.
    pub fn state_path(&self) -> Result<PathBuf, String> {
        Ok(config_dir()?.join("current.json"))
    }

    /// Default output path for `teum report --html` / `--open` when none is
    /// given. Machine-local, next to the config (not in a synced `data_dir`).
    pub fn report_path(&self) -> Result<PathBuf, String> {
        Ok(config_dir()?.join("report.html"))
    }

    /// Tags that make an interval count toward the report's highlight and
    /// priority buckets. Defaults to `#highlight` alone.
    pub fn highlight_tags(&self) -> Vec<String> {
        match self.highlight_tags {
            Some(ref tags) => tags.clone(),
            None => DEFAULT_HIGHLIGHT_TAGS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
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

    fn validate(&self) -> Result<(), String> {
        if let Some(sync) = self.sync.as_deref()
            && !matches!(sync, "none" | "git")
        {
            return Err(format!(
                "invalid sync method '{sync}' (use 'none' or 'git')"
            ));
        }
        for (name, preset) in &self.presets {
            crate::interval::validate_name(&preset.project, "project")
                .map_err(|e| format!("preset '{name}': {e}"))?;
            for tag in &preset.tags {
                crate::interval::validate_name(tag, "tag")
                    .map_err(|e| format!("preset '{name}': {e}"))?;
            }
        }
        if let Some(ref tags) = self.highlight_tags {
            for tag in tags {
                crate::interval::validate_name(tag, "tag")
                    .map_err(|e| format!("highlight_tags: {e}"))?;
            }
        }
        Ok(())
    }
}

pub fn config_dir() -> Result<PathBuf, String> {
    let base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(path) => PathBuf::from(path),
        Err(_) => home_dir()?.join(".config"),
    };
    Ok(base.join("teum"))
}

pub fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn default_data_dir() -> Result<PathBuf, String> {
    let base = match std::env::var("XDG_DATA_HOME") {
        Ok(path) => PathBuf::from(path),
        Err(_) => home_dir()?.join(".local").join("share"),
    };
    Ok(base.join("teum"))
}

fn shellexpand(s: &str) -> Result<String, String> {
    if let Some(rest) = s.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest).to_string_lossy().into_owned());
    }
    Ok(s.to_string())
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| {
        "home directory is unavailable; set XDG_CONFIG_HOME and XDG_DATA_HOME explicitly".into()
    })
}

pub fn write_default_config(path: &Path) -> Result<(), String> {
    let content = r#"# teum configuration
# data_dir = "~/.local/share/teum"

# Sync method: "git" or "none" (iCloud users just point data_dir to iCloud)
# sync = "none"
# auto_commit = false
# auto_push = true

# Tags that count toward the report's highlight and priority buckets.
# highlight_tags = ["highlight"]

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
    crate::fsutil::atomic_write(path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config() {
        let toml_str = r#"
data_dir = "~/my-data/teum"
sync = "git"
auto_commit = true
auto_push = false

[presets]
dev = { project = "work", tags = ["coding"] }
oss = { project = "side-project" }

[report_groups]
billable = ["work", "consulting"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.data_dir, Some("~/my-data/teum".into()));
        assert_eq!(config.sync, Some("git".into()));
        assert_eq!(config.auto_commit, Some(true));
        assert_eq!(config.auto_push, Some(false));
        assert_eq!(config.presets.len(), 2);
        assert_eq!(config.presets["dev"].project, "work");
        assert_eq!(config.presets["dev"].tags, vec!["coding"]);
        assert_eq!(config.report_groups["billable"], vec!["work", "consulting"]);
    }

    #[test]
    fn highlight_tags_default_and_override() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.highlight_tags(), vec!["highlight".to_string()]);

        let config: Config = toml::from_str(r#"highlight_tags = ["deep", "improving"]"#).unwrap();
        config.validate().unwrap();
        assert_eq!(
            config.highlight_tags(),
            vec!["deep".to_string(), "improving".to_string()]
        );

        let config: Config = toml::from_str(r#"highlight_tags = ["Bad Tag"]"#).unwrap();
        assert!(config.validate().is_err());
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

    #[test]
    fn validation_rejects_unknown_sync_and_invalid_preset_names() {
        let config: Config = toml::from_str("sync = \"cloud\"").unwrap();
        assert!(config.validate().is_err());

        let config: Config = toml::from_str(
            r#"
[presets]
bad = { project = "Private_Project" }
"#,
        )
        .unwrap();
        assert!(config.validate().is_err());
    }
}
