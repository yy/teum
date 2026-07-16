# Design principles

## Core philosophy

teum exists because time tracking should be a solved problem. You start a timer, you stop it, you look at where the time went. The tool should be fast, quiet, and permanent.

Three principles guide every decision:

1. Text is the universal interface. If your data is plain text, most tools can work with it. grep, awk, sed, sort, wc are all time tracking analysis tools.

2. Data outlives software. teum will be abandoned or replaced. The data format must survive that. Someone in 2040 should be able to open a `.txt` file and understand what's in it without running any software.

3. The default should be silence. Track time, report when asked. No reminders, no gamification, no streak counters.

## Why not Toggl / Clockify / etc.

Cloud time trackers work until they don't:

- API rate limits throttle your own tools
- Pricing changes or shutdowns strand your data
- The apps grow features you don't need, making the thing you do need slower
- You can't work offline or on a plane
- Your data lives on someone else's server

teum is 1600 lines of Rust that writes text files to a folder. No network, no account.

## Why not timewarrior

timewarrior is the closest existing tool to what teum does. It's good software. But:

- It's ~30,000 lines of C++ for a problem that needs ~1,600 lines
- The data format uses compact UTC timestamps (`20300107T140000Z`) that are unreadable without tooling
- It carries features most individual users don't need (exclusions, holidays, chart rendering)
- Building from source requires CMake and a C++ toolchain

teum borrows timewarrior's best idea (one line per interval, monthly files, plain text) and strips away everything else.

## The data format

### Human-readable, not machine-optimal

The format is:

```
2030-01-07 09:00 - 10:30 | @focus #build !4 | prototype
```

Every design choice favors readability:

- Full ISO dates (`2030-01-07`) instead of compact (`20300107`). You can read them.
- Local time instead of UTC. A personal tracker used by one person doesn't need timezone normalization. If you travel, the times still make sense as "what the clock on the wall said."
- Minute precision instead of seconds. Most activities do not need second-level precision, and dropping seconds saves 6 characters per line.
- Pipe separators create visual columns. The three sections (time, metadata, description) are immediately distinguishable.
- Plain `.txt` extension so every device opens them without file association setup.

### The `@` / `#` / `!` system

The metadata section uses three sigils:

- `@project` -- area being tracked (focus, support, side, personal). Exactly one per entry.
- `#tag` -- type of activity (build, planning, meeting, learning). Zero or more.
- `!N` -- energy level, 1 through 5. Optional.

This maps to the Toggl model (projects + tags), with energy as the addition. The sigils keep searches short: `grep "@focus"` and `grep "#build"` do what you expect.

### Why `@` for projects and `#` for tags (not the reverse)

In most systems (Twitter, GitHub, Slack), `@` addresses an entity and `#` classifies. A project is an entity you direct time toward; a tag classifies what you did. The convention maps.

### Energy tracking

Time shows how many hours went to a project. Energy adds another comparison: morning sessions may average !4 while late sessions average !2. You can use that pattern when planning future work.

The scale is simple:

| Level | Meaning |
|-------|---------|
| !1 | Drained, forcing it |
| !2 | Low, going through the motions |
| !3 | Neutral, steady |
| !4 | Good, focused |
| !5 | Peak, flow state |

It's optional. If you forget to log energy for a week, the tool doesn't complain and the existing data isn't degraded.

### Weekly files

Data is stored in weekly files named `YYYY-wWW.txt` (ISO week numbers).

Why weekly instead of daily or monthly:

| | Daily | Weekly | Monthly |
|---|---|---|---|
| Files per year | ~365 | ~52 | 12 |
| File size | Tiny (~5 lines) | Small (~35 lines) | Medium (~150 lines) |
| Git diffs | Minimal | Small | Larger |
| Merge conflicts | Almost impossible | Very rare | Occasional |
| Mobile editing | Trivial | Easy | Scroll to end |
| Natural summary unit | Too granular | Fits most review cycles | Too coarse |

Weekly files are small enough to edit on a phone, few enough to stay tidy, and aligned with how most people review their time.

### Open intervals

A running timer is represented as a line with no end time:

```
2030-01-07 15:15 -       | @focus #planning | roadmap
```

When you run `teum stop`, the end time is filled in by editing this line in place. Data files are not strictly append-only -- the last line may be modified. A separate state file would preserve append-only semantics but adds complexity and a new failure mode (state file out of sync with data).

## The CLI

### Command design

Every command maps to one action and finishes immediately. There are no daemons, no background processes, no watch modes.

The core loop is four commands: `start`, `stop`, `status`, `resume`. Everything else is querying or maintenance.

### Auto-stop

Running `teum start` while a timer is already running will stop the current timer and start the new one. This removes the most common friction point in time tracking: forgetting to stop before starting something new.

### Presets

Presets map short names to project + tags:

```bash
teum start -p build "prototype"
# expands to: @focus #build prototype
```

Presets keep common commands short. `teum start -p build` expands to the configured project and tags; the full line with pipes is reserved for `teum add` when backfilling.

## Sync

teum doesn't have sync. It has files. Where you put those files determines how they sync:

- iCloud Drive: automatic sync, works on iOS via Files app
- Git repo: explicit sync with `teum sync`, version history for free
- Dropbox, Syncthing, rsync: all work because it's just files

Sync is a solved problem. Time tracking is a different problem.

## What teum will not become

- A team tool. teum is for one person tracking their own time. Multi-user features are a different category of software.
- A billing tool. No invoicing, no rates, no currency. Export to a spreadsheet if you need to bill.
- A calendar. teum records what happened, not what should happen.
- A web app. The data is files. Build a web interface on top if you want one.
