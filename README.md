# Personal OEE

A Mac desktop app that tracks the working day in 20-minute blocks and scores it with OEE (Availability × Efficiency × Quality). The full requirements are in [`docs/PRD.md`](docs/PRD.md).

## Status

| Part | State |
|---|---|
| `oee-core`: rules, OEE maths, weekly pooling, calendar, day timeline, live tracker | ✅ Built, 25 tests passing |
| Storage (SQLite) | Next |
| Mac app: window, menu-bar timer, global shortcut, reminders, reports | Next |

## Run the tests

You'll need Rust installed (`curl https://sh.rustup.rs -sSf | sh`).

```sh
cargo test
```

The tests in `oee-core/tests/appendix_a.rs` check the engine against the Week 4 data in PRD Appendix A.

## Layout

```
oee-core/
  src/rules.rs      constants, lost-block counting, daily target
  src/category.rs   the five lost-time categories, loss minutes
  src/metrics.rs    daily Availability, Efficiency, Quality, OEE; holiday and zero-work days
  src/week.rs       pooled weekly report, over-capacity list
  src/calendar.rs   weekly Planned Downtime calendar
  src/timeline.rs   a day's blocks from downtime stops, planned windows and edits
  src/tracker.rs    live state machine: shortcut toggle, planned auto-switch, reminders
  tests/            rule, timeline and Appendix A tests
docs/PRD.md         requirements
```

## Planned Mac stack

- **eframe / egui**: native Rust UI, no web frontend to maintain.
- **global-hotkey**: the system-wide downtime shortcut (FR-011a). On macOS this needs Accessibility permission the first time.
- **tray-icon**: menu-bar timer showing the current block and mode.
- **rusqlite**: local storage in `~/Library/Application Support/PersonalOEE/`.
- **macOS notifications**: categorisation reminders (FR-026).
