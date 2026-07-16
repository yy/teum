# CLI reference

## teum start

Start tracking time.

```
teum start [OPTIONS] [ARGS]...
```

**Options:**
- `-p, --preset <NAME>` -- use a preset from config
- `-a, --at <HH:MM>` -- override start time (default: now)

**Arguments:** `@project #tag !energy description`

If a timer is already running, it is automatically stopped before the new one starts.

**Examples:**

```bash
teum start -p build "prototype"                # preset
teum start @focus #planning roadmap            # explicit
teum start @focus #build '!4' prototype        # with energy
teum start -p meet --at 14:00 "weekly sync"    # override time
```

---

## teum stop

Stop the running timer.

```
teum stop [OPTIONS]
```

**Options:**
- `-a, --at <HH:MM>` -- override stop time (default: now)
- `-e, --energy <N>` -- energy level, 1--5

Errors if nothing is running.

**Examples:**

```bash
teum stop                    # stop now
teum stop -e 3               # stop with energy level
teum stop --at 17:00 -e 4    # retroactive stop with energy
```

---

## teum status

Show what's currently running, or the last completed entry if nothing is active.

```
teum status
```

**Output when tracking:**

```
Tracking: @focus #build
          prototype
Started:  15:15 (47m ago)
```

**Output when idle:**

```
No active tracking.
Last:     14:30 - 15:00 | @support #meeting
```

---

## teum resume

Start a new timer with the same project, tags, energy, and description as the last completed entry.

```
teum resume [OPTIONS]
```

**Options:**
- `-a, --at <HH:MM>` -- override start time

If a timer is running, it is stopped first.

---

## teum cancel

Delete the currently running timer entirely (removes the line from the data file).

```
teum cancel
```

---

## teum log

Show time entries for a period.

```
teum log [PERIOD]
```

**Period:** `today` (default), `yesterday`, `week`, `last-week`, `month`

**Output:**

```
Today (2030-01-07, Mon)

  09:00 - 11:00  @focus #build !4                prototype                     2:00
  11:00 - 12:30  @support #meeting !3            weekly sync                   1:30
  13:30 - 15:00  @side #learning !4              tutorial                      1:30
  15:15 - ...    @focus #planning                roadmap                       0:45 (running)
                                                                         ─────
                                                                          5:45
```

---

## teum summary

Show time aggregated by project, with optional energy averages.

```
teum summary [PERIOD] [OPTIONS]
```

**Period:** `today`, `week` (default), `last-week`, `month`

**Options:**
- `-g, --group <NAME>` -- filter by a report group from config

**Output:**

```
Week 2 (Jan 07 to Jan 13)

  focus            4h 00m   !4.0  ████████████████████
  side             1h 30m   !4.0  ████████
  support          1h 30m   !3.0  ████████
                 ───────
  total            7h 00m
```

Energy averages are shown only when at least one entry in the period has an energy level.

---

## teum edit

Open a data file in your `$EDITOR`.

```
teum edit [TARGET]
```

**Target:** `current` (default) or a specific week like `2030-w02`

---

## teum add

Add a completed interval manually (backfilling).

```
teum add <LINE>
```

The line must be a complete interval with an end time (no open intervals).

**Examples:**

```bash
teum add "2030-01-07 09:00 - 10:30 | @focus #build !4 | prototype"
```

---

## teum sync

Commit and push data via git. The data directory must be a git repo.

```
teum sync
```

Runs: `git add -A`, `git commit`, `git pull --rebase`, `git push`

---

## teum init

Create the data directory and a default config file.

```
teum init
```

If `sync = "git"` is set in config, also runs `git init` in the data directory.
