# Personal OEE

A Mac desktop app that tracks the working day in blocks (20 minutes by default) and scores it with OEE (Availability × Efficiency × Quality).
The full requirements are in [`docs/PRD.md`](docs/PRD.md), the app's task list in [`docs/DESKTOP_BACKLOG.md`](docs/DESKTOP_BACKLOG.md), and the command-line contract in [`docs/CLI_API.md`](docs/CLI_API.md).

## Status

Everything in the backlog is built. What's left are checks by hand in the running app, listed under "Not checked" in each backlog item.

| Part | State |
|---|---|
| `oee-core`: rules, OEE maths, weekly pooling, calendar, day timeline, live tracker, goals, trends, CSV export | ✅ Built |
| `oee-store`: JSON day, calendar and settings files, lock file, clock, shared week and export logic | ✅ Built |
| Mac app: window, menu-bar timer, global downtime shortcut, reminders, Day / Week / Calendar / Trends / Settings tabs, mini widget | ✅ Built |
| `office-oee` command-line tool for agents and tests | ✅ Built |

What the app does:

- **Live day:** start the day, toggle downtime with a global shortcut (or the menu bar), and watch blocks fill in as Work, Lost or Planned. Breaks and meetings come from the weekly calendar.
- **Edit blocks:** categorise lost time (System Down, Change Over, Idle, Management, Wait), mark Rework, add notes, change several blocks at once.
- **Week and trends:** pooled weekly OEE, loss bars, over-capacity days, a weekly OEE goal with a streak, and 4 / 12 / 26-week trends.
- **Settings:** day start and finish, daily target, cycle time (block length), downtime shortcut, weekly goal.
- **Export:** days and blocks CSV files, and a spreadsheet-totals CSV laid out to paste into a tracking spreadsheet.

## Versions

The current version is in `Cargo.toml`. After each completed change the patch number goes up by 0.0.1 (`scripts/bump-version.sh`); the owner decides when a bigger release is made (PRD NFR-003).

## Build and run

You'll need Rust installed (`curl https://sh.rustup.rs -sSf | sh`).

```sh
scripts/bundle.sh            # builds dist/PersonalOEE.app
scripts/bundle.sh debug      # adds practice mode (a fake clock)
```

Data is saved in `~/Library/Application Support/PersonalOEE/`. To try the app on throwaway data:

```sh
OEE_DATA_DIR="$PWD/examples/week" dist/PersonalOEE.app/Contents/MacOS/PersonalOEE
```

Closing the window hides it; the app keeps running in the menu bar. Other launch options: `PERSONAL_OEE_HIDDEN=1` (start in the menu bar only), `PERSONAL_OEE_MINI=1` (open the mini widget), `PERSONAL_OEE_THEME`.

## Command-line tool

`office-oee` reads and writes the same files as the app, gives JSON by default (`--text` for a short summary), and takes a fixed clock so results repeat exactly. Writes need `--data-dir <path>` (a test folder) or `--real` (the app's folder).

```sh
office-oee --data-dir examples/week --date 2026-11-04 report week --text
office-oee --data-dir examples/week export csv --from 2026-11-02 --to 2026-11-06 --stdout totals
office-oee commands          # every command and flag, as JSON
```

While the app is open it holds a lock file, and CLI writes are refused unless `--force` is given; the app then reloads the changed day.

## Run the tests

```sh
cargo test
```

The tests in `oee-core/tests/appendix_a.rs` check the engine against the Week 4 data in PRD Appendix A. The CLI tests in `oee-cli/tests/` run the binary against a temporary data folder.

## Layout

```
oee-core/     pure logic, no I/O
  src/rules.rs      defaults, lost-block counting, daily target
  src/category.rs   the five lost-time categories, loss minutes
  src/metrics.rs    daily Availability, Efficiency, Quality, OEE; holiday and zero-work days
  src/week.rs       pooled weekly report, over-capacity list
  src/goal.rs       weekly OEE goal and streak
  src/trends.rs     weekly series for the Trends tab
  src/calendar.rs   weekly Planned Downtime calendar
  src/timeline.rs   a day's blocks from downtime stops, planned windows and edits
  src/tracker.rs    live state machine: shortcut toggle, planned auto-switch, reminders
  src/export.rs     days, blocks and spreadsheet-totals CSV
oee-store/    JSON files, lock file, clock, settings, week and export helpers shared by app and CLI
oee-cli/      the office-oee binary and its command catalogue
oee-app/      the egui app: main loop, screens, tray, hotkey, mini widget
examples/     a sample week of data
docs/         PRD, desktop backlog, CLI contract
```

## Mac stack

- **eframe / egui**: native Rust UI, no web frontend to maintain.
- **global-hotkey**: the system-wide downtime shortcut (FR-011a). On macOS this needs Accessibility permission the first time.
- **tray-icon**: menu-bar timer showing the current block and mode.
- **rfd**: the folder picker for CSV export.
- **JSON files** in `~/Library/Application Support/PersonalOEE/` for storage.
