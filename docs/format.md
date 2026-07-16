# Data format specification

Any tool that reads or writes teum data files should follow this spec.

## File naming

```
YYYY-wWW.txt
```

- `YYYY` -- ISO year (may differ from calendar year at year boundaries)
- `WW` -- ISO week number, zero-padded (01--53)
- `.txt` -- plain text, always

Examples: `2030-w02.txt`, `2030-w01.txt`, `2029-w52.txt`

Files live in the data directory (default: `~/.local/share/teum/`).

## File contents

Each file contains zero or more interval lines, one per line. Lines are ordered chronologically. Blank lines and lines starting with `#` (comments) are ignored.

## Interval line format

```
<date> <start> - <end> | <metadata> | <description>
```

### Fields

#### Date

```
YYYY-MM-DD
```

ISO 8601 date in local time. Example: `2030-01-07`

#### Start time

```
HH:MM
```

24-hour local time, zero-padded. Example: `09:00`, `14:30`

#### End time

```
HH:MM
```

Same format as start time. If the interval is still running (open), the end time is omitted and the space is left blank:

```
2030-01-07 15:15 -       | @focus #planning | roadmap
```

Only an omitted end time creates an open interval. Any other non-`HH:MM`
content is invalid and produces an error.

#### Cross-midnight intervals

If the end time is earlier than the start time, the interval crosses midnight. The end time refers to the following day. The entry is stored in the file corresponding to the start date's week.

```
2030-01-07 23:30 - 00:15 | @focus #build | late session
```

This interval is 45 minutes long (23:30 to 00:15 the next day).

#### Pipe separators

The three sections are separated by ` | ` (space, pipe, space). A trailing `|` after metadata (with no description following) is permitted:

```
2030-01-07 09:45 - 10:30 | @support #routine |
```

#### Metadata

The metadata section contains tokens separated by whitespace:

| Token | Meaning | Cardinality |
|-------|---------|-------------|
| `@name` | Project / area of work | Exactly one, required |
| `#name` | Activity tag | Zero or more |
| `!N` | Energy level (1--5) | Zero or one |

Token order within the metadata section does not matter, but the conventional order is `@project #tags !energy`.

**Project names** and **tag names** are lowercase, may contain letters, numbers, and hyphens. No spaces.

Valid: `@focus`, `#planning`, `#code-review`
Invalid: `@Focus`, `#project planning`, `@`

**Energy level** is an integer from 1 to 5 inclusive.

Valid: `!1`, `!3`, `!5`
Invalid: `!0`, `!6`, `!high`, `!`

At most one energy token is allowed.

#### Description

Free-form text after the second pipe. May contain any characters. Leading and trailing whitespace is trimmed. The description is optional; an empty description is represented by an empty string (or a trailing pipe with nothing after it).

## Examples

A complete day:

```
2030-01-07 09:00 - 10:30 | @focus #build !4 | prototype
2030-01-07 10:30 - 11:00 | @support #routine |
2030-01-07 11:00 - 12:00 | @support #meeting !3 | weekly sync
2030-01-07 13:00 - 14:30 | @side #learning !4 | tutorial
2030-01-07 14:30 - 15:00 | @personal #errand !3 | groceries
2030-01-07 15:15 -       | @focus #planning !4 | roadmap
```

## Parsing rules

1. Trim the line. Skip if empty or starts with `#` (comment, not tag -- comments only at the start of a line).
2. Split on ` | ` with a limit of 3 segments.
3. Parse segment 1 as time (date, start, optional end).
4. Parse segment 2 as metadata (project, tags, energy). Trim trailing `|` if present.
5. Segment 3, if present, is the description (trimmed).

## Serialization rules

1. Date: `%Y-%m-%d`
2. Times: `%H:%M`
3. Open intervals: 5 spaces where the end time would be
4. Metadata: `@project #tag1 #tag2 !N` (energy omitted if not set)
5. If description is empty: end with ` |`
6. If description is present: end with ` | <description>`
7. Each line ends with a newline (`\n`)
