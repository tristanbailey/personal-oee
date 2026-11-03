# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Personal OEE is a Mac desktop app (Rust, eframe/egui) that tracks a working day in 20-minute blocks and scores it with OEE. Requirements are in `docs/PRD.md` (FR-/NFR- IDs); the app's task list is `docs/DESKTOP_BACKLOG.md` (D-xx IDs); the CLI contract is `docs/CLI_API.md` (C-xx IDs). Code comments cite these IDs. The README's status table is out of date.

## Commands

```sh
cargo test                                   # whole workspace
cargo test -p oee-app a_new_day_does_not     # one test, by crate and name filter
scripts/bundle.sh [debug]                    # build dist/PersonalOEE.app; debug adds practice mode (fake clock)
scripts/bump-version.sh                      # patch version +0.0.1 (see Rules)
OEE_DATA_DIR="$PWD/examples/week" dist/PersonalOEE.app/Contents/MacOS/PersonalOEE   # app on throwaway data
office-oee --data-dir examples/week --date 2026-11-04 report week --text            # CLI on throwaway data
```

Useful env vars: `OEE_DATA_DIR` (data folder), `OEE_NOW` (fixed clock for the CLI), `PERSONAL_OEE_HIDDEN=1` (start in menu bar only), `PERSONAL_OEE_MINI=1`, `PERSONAL_OEE_THEME`.

## Architecture

Cargo workspace of four crates, layered:

- `oee-core`: pure logic, no I/O. `DayRecord` (`timeline.rs`) stores only `start`, `end`, planned windows, downtime events and per-block `edits`. **Blocks are derived** from that on a 20-minute grid, never stored, so any change to a day is a change to those fields. `Tracker` (`tracker.rs`) is the live state machine (Work / Downtime / Planned / Ended) driven by `tick(now)`. `rules.rs`, `metrics.rs`, `week.rs` hold the OEE maths (checked against PRD Appendix A in `tests/appendix_a.rs`).
- `oee-store`: JSON files in the data dir (`days/YYYY-MM-DD.json`, `calendar.json`, settings), lock file, clock, and the "start day" helpers shared by app and CLI.
- `oee-cli`: the `office-oee` binary. Command definitions, the machine-readable catalogue (`catalogue.rs`) and the tests that run the binary against a temp data dir (`oee-cli/tests`). A new command needs an entry in `catalogue.rs` and `docs/CLI_API.md`.
- `oee-app`: the egui app. `main.rs` holds `App` and the big `update()` loop (roll-over at a new date, disk reload, tick, screens); the other files are screens and widgets. Closing the window hides it; the process keeps running in the menu bar. Logic that lives inline in `update()` is hard to test, so pull it into a pure function when you change it.

The app keeps today's day in memory and saves on change; it also reloads if the file changed on disk (CLI `--force` edits). Day files are plain JSON; `data_format` changes only when the file layout does, and new fields use `#[serde(default)]` so old files still load.

## Rules

- **Versioning (NFR-003):** every completed change raises the patch version in the same commit via `scripts/bump-version.sh` (edits `Cargo.toml` and `scripts/bundle.sh`). Never bump minor or major; the owner decides.
- Run `cargo test` before committing. Commit only when asked.
- `personal-oee-prd.md` in the repo root is an untracked scratch file; leave it out of commits (use `git add -u`).
- Use a throwaway data folder (`OEE_DATA_DIR`, `--data-dir`) for experiments, not the real one in `~/Library/Application Support/PersonalOEE/`.
