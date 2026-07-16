use chrono::Duration;

pub fn duration_str(d: Duration) -> String {
    let total_minutes = d.num_minutes();
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    format!("{hours}:{minutes:02}")
}

pub fn duration_long(d: Duration) -> String {
    let total_minutes = d.num_minutes();
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

/// Like [`duration_long`] but surfaces whole days, so a stale open timer reads
/// "5d 1h 24m" instead of a deceptively small "1h 24m" (time-of-day only) or an
/// unreadable "121h 24m". Used for "N ago" elapsed reporting.
pub fn duration_ago(d: Duration) -> String {
    let total_minutes = d.num_minutes().max(0);
    let days = total_minutes / (60 * 24);
    let hours = (total_minutes % (60 * 24)) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes:02}m")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

pub fn bar(minutes: i64, max_minutes: i64, width: usize) -> String {
    if max_minutes == 0 {
        return String::new();
    }
    let filled = ((minutes as f64 / max_minutes as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    "\u{2588}".repeat(filled)
}
