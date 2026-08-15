# Changelog

All notable changes to teum are documented here. The project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `highlight_tags` config option, so the weekly report's highlight and priority
  buckets can recognize tags other than `#highlight`.

### Changed

- Widen the weekly table's priority column and label it `priority%` rather than
  the abbreviated `prior%`, which also realigns the header with its rows.

### Fixed

- Record the true start second in `current.json`, so a live readout counts from
  zero instead of from however far into the minute the timer began. The weekly
  log stays minute-resolution; `teum status` now carries the mirrored seconds
  forward instead of rounding them away on every reconcile.

## [0.1.1] - 2026-07-17

### Fixed

- Keep the active timer running when replacement `teum start` arguments are
  invalid.
- Reject interval lines that omit the `-` time separator instead of silently
  treating them as open timers.

## [0.1.0] - 2026-07-16

Initial public release.

### Fixed

- Make weekly log rewrites atomic and serialize every reader and writer through
  a shared lock protocol.
- Reject invalid configuration instead of silently writing to the default data
  directory.
- Reject ambiguous or longer-than-24-hour timer closures.
- Find stale timers across the complete data directory and reject multiple open
  timers.
- Return a failing exit status when git commit, pull, or push fails.
- Implement documented `auto_commit`, document `auto_push`, and keep runtime
  lock files out of git history.
- Exclude stale open timers from log and summary totals.
- Enforce the documented project, tag, energy, and injection-duration syntax.

[Unreleased]: https://github.com/yy/teum/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/yy/teum/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/yy/teum/releases/tag/v0.1.0
