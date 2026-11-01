# CLI API proposal — `office-oee`

**Status:** Decisions made (section 10). C-01, the crate extraction, is done; the rest is not built.
**Goal:** let an LLM agent (or a script) seed data, run a day, make the common edits and read every figure the app shows, from a terminal, without the UI, the menu-bar item or the global shortcut.
**Reviewed:** `oee-core` (all modules), `oee-app` `store.rs`, `settings.rs`, `start.rs`, `export_screen.rs`, `week_screen.rs`, `main.rs` (open, load, save, roll-over), `docs/PRD.md` and `docs/DESKTOP_BACKLOG.md`, as of D-56.

---

## 1. What the review found

### 1.1 Good news: the engine is already CLI-shaped

- `oee-core` is pure logic with no I/O. Time is passed in as minutes since midnight, so a CLI can drive any clock it likes.
- `Tracker::resume(record, settings, now)` rebuilds the live state (Work, Downtime, Planned, Ended) from a saved `DayRecord`. So the CLI can be **stateless**: each command loads the day, resumes at `now`, applies one action, saves, prints. No daemon needed.
- Every UI action has a core call: `start_day`, `toggle_downtime_report`, `extend_to`, `end_now`, `finalise`, `set_type`, `categorise`, `set_rework`, `set_note`, `apply_bulk`, `day_result`, `week_report`, `weekly_series`, `goal_status`, `goal_streak`, `days_csv`, `blocks_csv`. Calendar add and remove are on `WeeklyCalendar`.
- Everything that matters is already `serde`-serialisable (`DayRecord`, `WeeklyCalendar`, `Settings`), and saving is atomic (write temp, then rename).

### 1.2 Gaps to close before a CLI is reliable

| # | Gap | Where | Why it matters for a CLI |
|---|---|---|---|
| G1 | **No library crate for the non-UI app logic.** `Store`, `AppSettings`/`GoalSetting`, `planned_for`, `parse_hhmm`, `build_days`, `resolve` and the week and loss-bar builders live in `oee-app`, a binary crate that also pulls in egui, tray, hotkey and notifications. | `oee-app/src/*.rs` | The CLI can't import them, and copying them would let the CLI and the app drift apart. |
| G2 | **Data folder is hard-coded** to `~/Library/Application Support/PersonalOEE` (`Store::default_location`). | `store.rs` | Tests would read and write the user's real data. An agent needs a throwaway sandbox. |
| G3 | **The clock is the system clock.** `now_minutes()` reads `Local::now()`. Only a debug-build "practice" offset exists. | `main.rs`, `practice.rs` | Agents can't test 10:05–10:30 downtime without waiting 25 minutes. Every command needs an explicit `--now`. |
| G4 | **The running app doesn't re-read today's file.** It loads at launch and on day roll-over, and writes only when its in-memory JSON differs from `last_saved`. | `main.rs` `load_day`, `save_if_changed` | A CLI edit to today's record while the app is open is silently overwritten by the app's next change. See section 6. |
| G5 | **`settings.rs` mixes data and UI** (`AppSettings`, `Shortcut::to_hotkey`, `SettingsForm`, egui `show`). | `settings.rs` | The data and validation part must move to the library; the egui and hotkey part stays. |
| G6 | **Block addressing is by start minute (`u32`).** | `timeline.rs` | The CLI needs a stable, readable form: `HH:MM` within a date. |
| G7 | **Weekly and trend assembly is in the screens** (`WeekView::build`, `report`, `over_capacity_rows`, `loss_bars`). | `week_screen.rs` | The CLI's `report week` must show the same numbers as the Week tab, so the builders should be shared, not rewritten. |
| G8 | **No schema or version marker in the day files.** Older files load through `#[serde(default)]`. | `timeline.rs` | Fine for the app. A CLI that validates data (`office-oee check`) wants a `version` field, or at least a documented shape. |

---

## 2. Design principles (made for LLM callers)

1. **JSON on stdout, always.** One JSON object per command, never mixed with prose. `--text` gives a short human summary instead. Logs and warnings go to stderr.
2. **No prompts, no interactivity, no colour.** Everything a command needs is a flag. A missing flag is a usage error, not a question.
3. **Explicit clock.** `--now` (or `OEE_NOW`) overrides the clock on every command. Same inputs always give the same outputs.
4. **Data location is always explicit for writes.** A write command needs either `--data-dir <path>` (a test folder) or `--real` (the app's real folder). With neither, it refuses with exit code 7 and tells the caller which to pick. Reads default to the real folder.
5. **Self-describing.** `office-oee commands` lists every command with its flags as JSON; `office-oee schema <command>` gives the output shape. An agent can discover the API without reading this file.
6. **Actionable errors.** Every error is JSON with a stable `code`, a plain-words `message`, and often a `hint` naming the fix.
7. **Same engine as the app.** The CLI only parses arguments, calls the shared library and prints. No OEE maths lives in the CLI.
8. **Safe to retry.** Read commands change nothing. Write commands return the new state, so a retry can be compared. Destructive ones need `--yes`.

---

## 3. Crate layout

```
oee-core/     unchanged: pure rules and maths
oee-store/    NEW (extracted from oee-app): Store, AppSettings, GoalSetting, start/planned helpers,
              export range + build_days, week/loss/over-capacity builders, data-dir resolution
oee-cli/      NEW: binary `office-oee`; clap parsing, JSON output, exit codes
oee-app/      egui only; depends on oee-store; keeps tray, hotkey, notifications, screens
```

**Built (C-01):** `oee-store` holds `store`, `settings` (data and validation; `Shortcut::validate`), `start`, `tabs` and `export_range`. The app keeps the Settings screen, the `Shortcut` to system-hotkey conversion and the export panel. The week and loss-bar builders are still in `oee-app` because they are tangled with the screens' colours and tiles; they are split out as part of C-08, when the CLI first needs them (G7).

`oee-store` keeps the UI-free pieces and has no egui, chrono-clock or OS dependencies beyond `chrono` and `serde_json`. The app's existing tests for those pieces move with them. `Shortcut` data stays in `oee-store` (it is stored in `settings.json`); only `to_hotkey` stays in the app.

---

## 4. Conventions

### 4.1 Global flags

| Flag | Env | Meaning |
|---|---|---|
| `--data-dir <path>` | `OEE_DATA_DIR` | A test folder holding `days/`, `calendar.json`, `settings.json`. Created on first write. Required for writes unless `--real` is given. |
| `--real` | | Use the app's real folder, `~/Library/Application Support/PersonalOEE`. Required for writes when there is no `--data-dir`. Giving both is a usage error. |
| `--now <when>` | `OEE_NOW` | The clock. Accepts `HH:MM` (today's date) or `YYYY-MM-DDTHH:MM`. Seconds are ignored (the engine works in whole minutes). |
| `--date <YYYY-MM-DD>` | `OEE_DATE` | The day a command works on. Defaults to the date of `--now`, else today. |
| `--text` | | Human summary instead of JSON. |
| `--dry-run` | | Run a write command fully, print the result, save nothing. |
| `--yes` | | Confirm a destructive command (`day delete`, `data reset`). |

Time of day is always `HH:MM` (24-hour). Dates are always `YYYY-MM-DD`. Percentages are numbers `0–1` in JSON, with a `display` string ("54.0%") alongside where useful, matching the CSV convention (D-51).

### 4.2 Addressing blocks

A block is identified by its start time on the chosen date: `--block 10:40`. Several: `--blocks 10:40,11:00` or a range `--blocks 10:40..11:20` (every block starting in that span). `office-oee block list` returns each block's `start`, so an agent never has to calculate.

### 4.3 Output envelope

Success:

```json
{ "ok": true, "command": "day toggle", "date": "2026-11-02", "now": "10:25",
  "result": { ... }, "notices": [ ... ] }
```

Failure (exit code non-zero, JSON on stdout, nothing on stderr except warnings):

```json
{ "ok": false, "command": "block categorise",
  "error": { "code": "not_lost_time",
             "message": "only lost-time blocks can be categorised",
             "hint": "Run `office-oee block list` and pick a block with kind \"lost\"." } }
```

`notices` carries the tracker's `Notice` values (planned window started or ended, categorise reminder, day ended), so an agent sees what the app would have shown.

### 4.4 Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 2 | Usage error (bad flag, bad time format) |
| 3 | Not found (no day record, no such block, no such calendar entry) |
| 4 | Rule refused (e.g. rework on a lost block, toggle during Planned, day already started) |
| 5 | Data problem (unreadable or invalid file; see `office-oee check`) |
| 6 | App is open and the write was refused (see section 6) |
| 7 | No data location chosen for a write: pass `--data-dir <path>` or `--real` |
| 1 | Anything else |

Error codes map straight from core: `EditError::NoSuchBlock` → `no_such_block` (3), `NotLostTime` → `not_lost_time` (4), `NotWork` → `not_work` (4).

---

## 5. Commands

Each row says what the UI does today and which core call backs it, so the CLI can't drift from the app.

### 5.1 Day lifecycle

| Command | Does | Backed by | Notes |
|---|---|---|---|
| `office-oee day start [--at HH:MM] [--end HH:MM]` | Start the day. Snaps to the 00/20/40 grid, takes today's planned windows from the calendar. | `Tracker::start_day`, `start::planned_for` | D-48, D-49. Refuses (code `day_exists`) if the date already has a record. |
| `office-oee day status` | Mode, current block, time left in block, since-when for downtime, today's metrics so far. | `Tracker::resume` + `inputs_at` | The "Today" screen as JSON. |
| `office-oee day toggle` | Downtime ⇄ Work. Returns the stop summary when a stop ends. | `toggle_downtime_report` | Same message data as D-18: `{minutes, lost_blocks}`. Refused during Planned or after the day ends. |
| `office-oee day downtime start` / `stop` | Explicit forms of the toggle. Refuse if already in that state. | `toggle_downtime_report` | Safer for agents than a blind toggle. Preferred in scripts. |
| `office-oee day extend --to HH:MM` | Move the finish later; reopens an ended day. | `extend_to` | FR-006. |
| `office-oee day end` | End the day now. | `end_now` | D-27. |
| `office-oee day finish` | Finalise: uncategorised lost blocks default to System Down. | `finalise` | FR-027. |
| `office-oee day show [--date]` | The full record: start, end, planned, downtime events, every block, metrics. | `DayRecord`, `blocks_until`, `day_result` | The past-day view as JSON. |
| `office-oee day list [--from --to]` | Saved dates with status and OEE. | `Store::list_days` | |
| `office-oee day delete --date D --yes` | Remove a saved day. | `Store` | New (the UI has no delete today). Makes test cleanup easy. |

### 5.2 Blocks (editing)

| Command | Does | Backed by |
|---|---|---|
| `office-oee block list [--date]` | Every block: `start`, `end`, `kind` (work / lost / planned), `category`, `rework`, `note`, `uncategorised`, `counts_minutes`. | `blocks_until` |
| `office-oee block set-type --block T --type work\|lost\|break\|meeting` | Change type. | `set_type` (D-50) |
| `office-oee block reset-type --block T` | Go back to what the events and calendar say. | `set_type(None)` |
| `office-oee block categorise --blocks T,.. --category system-down\|change-over\|idle\|management\|wait` | Categorise lost blocks. | `categorise` / `apply_bulk` |
| `office-oee block rework --blocks T,.. [--off]` | Mark or unmark Rework. | `set_rework` / `apply_bulk` |
| `office-oee block note --block T [--text "…"]` | Set a note; omit `--text` to clear. | `set_note` |
| `office-oee block bulk --blocks T,.. (--type … \| --category … \| --rework on\|off)` | One change to many. Returns `{applied, skipped}` and each skipped block with a reason. | `apply_bulk` (D-54) |
| `office-oee block uncategorised [--date]` | Lost blocks still waiting for a category. | `uncategorised` |

Category and type names are the kebab-case form of the PRD labels. `office-oee commands` lists the allowed values.

### 5.3 Calendar and settings

| Command | Does | Backed by |
|---|---|---|
| `office-oee calendar list` | Entries per weekday, with ids. | `WeeklyCalendar::entries_for` |
| `office-oee calendar add --day mon --from HH:MM --to HH:MM --kind break\|meeting --label "Lunch"` | Add a window; snapped to the grid like the UI. | `add`, `timeline::snap` (D-32) |
| `office-oee calendar remove --id N` | Remove by id. | `remove` (D-30) |
| `office-oee settings get` | Day start, end, target, shortcut, weekly goal. | `AppSettings` |
| `office-oee settings set [--start --end --target --goal N --goal off]` | Same validation as the Settings tab. | `SettingsForm::parse` logic, moved to `oee-store` (G5) |

### 5.4 Reports (read-only)

| Command | Does | Backed by |
|---|---|---|
| `office-oee report day [--date]` | Availability, Efficiency, Quality, OEE, status (normal / holiday / zero-work), over-capacity flag, target, inputs. | `day_result`, `Metrics` |
| `office-oee report week [--week-of D]` | The Week tab: per-day rows, pooled week metrics, loss minutes per category, over-capacity days, goal status and streak. | `week_report`, `goal_status`, shared builders (G7) |
| `office-oee report trends [--weeks 4\|12\|26]` | One pooled point per working week. | `weekly_series` (D-55) |
| `office-oee report goal [--week-of D]` | Status (on track / behind / met / not met) and streak. | `goal_status`, `goal_streak` (D-56) |
| `office-oee report losses [--from --to]` | Minutes and blocks lost per category. | `LossMinutes` |
| `office-oee export csv --from D --to D --out <dir>` | Writes the days and blocks CSVs exactly as the Export panel does; `--stdout days\|blocks` prints one instead. | `days_csv`, `blocks_csv`, `build_days` (D-51, D-52) |

### 5.5 Testing and seeding (the part built for agents)

| Command | Does |
|---|---|
| `office-oee data seed --scenario <name>` | Fill the data dir with a known dataset. Proposed scenarios: `appendix-a` (PRD Week 4, expected 58.4%), `empty`, `holiday-week`, `zero-work-day`, `over-capacity`, `uncategorised-day`, `streak-3`. |
| `office-oee data reset --yes` | Delete everything in the data dir. On the real folder it needs `--real` and `--yes`. |
| `office-oee sim run <scenario.json>` | Run a scripted day in one call (see 5.6) and print the final state plus a step log. |
| `office-oee check` | Read every file in the data dir, report unreadable files, and recompute each day to confirm the invariants below. |
| `office-oee commands` | JSON list of all commands, flags, value enums and exit codes. |
| `office-oee schema <command>` | JSON Schema of that command's `result`. |
| `office-oee version` | CLI version, core version, data-file version. |

`office-oee check` invariants: blocks tile the day exactly (`slot_count` × 20 minutes); no block is both lost and planned; a lost block's category is set, or the day is not finalised; metrics are within range and Efficiency may exceed 1 only with the over-capacity flag; the stored target matches `Settings::target_points`.

### 5.6 Scripted simulation

The main reason for the CLI: let an agent replay a day deterministically, then assert on the result.

```json
{
  "date": "2026-11-02",
  "settings": { "start": "09:00", "end": "17:40", "target": 20 },
  "calendar": [ { "day": "mon", "from": "12:00", "to": "13:00", "kind": "break", "label": "Lunch" } ],
  "steps": [
    { "at": "09:00", "do": "day.start" },
    { "at": "10:05", "do": "downtime.start" },
    { "at": "10:30", "do": "downtime.stop" },
    { "at": "17:40", "do": "day.end" },
    { "do": "block.categorise", "blocks": "10:00", "category": "wait" },
    { "do": "day.finish" }
  ],
  "expect": { "oee": 0.540, "tolerance": 0.0005, "lost_blocks": 1 }
}
```

Steps without `at` reuse the previous time. Output has `steps` (each step's result and notices), `final` (the `report day` object) and `expect` (pass/fail per key). This also lets the existing PRD examples (20 − 1 rework → 92.9% Quality) become one-file regression tests.

---

## 6. Running next to the desktop app

The app holds the day in memory and writes it back when it changes, but never re-reads it (G4). Three safe options, in order of cost:

1. **Default for tests: sandbox.** Agents use `--data-dir`. The real app is never involved. Nothing else is needed.
2. **Same folder, app closed.** Fine. The CLI writes atomically, so a later launch sees the result.
3. **Same folder, app open.** To allow this safely:
   - The app takes an advisory lock on `<data-dir>/.lock` while running. A CLI **write** command finds the lock and exits with code 6 (`app_running`) unless `--force` is given. Reads are always allowed.
   - The app watches the modified time of today's file (checked in the existing once-a-second repaint) and reloads it when it changes and it has no unsaved change of its own. This is a small separate backlog item.

Recommendation: ship 1 and 2 first, add the lock (refusal only) with the CLI, and treat the reload as a later item.

---

## 7. Example agent session

```sh
export OEE_DATA_DIR=/tmp/oee-test   # or pass --real to work on the real data
office-oee data reset --yes
office-oee calendar add --day mon --from 12:00 --to 13:00 --kind break --label Lunch
office-oee day start --date 2026-11-02 --now 09:00
office-oee day downtime start --now 10:05
office-oee day downtime stop  --now 10:30     # result.stopped = { minutes: 25, lost_blocks: 1 }
office-oee block list --date 2026-11-02 --now 10:30
office-oee block categorise --blocks 10:00 --category wait --now 10:30
office-oee report day --date 2026-11-02 --now 17:40
office-oee export csv --from 2026-11-02 --to 2026-11-02 --stdout days
```

---

## 8. Out of scope

- The global shortcut, menu-bar item, notifications and window. The CLI never needs them. A "toggle from the shell" is just `office-oee day toggle`, so a user could bind it to any launcher later.
- Running the live timer. There is no background process; time only moves when `--now` moves.
- Remote access, auth, or an HTTP/MCP server. If wanted later, an MCP wrapper can call the same CLI JSON, which is why the JSON contract matters.
- Editing the PRD rules. The CLI exposes them; it does not add any.

---

## 9. Suggested backlog items

Proposed as a new milestone **M5 — CLI**. Each is test-first, in the same style as M0–M4.

- [x] **C-01** Extract `oee-store` from `oee-app` (G1, G5). *Done: 141 tests before and after (109 in the app, 32 in the new crate); `oee-store` has no egui, hotkey or OS dependencies. The week builders (G7) move in C-08.*
- [x] **C-02** Data-dir resolution: `OEE_DATA_DIR`, `--data-dir` and `--real`, with writes refused (exit 7) when none is given (G2). The app honours `OEE_DATA_DIR` too. Done when: a test with a temp dir never touches `~/Library`.
- [ ] **C-03** Clock injection: a `Clock` that is the system clock, `--now`, or `OEE_NOW` (G3). Done when: the same command with the same `--now` gives byte-identical JSON.
- [ ] **C-04** `oee-cli` skeleton: parsing, envelope, error codes, exit codes, `commands`, `schema`, `version`.
- [ ] **C-05** Day lifecycle commands (5.1) with tests against `Tracker`.
- [ ] **C-06** Block commands (5.2), including bulk with the skipped list.
- [ ] **C-07** Calendar and settings commands (5.3).
- [ ] **C-08** Report and export commands (5.4); `report week` equals the Week tab's figures on the Appendix A data (58.4%).
- [ ] **C-09** `data seed` scenarios, `data reset`, `check` (5.5).
- [ ] **C-10** `sim run` (5.6), with the PRD worked examples as scenario files.
- [ ] **C-11** App lock file and CLI refusal (section 6, option 3), then, separately, reload of a changed today file in the app.
- [ ] **C-12** Add a version field to day, calendar and settings files (G8), loading older files unchanged.

Suggested order: C-01 (done) → C-02 → C-03 → C-04 → C-05 → C-08 (the read side proves the numbers) → C-06 → C-07 → C-09 → C-10 → C-11 → C-12.

---

## 10. Decisions

1. **Binary name:** `office-oee`.
2. **Output:** JSON by default; `--text` for a human summary.
3. **Real versus test data:** writes must say which, with `--real` (the app's folder) or `--data-dir <path>` (a test folder). No flag means refusal (exit 7), not a guess.
4. **Crate extraction first:** done as C-01 (see section 3).
5. **Open:** whether to ship the lock file (C-11) with the first CLI release.
