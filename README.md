# teum

A minimal, text-based time tracker written in Rust.

teum stores time entries as plain text files. You can read them with `cat`, search them with `grep`, edit them in `vim`, and sync them with git or iCloud.

## Quick start

```bash
cargo install --git https://github.com/yy/teum --tag v0.1.0
teum init
```

To install a local source checkout instead, run `cargo install --path .` from its root.

Edit `~/.config/teum/config.toml` to set up presets:

```toml
[presets]
focus = { project = "focus" }
build = { project = "focus", tags = ["build"] }
meet  = { project = "support", tags = ["meeting"] }
side  = { project = "side" }
```

Start tracking:

```bash
teum start -p build "prototype"          # use a preset
teum start '@focus' '#planning' roadmap       # or spell it out
teum start '@focus' '#build' '!4' prototype   # with energy level
teum status                               # what's running?
teum stop                                 # done
teum stop -e 4                            # done, with energy level
teum log                                  # today's entries
teum summary                              # this week by project
```

## Data format

Each time entry is one line of text. Three columns separated by `|`:

```
<date> <start> - <end> | <metadata> | <description>
```

A synthetic file (`2030-w02.txt`) looks like this:

```
2030-01-07 09:00 - 11:00 | @focus #build !4      | prototype
2030-01-07 11:00 - 12:30 | @support #meeting !3  | weekly sync
2030-01-07 13:30 - 15:00 | @side #learning !4    | tutorial
2030-01-07 15:15 -       | @focus #planning      | roadmap
```

- `@project` -- the area being tracked (exactly one): focus, support, side, personal
- `#tag` -- the activity type (zero or more): build, planning, meeting, learning
- `!N` -- energy level, 1 to 5 (optional)
- No end time means the timer is still running

Files are named by ISO week (`YYYY-wWW.txt`) and stored as plain `.txt`. See [docs/format.md](docs/format.md) for the full specification.

## Commands

| Command | Description |
|---------|-------------|
| `teum start` | Start tracking time |
| `teum stop` | Stop the running timer |
| `teum status [--json]` | Show what's currently running (`--json` for scripts) |
| `teum resume` | Restart the last completed timer |
| `teum cancel` | Delete the running timer |
| `teum log [period]` | Show time entries (default: today) |
| `teum summary [period]` | Show time by project with energy (default: week) |
| `teum report [period]` | Weekly category analysis: text table + HTML charts (default: all) |
| `teum edit [target]` | Open data file in `$EDITOR` |
| `teum fill` | Fill the gap since the last entry ended (start = last end, end = now) |
| `teum inject <duration>` | Inject a past interval, trimming the previous entry |
| `teum add "line"` | Add a past entry manually |
| `teum sync` | Git add, commit, pull, push |
| `teum init` | Set up data directory and config |

Periods: `today`, `yesterday`, `week`, `last-week`, `month`, `year` (and `all` for `report`)

## Analysis (`teum report`)

`teum report` rolls entries up into weekly category buckets — the same kind of
long-run tracking you'd otherwise keep in a spreadsheet, but generated straight
from the plain-text log.

```bash
teum report                              # weekly table for all data (stdout)
teum report year                         # this year only
teum report --html report.html          # write a self-contained HTML report
teum report --html                       # ...to the default ~/.config/teum/report.html
teum report --open                       # write to the default path and open it
```

The plain-text table is grep/awk-friendly:

```
week         total   focus support    side  hilite prior%    f2s
2030-w02      7:00    4:00    1:30    1:30    2:00    71%  +0.45
```

The HTML report is a single self-contained file with inline SVG charts and no external JavaScript or network access. It charts total time, priority fraction, focus versus support, focus and side hours, and highlights. Each chart includes a moving-average trendline.

Buckets are derived from `@project` / `#tag`:

| Bucket | Definition |
|--------|------------|
| Focus | projects in the `focus` report group |
| Support | projects in the `support` report group |
| Side | projects in the `side` report group |
| Excluded | projects omitted from totals |
| Highlight | any non-excluded interval tagged `#highlight` |
| Priority | focus or highlighted intervals, counted once |
| Priority fraction | priority / total |
| Focus/support | `(focus - support) / (focus + support)`, in `[-1, 1]` |

Define these groups under `[report_groups]` to map the report to your own project names.

## Grep is a feature

The format is designed for grep:

```bash
grep "@focus" 2030-w*.txt             # all focus entries
grep "#build" 2030-w*.txt             # all build sessions
grep "@focus.*#planning" *.txt        # focus planning specifically
grep "^2030-01-07" 2030-w02.txt       # everything on January 7
grep '!5' 2030-w*.txt                 # peak energy moments
grep ' -       |' 2030-w*.txt         # open (running) timers
```

## Machine-readable state (`current.json`)

teum is headless: normally the only sign a timer is running is a line with no
end time inside a weekly file. That invisibility is a real failure mode — a
timer can stay open for days unnoticed. So teum also mirrors the running timer
into a small JSON file that external tools (e.g. a desk timer like
[dial](https://github.com/yy/dial)) can watch:

```
~/.config/teum/current.json
```

It sits next to the config, **not** in `data_dir` — a running timer is a
per-machine fact, and `data_dir` may be a synced folder (iCloud) shared across
machines. The file is rewritten on every state change (start/stop/cancel/
resume/fill/inject) and refreshed by `teum status`, so it self-heals if it
drifts.

Running:

```json
{
  "tracking": true,
  "project": "focus",
  "tags": ["build"],
  "description": "prototype",
  "start": "2030-01-08T09:00:00"
}
```

Idle: `{ "tracking": false }`.

The file stores **facts, not a stale elapsed count** — consumers compute
elapsed live from `start`. For a point-in-time query, `teum status --json`
returns the same shape plus an `elapsed_seconds` snapshot:

```bash
$ teum status --json
{ "tracking": true, ..., "start": "2030-01-08T09:00:00", "elapsed_seconds": 5400 }
```

Elapsed is computed from the full start datetime. A timer left open across days reports the full duration, and `teum status` prints a warning when the start predates today.

## Sync

teum reads and writes a directory of text files. Where that directory lives determines how it syncs:

- iCloud: set `data_dir` to your iCloud Drive path. Files sync automatically, editable on iOS.
- Git: keep the data directory as a git repo. `teum sync` commits and pushes.
- Local: the default. No sync, just local files.

See [docs/sync.md](docs/sync.md) for setup details.

## Energy tracking

Every entry can include an energy level (`!1` through `!5`). Over time, this reveals patterns:

```
$ teum summary week

Week 2 (Jan 07 to Jan 13)

  focus            4h 00m   !4.0  ████████████████████
  support          1h 30m   !3.0  ████████
  side             1h 30m   !4.5  ████████
                 ───────
  total            7h 00m
```

Energy averages help compare projects and activity types over time.

See [docs/design.md](docs/design.md) for the thinking behind energy tracking and the rest of the design.

## Configuration

Config lives at `~/.config/teum/config.toml`:

```toml
# Where data files are stored
data_dir = "~/.local/share/teum"

# Sync method: "git" or "none"
sync = "none"

# Auto-commit on stop (requires sync = "git")
auto_commit = false

# Push when running `teum sync` (default: true)
auto_push = true

[presets]
focus = { project = "focus" }
build = { project = "focus", tags = ["build"] }
meet = { project = "support", tags = ["meeting"] }
side = { project = "side" }
personal = { project = "personal" }

[report_groups]
focus = ["focus"]
support = ["support"]
side = ["side"]
excluded = ["personal"]
```

See [docs/config.md](docs/config.md) for all options.

## Why teum?

Most time trackers have web dashboards, team features, API rate limits, and subscription pricing. teum is a single binary that writes text files. The data is yours, and it will never break because a company pivoted or an API changed.

See [docs/design.md](docs/design.md) for the full design rationale.

## License

MIT
