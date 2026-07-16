use chrono::{Duration, NaiveDate, NaiveTime};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: Option<NaiveTime>,
    pub project: String,
    pub tags: Vec<String>,
    pub energy: Option<u8>,
    pub description: String,
}

impl Interval {
    pub fn is_open(&self) -> bool {
        self.end.is_none()
    }

    pub fn duration(&self) -> Option<Duration> {
        self.end.map(|end| {
            let diff = end - self.start;
            if diff < Duration::zero() {
                // Cross-midnight: end is next day
                diff + Duration::hours(24)
            } else {
                diff
            }
        })
    }

    pub fn duration_until(&self, now: NaiveTime) -> Duration {
        let diff = now - self.start;
        if diff < Duration::zero() {
            diff + Duration::hours(24)
        } else {
            diff
        }
    }

    pub fn parse(line: &str) -> Result<Interval, String> {
        let line = line.trim();
        if line.is_empty() {
            return Err("empty line".into());
        }

        // Split on " | " to get up to 3 segments: time, metadata, description
        let parts: Vec<&str> = line.splitn(3, " | ").collect();
        if parts.len() < 2 {
            return Err(format!(
                "expected at least 2 pipe-separated segments: {line}"
            ));
        }

        let time_part = parts[0].trim();
        let meta_part = parts[1].trim().trim_end_matches('|').trim();
        let description = if parts.len() > 2 {
            parts[2].trim().to_string()
        } else {
            String::new()
        };

        // Parse time segment: "YYYY-MM-DD HH:MM - HH:MM" or "YYYY-MM-DD HH:MM -"
        let (date, start, end) = parse_time_segment(time_part)?;

        // Parse metadata segment: "@project #tag1 #tag2 !3"
        let (project, tags, energy) = parse_meta_segment(meta_part)?;

        Ok(Interval {
            date,
            start,
            end,
            project,
            tags,
            energy,
            description,
        })
    }

    pub fn serialize(&self) -> String {
        let date = self.date.format("%Y-%m-%d");
        let start = self.start.format("%H:%M");
        let end_str = match self.end {
            Some(end) => format!("{}", end.format("%H:%M")),
            None => "     ".into(),
        };

        let mut meta = format!("@{}", self.project);
        for tag in &self.tags {
            meta.push_str(&format!(" #{tag}"));
        }
        if let Some(e) = self.energy {
            meta.push_str(&format!(" !{e}"));
        }

        if self.description.is_empty() {
            format!("{date} {start} - {end_str} | {meta} |")
        } else {
            format!("{date} {start} - {end_str} | {meta} | {}", self.description)
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialize())
    }
}

fn parse_time_segment(s: &str) -> Result<(NaiveDate, NaiveTime, Option<NaiveTime>), String> {
    // Expected: "YYYY-MM-DD HH:MM - HH:MM" or "YYYY-MM-DD HH:MM -" (open)
    let parts: Vec<&str> = s.splitn(2, " - ").collect();
    if parts.len() < 2 {
        // Try without end time: "YYYY-MM-DD HH:MM -"
        let s_trimmed = s.trim_end_matches('-').trim();
        return parse_start_only(s_trimmed);
    }

    let start_part = parts[0].trim();
    let end_part = parts[1].trim();

    let (date, start) = parse_date_time(start_part)?;

    let end = if end_part.is_empty() {
        None
    } else {
        Some(
            NaiveTime::parse_from_str(end_part, "%H:%M")
                .map_err(|e| format!("invalid end time '{end_part}': {e}"))?,
        )
    };

    Ok((date, start, end))
}

fn parse_start_only(s: &str) -> Result<(NaiveDate, NaiveTime, Option<NaiveTime>), String> {
    let (date, start) = parse_date_time(s)?;
    Ok((date, start, None))
}

fn parse_date_time(s: &str) -> Result<(NaiveDate, NaiveTime), String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(format!("expected 'YYYY-MM-DD HH:MM', got '{s}'"));
    }
    let date = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")
        .map_err(|e| format!("invalid date '{}': {e}", parts[0]))?;
    let time = NaiveTime::parse_from_str(parts[1], "%H:%M")
        .map_err(|e| format!("invalid time '{}': {e}", parts[1]))?;
    Ok((date, time))
}

fn parse_meta_segment(s: &str) -> Result<(String, Vec<String>, Option<u8>), String> {
    let mut project = None;
    let mut tags = Vec::new();
    let mut energy = None;

    for token in s.split_whitespace() {
        if let Some(p) = token.strip_prefix('@') {
            if p.is_empty() {
                return Err("empty project name (use @name)".into());
            }
            if project.is_some() {
                return Err("multiple @projects not allowed".into());
            }
            project = Some(p.to_string());
        } else if let Some(t) = token.strip_prefix('#') {
            if t.is_empty() {
                return Err("empty tag (use #name)".into());
            }
            tags.push(t.to_string());
        } else if let Some(e) = token.strip_prefix('!') {
            let level: u8 = e
                .parse()
                .map_err(|_| format!("invalid energy level '!{e}' (use 1-5)"))?;
            if !(1..=5).contains(&level) {
                return Err(format!("energy level !{level} out of range (use 1-5)"));
            }
            energy = Some(level);
        } else {
            return Err(format!("unexpected token in metadata: '{token}'"));
        }
    }

    let project = project.ok_or("missing @project in metadata")?;
    Ok((project, tags, energy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_interval() {
        let line = "2030-01-07 09:00 - 09:45 | @work #planning | sprint review";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.date, NaiveDate::from_ymd_opt(2030, 1, 7).unwrap());
        assert_eq!(iv.start, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(iv.end, Some(NaiveTime::from_hms_opt(9, 45, 0).unwrap()));
        assert_eq!(iv.project, "work");
        assert_eq!(iv.tags, vec!["planning"]);
        assert_eq!(iv.description, "sprint review");
    }

    #[test]
    fn parse_open_interval() {
        let line = "2030-01-07 15:15 -       | @work #coding | parser refactor";
        let iv = Interval::parse(line).unwrap();
        assert!(iv.is_open());
        assert_eq!(iv.project, "work");
        assert_eq!(iv.tags, vec!["coding"]);
        assert_eq!(iv.description, "parser refactor");
    }

    #[test]
    fn parse_no_description() {
        let line = "2030-01-07 09:45 - 10:30 | @personal #errands |";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.description, "");
        assert_eq!(iv.project, "personal");
    }

    #[test]
    fn parse_multiple_tags() {
        let line = "2030-01-07 13:00 - 14:30 | @work #writing #review | draft";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.tags, vec!["writing", "review"]);
    }

    #[test]
    fn parse_no_tags() {
        let line = "2030-01-07 13:00 - 14:30 | @work | thinking";
        let iv = Interval::parse(line).unwrap();
        assert!(iv.tags.is_empty());
        assert_eq!(iv.project, "work");
    }

    #[test]
    fn round_trip() {
        let line = "2030-01-07 09:00 - 09:45 | @work #planning | sprint review";
        let iv = Interval::parse(line).unwrap();
        let serialized = iv.serialize();
        let iv2 = Interval::parse(&serialized).unwrap();
        assert_eq!(iv, iv2);
    }

    #[test]
    fn round_trip_open() {
        let line = "2030-01-07 15:15 -       | @work #coding | parser refactor";
        let iv = Interval::parse(line).unwrap();
        let serialized = iv.serialize();
        let iv2 = Interval::parse(&serialized).unwrap();
        assert_eq!(iv, iv2);
    }

    #[test]
    fn round_trip_no_description() {
        let line = "2030-01-07 09:45 - 10:30 | @personal #errands |";
        let iv = Interval::parse(line).unwrap();
        let serialized = iv.serialize();
        let iv2 = Interval::parse(&serialized).unwrap();
        assert_eq!(iv, iv2);
    }

    #[test]
    fn duration_simple() {
        let line = "2030-01-07 09:00 - 10:30 | @work #writing | draft";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.duration(), Some(Duration::minutes(90)));
    }

    #[test]
    fn duration_cross_midnight() {
        let line = "2030-01-07 23:30 - 00:15 | @work #writing | late night";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.duration(), Some(Duration::minutes(45)));
    }

    #[test]
    fn duration_open() {
        let line = "2030-01-07 15:15 -       | @work #coding | wip";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.duration(), None);
    }

    #[test]
    fn parse_with_energy() {
        let line = "2030-01-07 09:00 - 10:30 | @work #writing !4 | draft";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.energy, Some(4));
        assert_eq!(iv.project, "work");
        assert_eq!(iv.tags, vec!["writing"]);
    }

    #[test]
    fn parse_without_energy() {
        let line = "2030-01-07 09:00 - 10:30 | @work #writing | draft";
        let iv = Interval::parse(line).unwrap();
        assert_eq!(iv.energy, None);
    }

    #[test]
    fn round_trip_with_energy() {
        let line = "2030-01-07 09:00 - 10:30 | @work #writing !3 | draft";
        let iv = Interval::parse(line).unwrap();
        let serialized = iv.serialize();
        let iv2 = Interval::parse(&serialized).unwrap();
        assert_eq!(iv, iv2);
    }

    #[test]
    fn reject_invalid_energy() {
        let line = "2030-01-07 09:00 - 10:30 | @work !0 | bad";
        assert!(Interval::parse(line).is_err());
        let line = "2030-01-07 09:00 - 10:30 | @work !6 | bad";
        assert!(Interval::parse(line).is_err());
    }

    #[test]
    fn reject_multiple_projects() {
        let line = "2030-01-07 09:00 - 10:00 | @work @personal | confused";
        assert!(Interval::parse(line).is_err());
    }

    #[test]
    fn reject_missing_project() {
        let line = "2030-01-07 09:00 - 10:00 | #coding | no project";
        assert!(Interval::parse(line).is_err());
    }
}
