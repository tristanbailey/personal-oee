# Desktop App Backlog — from the Web Prototype

**Source:** the web prototype (`web/index.html`, published as the private "Personal OEE" page) and `docs/PRD.md`.
**Target:** `oee-app` (Rust, eframe/egui 0.29) on top of `oee-core`.
**Last updated:** 2026-11-01
**Versioning (NFR-003):** after each task or change is completed, raise the patch version by 0.0.1 in the same commit (`scripts/bump-version.sh`). The owner decides minor and major releases.

Each item has an ID (D-xx), the PRD requirements it covers, and a "Done when" check. Items are grouped into milestones in build order. M0 fixes bugs in what exists today. M1 makes the app usable day to day. M2 brings in the week and calendar screens. M4 adds the efficiency features picked after the first month of use. M3 adds the Mac-only features the prototype had to leave out. M5 adds a command-line tool for agents and tests.

---

## What the app has today

| Area | In `oee-app` now | In the prototype |
|---|---|---|
| Start day | Button, starts at the current time, no planned windows | Start-time field, uses today's calendar entries |
| Mode and toggle | Text label and button | Stack light, big status, block countdown, big toggle, **D** key |
| Blocks | Plain list of finished blocks, category drop-down, Rework tick box | Hour grid of 3 blocks per row, colour by type, tap to open an editor with notes |
| Metrics | Sidebar, whole-number percentages | 4 tiles, 1 decimal place, "so far today" note |
| End of day | Finalise, +20 min, New day | Extend, End day now, Finish day with a reminder banner |
| Saving | None, lost on quit | Every change saved |
| Week, Calendar | None | Full screens |

---

## M0 — Fix what exists

- [x] D-01 Fix live metrics counting future blocks as Work. `Tracker::inputs()` uses `record.blocks()`, which includes blocks that haven't happened yet. Add `DayRecord::inputs_until(&Settings, until)` in oee-core: day minutes, planned minutes and blocks all clipped to `until`. Use it for the Today screen while the day is running (FR-044, FR-048). Done when: at 10:00 on a fresh 09:00 day, Efficiency shows 3 of 20 points, not 26 of 20.
- [x] D-02 Show running downtime in the shift log and metrics before it's toggled off. Add a provisional event `(since, now)` when building blocks while in `Mode::Downtime` (FR-013, FR-017). Done when: 25 minutes into a stop, the log shows 1 lost block and Availability has dropped.
- [x] D-03 Show percentages to 1 decimal place to match the PRD and the Appendix A tests. Done when: Week 4 shows 58.4%, not 58%.
- [x] D-04 Load today's planned windows from the weekly calendar at Start day. The app currently passes `Vec::new()` (FR-030, FR-033). Done when: with a 12:00–13:00 lunch in the calendar, the app switches to Break at 12:00. Depends on D-30.

## M1 — Today screen and saving

### Saving

- [x] D-10 Add `serde` derives to `DayRecord`, `DowntimeEvent`, `PlannedWindow`, `PlannedKind`, `LostCategory`, `WeeklyCalendar` and `Settings`. The private `edits` map must be included. Done when: a record round-trips through JSON with categories, rework and notes intact.
- [x] D-11 Add `finalised: bool` to `DayRecord`, set by `finalise()`. The prototype uses it to hide the Finish day banner once done (FR-027).
- [x] D-12 Add `Tracker::resume(record, settings, now)` that rebuilds the mode from a saved record, including running downtime (`down_since`) and whether the day has ended. Done when: quitting mid-downtime and reopening shows Downtime with the original start time.
- [x] D-13 Save each day to `~/Library/Application Support/PersonalOEE/days/YYYY-MM-DD.json`, and the calendar to `calendar.json`. Write after every change: start, toggle, edit, extend, end, finish. Use write-to-temp then rename so a crash can't corrupt a file. Done when: quitting at any point and reopening loses nothing.
- [x] D-14 On launch, open today's record if it exists, otherwise show the Not started screen. Remove the "New day" button. A new date starts a new record automatically.

### Andon panel

- [x] D-15 Add a stack light: three lamps, red = Downtime, amber = Planned, green = Work, all off = Not started or Day ended. Done when: each mode lights exactly one lamp, or none.
- [x] D-16 Add a big status word (WORK / DOWNTIME / BREAK / MEETING / DAY ENDED / NOT STARTED), the current clock, and a sub-line that changes with the mode. Done when each mode shows:
  - Work: "Block 7 of 26 · 12 min left".
  - Downtime: "Since 10:05 · 7 min". Under 5 minutes it adds "under 5 min is ignored" (FR-014).
  - Planned: "Lunch · until 13:00, then back to Work" (FR-037).
- [x] D-17 Add a full-width toggle button: "Start downtime" in red during Work, "Back to work" in green during Downtime, a disabled "Planned downtime" during Planned, and "Extend 20 min" once the day has ended (FR-011, FR-006).
- [x] D-18 Show a short message after a stop ends, using the result of the toggle. Done when: a 3-minute stop shows "3 min stop ignored. It counts as Work.", and a 25-minute stop shows "25 min stop logged as 1 lost block. Categorise it later." Needs `toggle_downtime` to also return the stop's length and lost-block count.

### Start screen (data entry)

- [x] D-19 Not started screen: a start-time field prefilled with the current time and editable to an earlier time (FR-005), a line listing today's planned windows or "No planned breaks or meetings for today. Add them in Calendar.", and a Start day button. Done when: entering 08:40 gives a day of 08:40–17:40 with a target of 21 (FR-047).

### Metrics tiles

- [x] D-20 Replace the sidebar with four tiles: Availability ("40 min lost of 460"), Efficiency ("14 of 20 points"), Quality ("2 rework"), and OEE in an inverted dark tile, captioned "So far today" while running and "A × E × Q" once ended (FR-040).
- [x] D-21 Show an Over capacity banner when Efficiency is above 100% (FR-048). Show a 0% day banner when the day has ended with time logged but no Work points (FR-045a).

### Shift log

- [x] D-22 Replace the block list with an hour grid: hour label on the left, three 20-minute blocks per row, each showing its start time and label. Colours: Work green, Lost red, Change Over amber, Planned blue. Uncategorised lost blocks are striped red with a dashed outline. Future blocks are dashed outlines. The current block has a dark outline. Purple dot for Rework, grey dot for a note. Done when: a 520-minute day shows 9 rows ending at 17:20.
- [x] D-23 Add a legend row above the grid, and a header with the day range and block count, e.g. "09:00–17:40 · 26 blocks". Add "+ 10 min" when the day length isn't a whole number of blocks.
- [x] D-24 Block labels: "Work", "Rework", the category name, "Categorise" for uncategorised, the planned window's name such as "Lunch", "Downtime…" for the current block during a stop, and "In progress" for the current block during Work.

### Block editor (data entry)

- [x] D-25 Clicking a finished, planned or current block opens a modal editor titled "Lost time", "Work" or "Planned downtime", with the block's time range. Esc or Cancel closes it without saving.
  - Lost: five buttons, one per category, each showing its name and description, with the selected one highlighted (FR-012). Change Over is highlighted amber.
  - Work: a "Mark as Rework (errors or corrections)" tick box, with the note "Rework still scores 1 Work point. It counts as half a loss in Quality." (FR-019, FR-022).
  - Planned: read-only text naming the planned window.
  - All types: a Note box (optional, max 300 characters) and Save / Cancel buttons (FR-025).
- [x] D-26 Saving calls `categorise`, `set_rework` and `set_note`. A blank note clears it. Any `EditError` is shown in plain words and nothing is saved. Then save the day (D-13).

### End of day

- [x] D-27 Add a row of controls under the grid: "Extend 20 min" always, and "End day now" until the day has ended. Ending now sets the finish time to the current time, which lowers the target (FR-047a). Needs `Tracker::end_now(now)` in oee-core. Done when: ending at 17:00 on a 09:00 day gives a target of 18.
- [x] D-28 Categorisation banners (FR-026):
  - During Planned Downtime: "N lost blocks need a category. Click the striped blocks below."
  - At end of day: "N lost blocks need a category. Anything left becomes System Down when you finish.", with a Finish day button that calls `finalise()` (FR-027).
  - If everything is already categorised: "Day ended. Everything is categorised." with Finish day.

## M2 — Week, Calendar and past days

- [x] D-29 Add three tabs across the top: Today, Week, Calendar. Remember the last tab between launches.

### Calendar screen (data entry)

- [x] D-30 Save the weekly calendar to `calendar.json`, and give each entry an id so it can be removed reliably. `WeeklyCalendar::remove` currently works by index (FR-030, FR-034, FR-035).
- [x] D-31 List Mon–Sun as cards. Each entry shows "12:00–13:00 · Break · Lunch" with a Remove button. Empty days show "Nothing planned". Add a header note: "Repeats every week. Applies to days you start after a change."
- [x] D-32 For entries that aren't on the 20-minute grid from 09:00, show how they'll count, e.g. "Counts as 09:20–10:00 (40 min) on a 09:00 start" (FR-038). Needs the private `snap` function in `timeline.rs` made public.
- [x] D-33 Add-entry form with these fields: Day (Mon–Sun, or "Mon–Fri" to add five entries), From, To, Type (Meeting or Break), Name (optional, max 40 characters), and an Add button. Done when: an end time not after the start shows "The end time must be after the start time." and nothing is added.

### Week screen

- [x] D-34 Add week navigation: ‹ and › buttons around a title like "19 Oct – 25 Oct", with a sub-line "Pooled from N working days, holidays left out" or "No working days this week" (FR-051, FR-052).
- [x] D-35 Show four pooled tiles for the week using `week_report`, with the OEE tile captioned "Week, pooled" (FR-041, FR-053).
- [x] D-36 Add a day table with columns Day, Status, Avail., Effic., Quality, OEE and Points ("18/20"), plus a bold Week total row. Status pills: Worked (green), Over capacity (blue), 0% day (red), Holiday (grey). Show Mon–Fri always, and Sat and Sun only when they have a record. Clicking a row opens that day (D-39).
- [x] D-37 Add a loss breakdown: one horizontal bar per category (Change Over in amber) scaled to the largest, with minutes on the right, plus a Rework bar showing the block count (FR-042).
- [x] D-38 Add an over-capacity review list of every flagged day across all saved records, newest first, with columns Day, Points, Efficiency and "Review →". Clicking opens that day. If there are none, show "No over-capacity days yet." (FR-043).

### Past day view

- [x] D-39 Open any past date in the Today layout without the andon panel: a "← Back to today" button, a title like "Tue 20 Oct", a status pill, the tiles, and the shift log with the same block editor. If uncategorised blocks are left, show a banner with Finish day. Edits save to that day's file. A date with no record shows "Nothing logged on Tue 20 Oct. It counts as a holiday."

## M3 — Mac-only features (left out of the prototype)

- [x] D-40 Register a system-wide shortcut that toggles downtime even when the app isn't the active window, using the `global-hotkey` crate. Default to ⌃⌥D, changeable in settings. Inside the window, **D** also works (FR-011a, NFR-002). Done when: the shortcut toggles downtime with Safari in front. *Status: registered at launch with no error and wired to the same toggle as the button; pressing it with another app in front still needs a manual check.*
- [x] D-41 Add a menu-bar item using the `tray-icon` crate, showing a coloured dot for the mode and the time left in the block, e.g. "● 12m". Its menu has Start/stop downtime, Open window and Quit. *Status: dot and countdown seen working in the menu bar; the menu items are wired but still need a manual click-through.*
- [x] D-42 Send macOS notifications for each `Notice`: planned downtime started or ended, the categorisation reminder, and day ended (FR-026). Clicking a notification opens the window. *Status: built and attributed to the app bundle (macOS showed its notification-permission prompt); the banner text and click-to-open still need a manual check after allowing notifications. Run `scripts/bundle.sh` and open `dist/PersonalOEE.app`.*
- [x] D-43 Keep the timer running with the window closed, so the app lives in the menu bar and doesn't quit when the window closes. After the Mac wakes from sleep, call `tick(now)` straight away. The engine already handles sleeping through a planned window. *Status: with the window hidden (started via `PERSONAL_OEE_HIDDEN=1`) the menu-bar countdown kept ticking. Clicking the red close button to hide, and a real sleep and wake, still need a manual check. The Dock icon stays; Open window in the menu-bar menu brings the window back.*
- [x] D-44 Settings: default start time, default finish time, base target and the shortcut key. Save to `settings.json`. *Status: new Settings tab, seen rendering; changing the shortcut and re-registering it live still needs a manual check. Each day now keeps the settings it started with, so editing the target doesn't rewrite past days.*

## M4 — Faster days and better insight

- [x] D-54 Bulk edit in the shift log. Click or shift-click several blocks (on the Today screen and on past days), then set their category, type, or Rework in one step. Only changes that apply to every selected block are offered; blocks that can't take the change are skipped and the count is shown ("Set 4 of 5 blocks"). Each block is saved as one edit, so single-block editing (D-25 to D-28, D-50) is unchanged. Done when: selecting 6 uncategorised lost blocks and choosing Wait categorises all 6 in one action, and the day's file is saved once. Needs: a `DayRecord` method that applies one change to many blocks with tests first. *Status: built (`bulk.rs`, `DayRecord::apply_bulk`, 6 new tests). Shift-click or Cmd-click picks blocks; once any are picked a plain click picks or unpicks too, and Esc or Clear selection drops the pick. The bar offers Type, Rework (if a Work block is picked) and Category (if a lost block is picked). Blocks that can't take a change are skipped and counted. The selection, the bar and saving once still need a manual check in the running app.*
- [x] D-55 Trends (FR-041). A line chart of weekly OEE, Availability, Efficiency and Quality over the last 4, 12 or 26 weeks, on the Week tab or a new Trends tab. Weeks use the pooled figures (FR-051 to FR-053), so holidays are left out and a week with every day a holiday is skipped, not drawn as 0. Hovering a point shows the week and its values. Done when: a series of saved weeks draws one point per working week, with the over-capacity weeks marked. Needs: a function in `oee-core` that turns saved days into a list of weekly results, with tests. *Status: built. `oee-core::weekly_series` (`trends.rs`, 5 tests) gives one pooled point per working week; the chart is on the Week tab below the loss bars (`trends_screen.rs`, 6 tests), hand-drawn with 4, 12 or 26 week buttons (12 by default), a diamond on over-capacity weeks and a hover tooltip. The y-axis grows past 100% in 25% steps when Efficiency does. Weeks with nothing logged have no point and the line joins across them. How the chart looks and the hover still need a manual check in the running app.*
- [x] D-56 Targets. A weekly OEE goal in Settings (default 85%, can be turned off). The Week screen shows the goal as a line on the trend chart and a status under the week tiles: on track, behind, or met, based on the days so far, plus the number of weeks in a row the goal was met. Done when: setting the goal to 80% with a week pooled at 83% shows "Met", and turning the goal off hides every trace of it. Needs: a new requirement in the PRD (FR-054) for the goal and streak rules, and a settings field that older `settings.json` files load without. *Status: built. FR-054 is now in the PRD, including the rule that an in-progress week within 5 points of the goal is "On track" and a finished week under it is "Not met". Rules are in `oee-core/src/goal.rs` (8 tests). Settings has a Weekly OEE goal tick box and percent (`settings.json` files without it load as 85%, on). The Week tab shows "Goal 85% · Met · 3 weeks in a row" under the tiles and a dashed goal line on the trend chart; with the goal off neither appears. The streak looks back over every saved week. How the status line and the goal line look still need a manual check in the running app.*
- [x] D-57 Mini widget, from design C in the OEE Tracker Desktop Designs canvas (https://claude.ai/artifact/TjYyjyP9hJYX2FtAJeaWXc, board "C · Mini widget + categorise"). *Form chosen: an always-on-top floating widget (option a), built as a second egui window rather than a native WidgetKit widget, because WidgetKit needs Xcode and a signed extension, which this Rust build doesn't have.* *Status: built in `oee-app/src/mini.rs` (6 tests). Open it with the "Mini widget" button in the top bar, or launch with `PERSONAL_OEE_MINI=1`. A borderless, always-on-top window 340 pt wide that resizes its height to fit what it shows, draggable by its title row, with Open (brings the main window forward) and ✕. It shows the status word, the block countdown line, today's OEE, a strip of the day's blocks (current one outlined), and two buttons: the D-17 toggle ("Start downtime" / "Back to work · N lost" / "Extend 20 min") and Rework (flips Rework on the current Work block, or the latest finished Work block during a stop). At Planned Downtime and at the end of the day, with lost blocks waiting, it shows the five categories and gives every waiting block the chosen one. It draws from the same `Tracker` as the main window and works while the main window is hidden. Not done: the note field from the design's categorise prompt (use the block editor), and the widget's position isn't remembered between launches. Not checked by eye: this session couldn't capture the window, so the layout and the always-on-top behaviour need a look in the running app. The global shortcut (FR-011a) is unchanged and still toggles downtime while the widget is open.*
- [x] D-58 The Mac menu bar copies the window's actions. Personal OEE menu: **Settings…** (Cmd+,). New **View** menu: Today (Cmd+1), Week (Cmd+2), Calendar (Cmd+3). Window menu: **Mini widget** (Cmd+Shift+M), ticked while the widget is open. Picking a screen brings the window forward showing it, even after it was closed; Mini widget toggles the widget without touching the main window. *Status: built in `oee-app/src/tray.rs` (menu items, `Actions.show_tab` and `.mini`) and wired in `main.rs`; one test covers the tab flag encoding. The menu items themselves need a look in the running app.*
- [x] D-59 An example week for testing: `examples/week/` is a data folder (days, calendar, settings) generated by `office-oee data seed --scenario example-week`. Mon–Fri 2–6 Nov 2026 with lunch and a Tuesday meeting from the calendar, lost time in four categories with notes, three Rework marks, a Wednesday holiday, a long Thursday and a short Friday; 84.9% OEE for the week. `examples/README.md` says how to open it in the app (`OEE_DATA_DIR`) or with the CLI and how to regenerate it; a test fails if the folder drifts from the scenario or `check` finds a problem. *Status: done.*

## M5 — Command-line tool for agents and tests

Binary `office-oee`, JSON output by default. Full design, command list and reasons: [`CLI_API.md`](CLI_API.md). Writes must name their data: `--data-dir <path>` for a test folder or `--real` for the app's folder; with neither they refuse (exit 7).

- [x] C-01 Extract `oee-store` from `oee-app`: the saved-day store, settings data and validation, start rules, tabs and the export range/rows/file writing. Done when: `cargo test` passes with no behaviour change and `oee-store` has no egui dependency. *Status: done. 141 tests before and after (109 in the app, 32 in `oee-store`). `Shortcut::validate` is in the store; the system-hotkey conversion stays in the app. The week and loss-bar builders stay in `oee-app` until C-08.*
- [x] C-02 Data location: `OEE_DATA_DIR`, `--data-dir` and `--real`. Writes with none of them are refused with exit code 7; both together is a usage error. The app honours `OEE_DATA_DIR` too. Done when: a test with a temp dir never touches `~/Library`. *Status: done in `oee-store::location` (7 tests): `resolve` takes the flag, `OEE_DATA_DIR`, `--real`, `HOME` and whether the command writes, and gives a folder or `data_location_required` / `location_conflict` / `no_home`. The flag beats the variable; `--real` with either is a conflict; an empty variable counts as unset. `Store::default_location` (the app) now honours `OEE_DATA_DIR`. The CLI maps these errors to exit 7 in C-04.*
- [x] C-03 Clock: system clock, `--now` or `OEE_NOW` (`HH:MM` or `YYYY-MM-DDTHH:MM`). Done when: the same command with the same `--now` gives byte-identical JSON. *Status: done in `oee-store::clock` (7 tests): `resolve(flag, env, system)` gives a date and whole minutes. The flag beats `OEE_NOW`, which beats the system clock; a bare time takes the system date; seconds are dropped; bad input gets a plain-words error. The CLI uses it for every command in C-04.*
- [x] C-04 `oee-cli` skeleton: argument parsing, the `{ok, result, notices}` / `{ok:false, error}` envelope, exit codes 0–7, `commands`, `schema`, `version`, `--text`, `--dry-run`. *Status: done. New crate `oee-cli` builds `office-oee` (clap). `run(args, env, clock)` returns stdout and exit code so it is tested without a process (15 tests). Working: `version`, `commands` (commands, global flags, exit codes 0–7), `schema <command>`, `--text`, `--now`/`OEE_NOW`, `--date`, one-line JSON envelope, usage errors as JSON with exit 2. `Context::data_folder(writes)` applies the C-02 rules and gives exit 7 or a conflict (2). `--dry-run` and `--yes` are accepted but not used until write commands arrive (C-05). Each new command must be added to `catalogue.rs`.*
- [x] C-05 Day commands: `day start`, `status`, `toggle`, `downtime start|stop`, `extend`, `end`, `finish`, `show`, `list`, `delete`. Done when: a 25-minute stop returns `{minutes: 25, lost_blocks: 1}` and a stop under 5 minutes is ignored (FR-014). *Status: done (`oee-cli/src/day.rs`, 15 tests in `tests/day.rs`, plus `Tracker::resume_with_notices` in the core and `Store::delete_day`). Every write returns the new day state with `saved`; `--dry-run` saves nothing. A 25-minute stop gives `{minutes: 25, lost_blocks: 1}` and 3 minutes is ignored. Refusals: `no_day` (3), `day_exists`, `not_in_downtime`, `already_in_downtime`, `in_planned_downtime`, `day_ended`, `not_later`, `already_ended`, `not_today`, `future_date` (4), `confirmation_required` (2). A past `--date` is treated as complete, so `day start --date <past>` makes a full Work day to back-fill. `day start` has no `--end`, as the app has none; use `day extend`. Notices from catching up (a planned window started) appear in `notices` on every command while it is running, because the mode is rebuilt from the record each time.*
- [x] C-06 Block commands: `list`, `set-type`, `reset-type`, `categorise`, `rework`, `note`, `bulk` (with the skipped list), `uncategorised`. Done when: each core `EditError` maps to its own error code. *Status: done in `oee-cli/src/block.rs`, 11 tests in `oee-cli/tests/block.rs`. `NoSuchBlock`, `NotLostTime` and `NotWork` give `no_such_block` (3), `not_lost_time` (4) and `not_work` (4). `categorise` and `rework` change all the named blocks or none; `bulk` changes what it can and lists the rest as `skipped` with a code. Blocks are addressed as `10:40`, `10:40,11:00` or `10:40..11:20`. Works on any saved date, so past days can be back-filled. The note flag is `--note` (not `--text`, which is the global human-output flag).*
- [x] C-07 Calendar and settings commands, using the same validation and grid snapping as the app. *Status: done. The add-entry checks moved to `oee-store::calendar` (`DayChoice`, `add_entries`; the Calendar tab uses them too) and the Settings tab's `SettingsForm::parse` does the settings checks. New `calendar list|add|remove` (off-grid entries report `counts_as`) and `settings get|set` (`--start --end --target --goal N|off --shortcut ctrl+alt+D`), 8 tests in `oee-cli/tests/config.rs`, including a started day picking up a calendar break. A refused setting changes nothing (`bad_setting`, exit 4).*
- [x] C-08 Report and export commands: `report day|week|trends|goal|losses` and `export csv`. This item also moves the week and loss-bar builders out of `oee-app` so the Week tab and the CLI share them. Done when: `report week` on the Appendix A data gives 58.4%, equal to the Week tab. *Status: done. `oee-store::weekly` now holds the week/trend/over-capacity builders (`week_start`, `day_inputs`, `report`, `series_from`, `first_monday`, `over_capacity`; 6 tests) and the Week and Trends screens use it, so the CLI and the app share one implementation (app tests unchanged). New `report day|week|trends|goal|losses` and `export csv` (`--out` or `--stdout days|blocks`; never overwrites; `--dry-run` writes nothing) with 11 tests in `oee-cli/tests/report.rs` that compare every figure with the engine. The Appendix A figure (58.4%) can't be built from blocks until the seed command exists, so that check moves to C-09 (`data seed --scenario appendix-a` then `report week` must give 58.4%). The loss-bar widget code (colours, fractions) stays in the app.*
- [x] C-09 `data seed` (scenarios `empty`, `week4`, `holiday-week`, `zero-work-day`, `over-capacity`, `uncategorised-day`, `streak-3`), `data reset` (needs `--yes`; writing needs `--data-dir` or `--real` like every write) and `check`. *Status: done. Seeds live in `oee-store::seed`, the checks in `oee-store::check`; 7 tests in `oee-cli/tests/data.rs`. `data seed` replaces the folder's contents and needs `--yes` if it holds data. `check` reads every file and tests: blocks tile the day, a finalised day has no uncategorised lost block, metrics are in range, the over-capacity flag is set when Efficiency is above 100%, planned time fits the day; any finding gives `check_failed` (exit 5) naming the file. **Deviation:** the exact Appendix A figures (58.4%) cannot be built from blocks, since its planned times (150 min) aren't whole blocks and its points don't tile the day. So `appendix-a` became `week4`, built from whole blocks: it matches Appendix A's pooled 74 Work points of 100, 3 Rework, Quality 98.0% and Efficiency 74.0%, and gives OEE 57.6%. The exact 58.4% stays proved by `oee-core/tests/appendix_a.rs`; C-10 can also reproduce it with `sim run`, which takes minutes directly.*
- [x] C-10 `sim run <scenario.json>`: replay a scripted day with an `expect` block. Done when: the PRD worked examples (91.3% Availability, 92.9% Quality, 59.3% OEE) run as scenario files. *Status: done in `oee-cli/src/sim.rs`. It runs the real commands against a private temporary folder, so no data folder is needed or touched. A step is `{at, do: group.name, flags…}` (groups `day`, `downtime`, `block`; every other key becomes that command's flag); a step without `at` reuses the previous time; `expect_error` names the code a step must fail with. `expect` keys: `oee`, `availability`, `efficiency`, `quality` (with `tolerance`, default 0.0005), and exact `work_points`, `rework_points`, `target_points`, `planned_minutes`, `downtime_minutes`, `day_minutes`, `lost_blocks`, `uncategorised_blocks`, `over_capacity`, `status`. A failed expectation gives `expect_failed` (exit 4) naming each key with both values. The three PRD examples are `oee-cli/scenarios/prd-*.json`; 9 tests in `oee-cli/tests/sim.rs`.*
- [x] C-11 App lock file: the app holds `<data-dir>/.lock` while open; CLI writes then exit with code 6 unless `--force`. *Status: done. `oee-store::lock` (4 tests): the app writes its process id at launch and removes the file on quit (`on_exit`); a lock left by a crash names a dead process and is ignored. The CLI checks it for every write command in `Context::data_folder`, so reads, `sim run` and test folders are never blocked; new global flag `--force`. One CLI test covers refuse, reads, force and a stale lock. Not checked by hand: a real app launch holding and releasing the file (builds and tests pass). **Still open as its own later item (C-13):** the app reloading today's file when it changes on disk, so a `--force` write shows up without a restart.*
- [x] C-13 The app reloads today's day file when it changes on disk and it has no unsaved change of its own (checked in the once-a-second repaint), so CLI writes made with `--force` show up without a restart. From C-11. *Status: done in `App::reload_if_changed_on_disk`, checked once a second; it shows "Today was changed outside the app. Reloaded it." A test covers the day started elsewhere, a block edited elsewhere, and the app's own unsaved change winning. Not checked by hand in the running app.*
- [x] C-12 Add a version field to day, calendar and settings files, loading older files unchanged. *Status: done in `oee-store::store`. Every saved file carries `"version": 1` (`FORMAT_VERSION`); a file without it (everything saved before) loads as before; a higher number is refused with a message saying a newer version saved it, so `check` and the commands report it instead of guessing. `version` now also prints `data_format`. 2 tests.*

Suggested order: C-01 (done), C-02, C-03, C-04, C-05, C-08, C-06, C-07, C-09, C-10, C-11, C-12.

## Optional

- [x] D-45 Practice mode, a debug-only fake clock with +1 / +5 / +20 / +60 min buttons, matching the prototype. Useful for manual testing. Hidden in release builds. *Status: buttons seen in a debug build; the release binary contains none of the practice code. Build a debug bundle with `scripts/bundle.sh debug`. Clicking the buttons still needs a manual check.*
- [x] D-46 Bring the prototype's colours into the egui theme for light and dark mode, using the tokens below. *Status: done for colours, in light and dark (follows the system; `PERSONAL_OEE_THEME=light|dark` forces one). The Barlow and JetBrains Mono fonts are not bundled yet.*
- [x] D-47 Make it obvious how to reopen the window after closing it. Add an "Open Personal OEE" item to the app's own menu in the macOS menu bar (and a Window menu entry), and reopen the window when the Dock icon is clicked. *From manual testing of D-43: the window closed and the app kept running, but the only way back was the small menu-bar item.* *Status: built. The app menu has "Open Personal OEE" (Cmd+O) and Quit (Cmd+Q), the Window menu has "Personal OEE" (Cmd+0), and a Dock click shows the window when it's hidden (`reopen.rs` adds `applicationShouldHandleReopen:hasVisibleWindows:` to winit's delegate). The app launches and tests pass; the menu items and the Dock click still need a manual check.*
- [x] D-48 Always start blocks on the clock's 00/20/40 grid. Starting the day mid-block counts that whole block as started on time, so the block start is never moved to now. *Status: done in `Tracker::start_day`, with tests.*
- [x] D-49 Always show the day as the normal set hours (default start to default finish), not starting at the moment the timer began. Blocks before the first started block default to lost time (uncategorised, so System Down until changed). An earlier start still extends the day back to it, and starting after the finish time gives a one-block day. *Status: done in `Tracker::start_day`, with tests. A late start no longer lowers the target; the missed blocks count as lost instead.*
- [x] D-50 Let any block be changed to a different type after it has started or finished, so a day can be back-filled or corrected: Downtime to Work, Work to Downtime, and either to Planned (Break or Meeting), or a planned block to Work or Lost. The block editor gets a Type picker (Work / Lost time / Planned). A block turned into Planned counts as planned time; a planned block turned into Work or Lost stops counting as planned. Saved in the day's file; older files still load. *Status: done in `DayRecord::set_type` and the block editor, with tests. The picker itself still needs a manual check in the running app.*
- [x] D-51 CSV export in `oee-core` (`export.rs`): `days_csv` gives one row per day (date, status, start, end, minutes, points, Availability, Efficiency, Quality, OEE as 0–1 numbers, over-capacity flag, minutes per loss category), and `blocks_csv` gives one row per block (date, start, end, type, category, rework, note, uncategorised flag). Holidays are in the days file as zero rows. Text is quoted per RFC 4180, and notes starting with `=`, `+`, `-` or `@` get a leading `'` so spreadsheets don't run them. Done when: tests in `oee-core/tests/export.rs` cover quoting, holiday, 0% day, over-capacity, uncategorised blocks and a day still in progress. *Status: done in `oee-core/src/export.rs`, 12 tests.*
- [x] D-52 Export from the app: an "Export CSV…" button with a range (this week, custom dates, all days) that writes both files through a save dialog (`rfd`). Builds the day list from the store (weekdays with no record become holiday rows, weekends only if they have a record; today is scored up to now). Done when: choosing a range writes `oee-days-<from>-<to>.csv` and `oee-blocks-<from>-<to>.csv`. *Status: built (`export_screen.rs`, 8 tests). It uses one folder picker for both files, adds " (2)" rather than overwrite an earlier export, and leaves out dates after today. The Export CSV panel on the Week tab and the folder dialog still need a manual check (D-53).*
- [x] D-53 Manual check: open both files in Numbers and Excel; commas, quotes and line breaks in notes survive, and percentages read as numbers. *Status: checked with a sample export read back through Python's `csv` module: every row has the right number of columns, and quotes, commas, line breaks, accents and Japanese text come back intact. Files now start with a UTF-8 byte-order mark so Excel reads accents. Opening them in Numbers and Excel is still a manual check.*

### Design tokens from the prototype

| Token | Light | Dark | Used for |
|---|---|---|---|
| bg | #eef0ef | #111415 | Window background |
| surface | #ffffff | #1a1e1f | Panels, tiles |
| ink | #15191a | #e6ebe9 | Text, OEE tile |
| muted | #5b6466 | #96a1a2 | Secondary text |
| line | #d3d8d6 | #2e3536 | Borders |
| accent | #d9a400 | #f0c22e | Primary buttons, selected tab |
| work / soft | #1d8048 / #dcefe3 | #46b874 / #183a26 | Work lamp, blocks, pills |
| down / soft | #c2352b / #f7dfdc | #ef5b50 / #4a1f1c | Downtime lamp, lost blocks |
| plan / soft | #44689c / #dfe7f2 | #84a6dc / #1e2b3e | Planned blocks, over-capacity pill |
| change / soft | #c47f00 / #f6ead2 | #f0a62a / #3e2e10 | Change Over, amber lamp, banners |
| rework | #7a4cae | #b08ae6 | Rework dot and bar |

Fonts: Barlow Condensed for headings and the status word, Barlow for body text, JetBrains Mono for times and numbers. All three are on Google Fonts under the SIL Open Font License, so they can be bundled with the app.

## oee-core changes needed (summary)

| Change | For |
|---|---|
| `DayRecord::inputs_until(&Settings, until)` | D-01 |
| Provisional running-downtime event in block building | D-02 |
| `serde` derives, including `edits` | D-10 |
| `DayRecord.finalised` | D-11 |
| `Tracker::resume(record, settings, now)` | D-12 |
| `toggle_downtime` also returns stop length and lost blocks | D-18 |
| `Tracker::end_now(now)` | D-27 |
| Calendar entry ids; remove by id | D-30 |
| Public `snap` | D-32 |
| `Tracker::start_day` snaps to the grid and keeps the default hours | D-48, D-49 |
| `DayRecord::set_type`, `BlockType` | D-50 |
| `weekly_series` in `trends.rs` | D-55 |
| `goal_status`, `goal_streak` in `goal.rs` | D-56 |

Each change needs tests added to `oee-core/tests/` before the app work that uses it.
