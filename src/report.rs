//! Weekly analysis: bucket time into categories, render a plain-text table and
//! a self-contained HTML report with inline-SVG charts.
//!
//! Buckets come from configurable `[report_groups]` entries:
//!   - Focus     = projects in the `focus` group
//!   - Support   = projects in the `support` group
//!   - Side      = projects in the `side` group
//!   - Excluded  = projects omitted from totals
//!   - Highlight = any non-excluded interval tagged `#highlight`
//!   - Priority  = Focus or Highlight, counted once
//!   - Total     = every non-excluded interval
//!
//! Derived indices:
//!   - Priority fraction = Priority / Total                 (0..1)
//!   - Focus-to-support = (Focus - Support) / their sum    (-1..1)

use chrono::{Datelike, NaiveDate, NaiveTime};
use std::collections::BTreeMap;

use crate::config::Config;
use crate::interval::Interval;

const DEFAULT_FOCUS: &[&str] = &["focus"];
const DEFAULT_SUPPORT: &[&str] = &["support"];
const DEFAULT_SIDE: &[&str] = &["side"];
const DEFAULT_EXCLUDED: &[&str] = &["personal"];

/// One ISO week of aggregated minutes.
#[derive(Debug, Clone)]
pub struct WeeklyStats {
    pub year: i32,
    pub week: u32,
    pub monday: NaiveDate,
    pub total: i64,
    pub focus: i64,
    pub support: i64,
    pub side: i64,
    pub highlight: i64,
    pub priority: i64,
}

impl WeeklyStats {
    fn new(year: i32, week: u32, monday: NaiveDate) -> Self {
        WeeklyStats {
            year,
            week,
            monday,
            total: 0,
            focus: 0,
            support: 0,
            side: 0,
            highlight: 0,
            priority: 0,
        }
    }

    pub fn priority_fraction(&self) -> f64 {
        if self.total > 0 {
            self.priority as f64 / self.total as f64
        } else {
            0.0
        }
    }

    /// (Focus - Support) / (Focus + Support), in [-1, 1].
    pub fn focus_to_support(&self) -> f64 {
        let denom = self.focus + self.support;
        if denom > 0 {
            (self.focus - self.support) as f64 / denom as f64
        } else {
            0.0
        }
    }
}

fn hours(minutes: i64) -> f64 {
    minutes as f64 / 60.0
}

/// Resolve a report group from config, falling back to a built-in default.
fn group_set(config: &Config, name: &str, default: &[&str]) -> Vec<String> {
    match config.report_groups.get(name) {
        Some(v) => v.clone(),
        None => default.iter().map(|s| s.to_string()).collect(),
    }
}

/// Aggregate intervals into per-ISO-week buckets, sorted chronologically.
///
/// An open (unclosed) interval is only counted if it is dated `today` — the
/// live running timer, extrapolated to `now`. An open interval on an earlier
/// day is a stale/forgotten timer, not real elapsed time, so it is skipped
/// rather than extrapolated (which would dump phantom hours into that week).
pub fn aggregate(
    intervals: &[Interval],
    config: &Config,
    today: NaiveDate,
    now: NaiveTime,
) -> Vec<WeeklyStats> {
    let focus_set = group_set(config, "focus", DEFAULT_FOCUS);
    let support_set = group_set(config, "support", DEFAULT_SUPPORT);
    let side_set = group_set(config, "side", DEFAULT_SIDE);
    let excluded_set = group_set(config, "excluded", DEFAULT_EXCLUDED);
    let is_focus = |p: &str| focus_set.iter().any(|c| c == p);
    let is_support = |p: &str| support_set.iter().any(|c| c == p);
    let is_side = |p: &str| side_set.iter().any(|c| c == p);
    let is_excluded = |p: &str| excluded_set.iter().any(|c| c == p);

    let mut weeks: BTreeMap<(i32, u32), WeeklyStats> = BTreeMap::new();

    for iv in intervals {
        let Some(dur) = iv.report_duration(today, now) else {
            continue; // stale open timer on a past day — skip
        };
        let m = dur.num_minutes();
        if m <= 0 {
            continue;
        }

        let iw = iv.date.iso_week();
        let (y, w) = (iw.year(), iw.week());
        let entry = weeks.entry((y, w)).or_insert_with(|| {
            let monday = NaiveDate::from_isoywd_opt(y, w, chrono::Weekday::Mon).unwrap_or(iv.date);
            WeeklyStats::new(y, w, monday)
        });

        let p = iv.project.as_str();
        let excluded = is_excluded(p);
        let highlight = iv.tags.iter().any(|t| t == "highlight");

        if !excluded {
            entry.total += m;
        }
        if !excluded && is_focus(p) {
            entry.focus += m;
        }
        if !excluded && is_support(p) {
            entry.support += m;
        }
        if !excluded && is_side(p) {
            entry.side += m;
        }
        if !excluded && highlight {
            entry.highlight += m;
        }
        // Priority is a union, so an interval in Focus with #highlight counts once.
        if !excluded && (is_focus(p) || highlight) {
            entry.priority += m;
        }
    }

    weeks.into_values().collect()
}

// ── Plain-text table ────────────────────────────────────────────────────────

fn hm(minutes: i64) -> String {
    let h = minutes / 60;
    let m = minutes % 60;
    format!("{h}:{m:02}")
}

/// Render the weekly table as plain text (also handy for `grep`/`awk`).
pub fn text_table(weeks: &[WeeklyStats]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<10} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>6}\n",
        "week", "total", "focus", "support", "side", "hilite", "prior%", "f2s",
    ));
    for w in weeks {
        out.push_str(&format!(
            "{:<10} {:>7} {:>7} {:>7} {:>7} {:>7} {:>6} {:>+6.2}\n",
            format!("{}-w{:02}", w.year, w.week),
            hm(w.total),
            hm(w.focus),
            hm(w.support),
            hm(w.side),
            hm(w.highlight),
            format!("{:.0}%", w.priority_fraction() * 100.0),
            w.focus_to_support(),
        ));
    }
    out
}

// ── SVG charts ──────────────────────────────────────────────────────────────

const W: f64 = 480.0;
const H: f64 = 280.0;
const ML: f64 = 48.0; // left margin
const MR: f64 = 14.0;
const MT: f64 = 14.0;
const MB: f64 = 40.0;

enum Mark {
    Dot,
    Line,
}

struct Plot {
    color: String,
    mark: Mark,
    pts: Vec<(f64, f64)>, // (x = days-since-first-monday, y = data value)
}

/// Trailing-centered moving average over the ordered (x, y) points.
fn moving_avg(pts: &[(f64, f64)], window: usize) -> Vec<(f64, f64)> {
    if pts.is_empty() {
        return Vec::new();
    }
    let half = (window / 2) as isize;
    let n = pts.len() as isize;
    (0..n)
        .map(|i| {
            let lo = (i - half).max(0);
            let hi = (i + half).min(n - 1);
            let slice = &pts[lo as usize..=hi as usize];
            let avg = slice.iter().map(|p| p.1).sum::<f64>() / slice.len() as f64;
            (pts[i as usize].0, avg)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn render_chart(
    title: &str,
    plots: &[Plot],
    ymin: f64,
    ymax: f64,
    y_ticks: &[(f64, String)],
    xmin: f64,
    xmax: f64,
    x_ticks: &[(f64, String)],
    zero_line: bool,
) -> String {
    let px = |x: f64| -> f64 {
        let span = (xmax - xmin).max(1.0);
        ML + (x - xmin) / span * (W - ML - MR)
    };
    let py = |y: f64| -> f64 {
        let span = (ymax - ymin).max(1e-9);
        H - MB - (y - ymin) / span * (H - MT - MB)
    };

    let mut s = String::new();
    s.push_str(&format!(
        "<svg viewBox=\"0 0 {W} {H}\" xmlns=\"http://www.w3.org/2000/svg\" class=\"chart\">"
    ));
    s.push_str(&format!(
        "<text x=\"{ML}\" y=\"11\" class=\"title\">{}</text>",
        esc(title)
    ));

    // y gridlines + labels
    for (yv, label) in y_ticks {
        let y = py(*yv);
        s.push_str(&format!(
            "<line x1=\"{ML}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" class=\"grid\"/>",
            W - MR
        ));
        s.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" class=\"ylab\">{}</text>",
            ML - 5.0,
            y + 3.0,
            esc(label)
        ));
    }
    // x labels
    for (xv, label) in x_ticks {
        let x = px(*xv);
        s.push_str(&format!(
            "<text x=\"{x:.1}\" y=\"{:.1}\" class=\"xlab\">{}</text>",
            H - MB + 14.0,
            esc(label)
        ));
    }
    // zero reference line
    if zero_line && ymin < 0.0 && ymax > 0.0 {
        let y = py(0.0);
        s.push_str(&format!(
            "<line x1=\"{ML}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" class=\"zero\"/>",
            W - MR
        ));
    }

    for plot in plots {
        match plot.mark {
            Mark::Line => {
                let d: String = plot
                    .pts
                    .iter()
                    .enumerate()
                    .map(|(i, (x, y))| {
                        let cmd = if i == 0 { "M" } else { "L" };
                        format!("{cmd}{:.1} {:.1} ", px(*x), py(*y))
                    })
                    .collect();
                s.push_str(&format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\" opacity=\"0.55\"/>",
                    plot.color
                ));
            }
            Mark::Dot => {
                for (x, y) in &plot.pts {
                    s.push_str(&format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"{}\"/>",
                        px(*x),
                        py(*y),
                        plot.color
                    ));
                }
            }
        }
    }
    s.push_str("</svg>");
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// x tick positions/labels: pick up to 5 real Mondays spread across the range.
fn x_ticks(weeks: &[WeeklyStats], x_of: &dyn Fn(&WeeklyStats) -> f64) -> Vec<(f64, String)> {
    if weeks.is_empty() {
        return Vec::new();
    }
    let n = weeks.len();
    let count = n.min(5);
    let mut ticks = Vec::new();
    for k in 0..count {
        let idx = if count == 1 {
            0
        } else {
            k * (n - 1) / (count - 1)
        };
        let w = &weeks[idx];
        ticks.push((x_of(w), w.monday.format("%b %d").to_string()));
    }
    ticks.dedup_by(|a, b| a.1 == b.1);
    ticks
}

/// Nice y ticks for an hours axis (0..max), stepping by ~a quarter of the max.
fn hour_ticks(max_hours: f64) -> (f64, Vec<(f64, String)>) {
    let top = (max_hours / 12.0).ceil().max(1.0) * 12.0;
    let step = top / 4.0;
    let mut ticks = Vec::new();
    let mut v = 0.0;
    while v <= top + 0.01 {
        ticks.push((v, format!("{v:.0}h")));
        v += step;
    }
    (top, ticks)
}

// ── HTML report ─────────────────────────────────────────────────────────────

const CSS: &str = r#"
:root { color-scheme: light dark; }
* { box-sizing: border-box; }
body { font: 14px/1.5 -apple-system, system-ui, sans-serif; margin: 0; padding: 28px;
       background: #fafafa; color: #1a1a1a; }
h1 { font-size: 22px; margin: 0 0 4px; }
.meta { color: #666; margin: 0 0 24px; }
.grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px; }
.card { background: #fff; border: 1px solid #e5e5e5; border-radius: 10px; padding: 8px 10px 4px; }
.legend { font-size: 12px; color: #555; margin: 2px 0 0 44px; }
.legend .dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%;
               margin: 0 4px 0 12px; vertical-align: middle; }
svg.chart { width: 100%; height: auto; display: block; }
.chart .title { font: 600 13px sans-serif; fill: #333; }
.chart .grid { stroke: #ececec; stroke-width: 1; }
.chart .zero { stroke: #999; stroke-width: 1; stroke-dasharray: 3 3; }
.chart .ylab { font: 10px sans-serif; fill: #999; text-anchor: end; }
.chart .xlab { font: 10px sans-serif; fill: #999; text-anchor: middle; }
table { border-collapse: collapse; width: 100%; margin-top: 8px; font-variant-numeric: tabular-nums; }
th, td { padding: 4px 8px; text-align: right; border-bottom: 1px solid #eee; white-space: nowrap; }
th:first-child, td:first-child { text-align: left; }
th { font-weight: 600; color: #555; }
.tablewrap { background:#fff; border:1px solid #e5e5e5; border-radius:10px; padding:6px 12px; overflow-x:auto; margin-top:18px; }
h2 { font-size: 15px; margin: 28px 0 0; }
@media (prefers-color-scheme: dark) {
  body { background: #16171a; color: #e6e6e6; }
  .card, .tablewrap { background: #1e1f23; border-color: #303236; }
  .meta, th { color: #9aa0a6; }
  .chart .title { fill: #ddd; } .chart .grid { stroke: #2a2c30; }
  th, td { border-color: #2a2c30; }
}
"#;

const BLUE: &str = "#3b6fd4";
const RED: &str = "#c0392b";

fn legend(items: &[(&str, &str)]) -> String {
    let mut s = String::from("<div class=\"legend\">");
    for (label, color) in items {
        s.push_str(&format!(
            "<span class=\"dot\" style=\"background:{color}\"></span>{}",
            esc(label)
        ));
    }
    s.push_str("</div>");
    s
}

/// Build a complete self-contained HTML report.
pub fn html_report(weeks: &[WeeklyStats], range_label: &str) -> String {
    let x_of = |w: &WeeklyStats| -> f64 {
        let first = weeks.first().map(|f| f.monday).unwrap_or(w.monday);
        (w.monday - first).num_days() as f64
    };
    let xmin = 0.0;
    let xmax = weeks.last().map(&x_of).unwrap_or(1.0);
    let xt = x_ticks(weeks, &x_of);

    // Shared maxima keep related hour charts comparable.
    let max_hours_total = weeks.iter().map(|w| hours(w.total)).fold(0.0_f64, f64::max);
    let max_hours_fs = weeks
        .iter()
        .flat_map(|w| [hours(w.focus), hours(w.support)])
        .fold(0.0_f64, f64::max);
    let max_hours_side = weeks
        .iter()
        .flat_map(|w| [hours(w.focus), hours(w.side)])
        .fold(0.0_f64, f64::max);
    let max_hours_highlight = weeks
        .iter()
        .map(|w| hours(w.highlight))
        .fold(0.0_f64, f64::max);

    // 1. Priority fraction (0..1) with trendline.
    let priority_pts: Vec<(f64, f64)> = weeks
        .iter()
        .map(|w| (x_of(w), w.priority_fraction()))
        .collect();
    let c_priority = render_chart(
        "Priority fraction",
        &[
            Plot {
                color: BLUE.into(),
                mark: Mark::Line,
                pts: moving_avg(&priority_pts, 5),
            },
            Plot {
                color: BLUE.into(),
                mark: Mark::Dot,
                pts: priority_pts.clone(),
            },
        ],
        0.0,
        1.0,
        &[
            (0.0, "0".into()),
            (0.25, ".25".into()),
            (0.5, ".5".into()),
            (0.75, ".75".into()),
            (1.0, "1".into()),
        ],
        xmin,
        xmax,
        &xt,
        false,
    );

    // 2. Focus vs. support index (-1..1) with zero line and trendline.
    let f2s_pts: Vec<(f64, f64)> = weeks
        .iter()
        .map(|w| (x_of(w), w.focus_to_support()))
        .collect();
    let c_f2s = render_chart(
        "Focus vs. support",
        &[
            Plot {
                color: BLUE.into(),
                mark: Mark::Line,
                pts: moving_avg(&f2s_pts, 5),
            },
            Plot {
                color: BLUE.into(),
                mark: Mark::Dot,
                pts: f2s_pts.clone(),
            },
        ],
        -1.0,
        1.0,
        &[
            (-1.0, "-1".into()),
            (-0.5, "-.5".into()),
            (0.0, "0".into()),
            (0.5, ".5".into()),
            (1.0, "1".into()),
        ],
        xmin,
        xmax,
        &xt,
        true,
    );

    // 3. Focus and support hours.
    let (fs_top, fs_ticks) = hour_ticks(max_hours_fs);
    let c_fs = render_chart(
        "Focus & support",
        &[
            Plot {
                color: BLUE.into(),
                mark: Mark::Line,
                pts: moving_avg(
                    &weeks
                        .iter()
                        .map(|w| (x_of(w), hours(w.focus)))
                        .collect::<Vec<_>>(),
                    5,
                ),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Line,
                pts: moving_avg(
                    &weeks
                        .iter()
                        .map(|w| (x_of(w), hours(w.support)))
                        .collect::<Vec<_>>(),
                    5,
                ),
            },
            Plot {
                color: BLUE.into(),
                mark: Mark::Dot,
                pts: weeks.iter().map(|w| (x_of(w), hours(w.focus))).collect(),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Dot,
                pts: weeks.iter().map(|w| (x_of(w), hours(w.support))).collect(),
            },
        ],
        0.0,
        fs_top,
        &fs_ticks,
        xmin,
        xmax,
        &xt,
        false,
    );

    // 4. Focus and side hours.
    let (side_top, side_ticks) = hour_ticks(max_hours_side);
    let c_side = render_chart(
        "Focus & side",
        &[
            Plot {
                color: BLUE.into(),
                mark: Mark::Line,
                pts: moving_avg(
                    &weeks
                        .iter()
                        .map(|w| (x_of(w), hours(w.focus)))
                        .collect::<Vec<_>>(),
                    5,
                ),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Line,
                pts: moving_avg(
                    &weeks
                        .iter()
                        .map(|w| (x_of(w), hours(w.side)))
                        .collect::<Vec<_>>(),
                    5,
                ),
            },
            Plot {
                color: BLUE.into(),
                mark: Mark::Dot,
                pts: weeks.iter().map(|w| (x_of(w), hours(w.focus))).collect(),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Dot,
                pts: weeks.iter().map(|w| (x_of(w), hours(w.side))).collect(),
            },
        ],
        0.0,
        side_top,
        &side_ticks,
        xmin,
        xmax,
        &xt,
        false,
    );

    // 5. Highlighted hours with trendline.
    let highlight_pts: Vec<(f64, f64)> = weeks
        .iter()
        .map(|w| (x_of(w), hours(w.highlight)))
        .collect();
    let (highlight_top, highlight_ticks) = hour_ticks(max_hours_highlight);
    let c_highlight = render_chart(
        "Highlights",
        &[
            Plot {
                color: RED.into(),
                mark: Mark::Line,
                pts: moving_avg(&highlight_pts, 5),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Dot,
                pts: highlight_pts.clone(),
            },
        ],
        0.0,
        highlight_top,
        &highlight_ticks,
        xmin,
        xmax,
        &xt,
        false,
    );

    // 6. Total tracked time with trendline.
    let total_pts: Vec<(f64, f64)> = weeks.iter().map(|w| (x_of(w), hours(w.total))).collect();
    let (total_top, total_ticks) = hour_ticks(max_hours_total);
    let c_total = render_chart(
        "Total tracked",
        &[
            Plot {
                color: RED.into(),
                mark: Mark::Line,
                pts: moving_avg(&total_pts, 5),
            },
            Plot {
                color: RED.into(),
                mark: Mark::Dot,
                pts: total_pts.clone(),
            },
        ],
        0.0,
        total_top,
        &total_ticks,
        xmin,
        xmax,
        &xt,
        false,
    );

    let fs_leg = legend(&[("focus", BLUE), ("support", RED)]);
    let side_leg = legend(&[("focus", BLUE), ("side", RED)]);

    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    html.push_str("<title>teum — weekly analysis</title>");
    html.push_str(&format!("<style>{CSS}</style></head><body>"));
    html.push_str("<h1>teum — weekly analysis</h1>");
    html.push_str(&format!(
        "<p class=\"meta\">{} · {} weeks</p>",
        esc(range_label),
        weeks.len()
    ));

    html.push_str("<div class=\"grid\">");
    for (chart, leg) in [
        (&c_total, None),
        (&c_priority, None),
        (&c_f2s, None),
        (&c_fs, Some(&fs_leg)),
        (&c_side, Some(&side_leg)),
        (&c_highlight, None),
    ] {
        html.push_str("<div class=\"card\">");
        html.push_str(chart);
        if let Some(l) = leg {
            html.push_str(l);
        }
        html.push_str("</div>");
    }
    html.push_str("</div>");

    // table
    html.push_str("<h2>Weekly table</h2><div class=\"tablewrap\"><table><thead><tr>");
    for h in [
        "week",
        "total",
        "focus",
        "support",
        "side",
        "highlight",
        "priority%",
        "focus/support",
    ] {
        html.push_str(&format!("<th>{h}</th>"));
    }
    html.push_str("</tr></thead><tbody>");
    for w in weeks {
        html.push_str("<tr>");
        html.push_str(&format!("<td>{}-w{:02}</td>", w.year, w.week));
        for v in [w.total, w.focus, w.support, w.side, w.highlight] {
            html.push_str(&format!("<td>{}</td>", hm(v)));
        }
        html.push_str(&format!("<td>{:.0}%</td>", w.priority_fraction() * 100.0));
        html.push_str(&format!("<td>{:+.2}</td>", w.focus_to_support()));
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div>");
    html.push_str("</body></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn iv(date: &str, mins: i64, project: &str, tags: &[&str]) -> Interval {
        let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        Interval {
            date: d,
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap() + chrono::Duration::minutes(mins)),
            project: project.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            energy: None,
            description: String::new(),
        }
    }

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn buckets_and_indices() {
        let now = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2030, 1, 11).unwrap();
        let ivs = vec![
            iv("2030-01-07", 120, "focus", &["build"]),
            iv("2030-01-07", 60, "support", &["routine"]),
            iv("2030-01-07", 30, "support", &["highlight"]),
            iv("2030-01-07", 45, "side", &["build"]),
            iv("2030-01-07", 90, "personal", &["highlight"]),
        ];
        let weeks = aggregate(&ivs, &cfg(), today, now);
        assert_eq!(weeks.len(), 1);
        let w = &weeks[0];
        assert_eq!(w.total, 255);
        assert_eq!(w.focus, 120);
        assert_eq!(w.support, 90);
        assert_eq!(w.highlight, 30);
        assert_eq!(w.side, 45);
        assert_eq!(w.priority, 150);
        assert!((w.priority_fraction() - 150.0 / 255.0).abs() < 1e-9);
        assert!((w.focus_to_support() - 30.0 / 210.0).abs() < 1e-9);
    }

    #[test]
    fn configured_group_names_are_used() {
        let mut config = cfg();
        config
            .report_groups
            .insert("focus".into(), vec!["project-a".into()]);
        config
            .report_groups
            .insert("excluded".into(), vec!["off-clock".into()]);
        let today = NaiveDate::from_ymd_opt(2030, 1, 11).unwrap();
        let now = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let intervals = vec![
            iv("2030-01-07", 60, "project-a", &[]),
            iv("2030-01-07", 30, "off-clock", &["highlight"]),
        ];

        let weeks = aggregate(&intervals, &config, today, now);

        assert_eq!(weeks[0].total, 60);
        assert_eq!(weeks[0].focus, 60);
        assert_eq!(weeks[0].priority, 60);
    }

    #[test]
    fn stale_open_timer_is_skipped_not_extrapolated() {
        let today = NaiveDate::from_ymd_opt(2030, 1, 19).unwrap();
        let now = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        // An open timer from a past day (no end): must contribute zero.
        let mut open = iv("2030-01-12", 0, "support", &["routine"]);
        open.end = None;
        let closed = iv("2030-01-12", 90, "support", &["routine"]);
        let weeks = aggregate(&[open, closed], &cfg(), today, now);
        assert_eq!(weeks.len(), 1);
        assert_eq!(weeks[0].support, 90);
    }

    #[test]
    fn live_open_timer_today_is_counted() {
        let today = NaiveDate::from_ymd_opt(2030, 1, 19).unwrap();
        let now = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let mut open = iv("2030-01-19", 0, "focus", &["build"]);
        open.start = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        open.end = None;
        let weeks = aggregate(&[open], &cfg(), today, now);
        assert_eq!(weeks[0].focus, 30);
    }

    #[test]
    fn moving_avg_smooths() {
        let pts = vec![(0.0, 0.0), (1.0, 10.0), (2.0, 0.0)];
        let ma = moving_avg(&pts, 3);
        assert_eq!(ma.len(), 3);
        // centered window of 3 at the middle point averages all three
        assert!((ma[1].1 - 10.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn html_is_self_contained() {
        let now = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let ivs = vec![iv("2030-01-07", 120, "focus", &["build"])];
        let today = NaiveDate::from_ymd_opt(2030, 1, 11).unwrap();
        let weeks = aggregate(&ivs, &cfg(), today, now);
        let html = html_report(&weeks, "all");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<svg"));
        assert!(!html.contains("http://") || html.contains("w3.org")); // only the svg ns url
    }
}
