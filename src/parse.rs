use chrono::{NaiveTime, Timelike};

/// Parse a time string like "HH:MM" or fall back to the default (truncated to minutes).
pub fn parse_time_or(at: Option<&str>, default: NaiveTime) -> Result<NaiveTime, String> {
    match at {
        Some(s) => {
            NaiveTime::parse_from_str(s, "%H:%M").map_err(|e| format!("invalid time '{s}': {e}"))
        }
        None => Ok(truncate_to_minutes(default)),
    }
}

/// Truncate a NaiveTime to minute precision (zero out seconds).
pub fn truncate_to_minutes(t: NaiveTime) -> NaiveTime {
    NaiveTime::from_hms_opt(t.hour(), t.minute(), 0).unwrap()
}

/// Parse positional args into (project, tags, energy, description).
/// Expects @project, optional #tags, optional !energy, and remaining words as description.
pub fn parse_start_args(
    args: &[String],
) -> Result<(String, Vec<String>, Option<u8>, String), String> {
    let mut project = None;
    let mut tags = Vec::new();
    let mut energy = None;
    let mut desc_parts = Vec::new();
    let mut in_desc = false;

    for arg in args {
        if in_desc {
            desc_parts.push(arg.as_str());
        } else if let Some(p) = arg.strip_prefix('@') {
            crate::interval::validate_name(p, "project")?;
            if project.is_some() {
                return Err("multiple @projects not allowed".into());
            }
            project = Some(p.to_string());
        } else if let Some(t) = arg.strip_prefix('#') {
            crate::interval::validate_name(t, "tag")?;
            tags.push(t.to_string());
        } else if let Some(e) = arg.strip_prefix('!') {
            if energy.is_some() {
                return Err("multiple energy levels not allowed".into());
            }
            let level: u8 = e
                .parse()
                .map_err(|_| format!("invalid energy level '!{e}' (use 1-5)"))?;
            if !(1..=5).contains(&level) {
                return Err(format!("energy level !{level} out of range (use 1-5)"));
            }
            energy = Some(level);
        } else {
            in_desc = true;
            desc_parts.push(arg.as_str());
        }
    }

    let project = project.ok_or("missing @project (e.g., @focus) or use -p preset")?;
    Ok((project, tags, energy, desc_parts.join(" ")))
}

/// Parse energy and description from args (used with presets where project/tags come from config).
pub fn parse_energy_and_desc(args: &[String]) -> Result<(Option<u8>, String), String> {
    let mut energy = None;
    let mut desc_parts = Vec::new();

    for arg in args {
        if let Some(e) = arg.strip_prefix('!') {
            if energy.is_some() {
                return Err("multiple energy levels not allowed".into());
            }
            let level: u8 = e
                .parse()
                .map_err(|_| format!("invalid energy level '!{e}' (use 1-5)"))?;
            if !(1..=5).contains(&level) {
                return Err(format!("energy level !{level} out of range (use 1-5)"));
            }
            energy = Some(level);
            continue;
        }
        desc_parts.push(arg.as_str());
    }

    Ok((energy, desc_parts.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_args_reject_invalid_and_duplicate_energy() {
        assert!(parse_energy_and_desc(&["!6".into()]).is_err());
        assert!(parse_energy_and_desc(&["!3".into(), "!4".into()]).is_err());
    }

    #[test]
    fn explicit_args_reject_invalid_names() {
        assert!(parse_start_args(&["@Focus".into()]).is_err());
        assert!(parse_start_args(&["@focus".into(), "#code_review".into()]).is_err());
    }
}
