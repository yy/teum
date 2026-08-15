# Configuration

teum reads its configuration from `~/.config/teum/config.toml`. The file is created with commented-out defaults when you run `teum init`.

If `XDG_CONFIG_HOME` is set, the config is at `$XDG_CONFIG_HOME/teum/config.toml`.

## Options

### `data_dir`

Where data files are stored.

```toml
data_dir = "~/.local/share/teum"
```

Default: `~/.local/share/teum` (or `$XDG_DATA_HOME/teum` if set).

Tilde expansion is supported. To use iCloud sync, point this to your iCloud Drive:

```toml
data_dir = "~/Library/Mobile Documents/com~apple~CloudDocs/teum"
```

### `sync`

Sync method. Affects `teum init` (whether to `git init` the data directory) and is checked by `teum sync`.

```toml
sync = "none"
```

Values:
- `"none"` (default) -- no sync. Data stays local.
- `"git"` -- the data directory is a git repo. `teum sync` runs add/commit/pull/push.

### `auto_commit`

Automatically git-commit after `teum stop`. Only applies when `sync = "git"`.

```toml
auto_commit = false
```

Default: `false`

### `auto_push`

Whether `teum sync` pushes after committing and rebasing. Set this to `false`
when you want synchronization to stop after the local commit and pull.

```toml
auto_push = true
```

Default: `true`

### `highlight_tags`

Tags that make an interval count toward the weekly report's highlight and
priority buckets. Any interval carrying at least one of these tags is
highlighted; the tags are matched exactly, so list every variant you use.

```toml
highlight_tags = ["highlight", "improving"]
```

Default: `["highlight"]`

## Presets

Presets map short names to a project and optional tags. They save typing on the most common `teum start` invocations.

```toml
[presets]
focus = { project = "focus" }
build = { project = "focus", tags = ["build"] }
meet = { project = "support", tags = ["meeting"] }
side = { project = "side" }
personal = { project = "personal" }
```

Usage:

```bash
teum start -p build "prototype"
# equivalent to: teum start @focus #build prototype
```

Energy and description can be added alongside presets:

```bash
teum start -p build '!4' "prototype"
```

### Preset fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project` | string | yes | The `@project` name |
| `tags` | array of strings | no | Tags to apply (without `#` prefix) |

## Report groups

Named groups of projects filter `teum summary`. The weekly report also reads four group names: `focus`, `support`, `side`, and `excluded`. Their default project names match the group names, but you can map them to any private project names in your local config. Add `#highlight` to an interval when you want it included in the report's priority measure, or set `highlight_tags` to recognize your own tags instead.

```toml
[report_groups]
focus = ["focus", "project-a"]
support = ["support"]
side = ["side"]
excluded = ["personal"]
```

Usage:

```bash
teum summary week --group focus
# only shows projects assigned to the focus group
```

## Full example

```toml
data_dir = "~/.local/share/teum"
sync = "git"
auto_commit = true
auto_push = true

[presets]
focus = { project = "focus" }
build = { project = "focus", tags = ["build"] }
plan = { project = "focus", tags = ["planning"] }
meet = { project = "support", tags = ["meeting"] }
side = { project = "side" }
personal = { project = "personal" }

[report_groups]
focus = ["focus"]
support = ["support"]
side = ["side"]
excluded = ["personal"]
```
