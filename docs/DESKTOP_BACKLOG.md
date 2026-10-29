# Desktop App Backlog — from the Web Prototype

**Source:** the web prototype (`web/index.html`, published as the private "Personal OEE" page) and `docs/PRD.md`.
**Target:** `oee-app` (Rust, eframe/egui 0.29) on top of `oee-core`.
**Last updated:** 2026-10-29

Each item has an ID (D-xx), the PRD requirements it covers, and a "Done when" check. Items are grouped into milestones in build order. M0 fixes bugs in what exists today. M1 makes the app usable day to day. M2 brings in the week and calendar screens. M3 adds the Mac-only features the prototype had to leave out.

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
- [ ] D-04 Load today's planned windows from the weekly calendar at Start day. The app currently passes `Vec::new()` (FR-030, FR-033). Done when: with a 12:00–13:00 lunch in the calendar, the app switches to Break at 12:00. Depends on D-30.

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
- [ ] D-17 Add a full-width toggle button: "Start downtime" in red during Work, "Back to work" in green during Downtime, a disabled "Planned downtime" during Planned, and "Extend 20 min" once the day has ended (FR-011, FR-006).
- [ ] D-18 Show a short message after a stop ends, using the result of the toggle. Done when: a 3-minute stop shows "3 min stop ignored. It counts as Work.", and a 25-minute stop shows "25 min stop logged as 1 lost block. Categorise it later." Needs `toggle_downtime` to also return the stop's length and lost-block count.

### Start screen (data entry)

- [ ] D-19 Not started screen: a start-time field prefilled with the current time and editable to an earlier time (FR-005), a line listing today's planned windows or "No planned breaks or meetings for today. Add them in Calendar.", and a Start day button. Done when: entering 08:40 gives a day of 08:40–17:40 with a target of 21 (FR-047).

### Metrics tiles

- [ ] D-20 Replace the sidebar with four tiles: Availability ("40 min lost of 460"), Efficiency ("14 of 20 points"), Quality ("2 rework"), and OEE in an inverted dark tile, captioned "So far today" while running and "A × E × Q" once ended (FR-040).
- [ ] D-21 Show an Over capacity banner when Efficiency is above 100% (FR-048). Show a 0% day banner when the day has ended with time logged but no Work points (FR-045a).

### Shift log

- [ ] D-22 Replace the block list with an hour grid: hour label on the left, three 20-minute blocks per row, each showing its start time and label. Colours: Work green, Lost red, Change Over amber, Planned blue. Uncategorised lost blocks are striped red with a dashed outline. Future blocks are dashed outlines. The current block has a dark outline. Purple dot for Rework, grey dot for a note. Done when: a 520-minute day shows 9 rows ending at 17:20.
- [ ] D-23 Add a legend row above the grid, and a header with the day range and block count, e.g. "09:00–17:40 · 26 blocks". Add "+ 10 min" when the day length isn't a whole number of blocks.
- [ ] D-24 Block labels: "Work", "Rework", the category name, "Categorise" for uncategorised, the planned window's name such as "Lunch", "Downtime…" for the current block during a stop, and "In progress" for the current block during Work.

### Block editor (data entry)

- [ ] D-25 Clicking a finished, planned or current block opens a modal editor titled "Lost time", "Work" or "Planned downtime", with the block's time range. Esc or Cancel closes it without saving.
  - Lost: five buttons, one per category, each showing its name and description, with the selected one highlighted (FR-012). Change Over is highlighted amber.
  - Work: a "Mark as Rework (errors or corrections)" tick box, with the note "Rework still scores 1 Work point. It counts as half a loss in Quality." (FR-019, FR-022).
  - Planned: read-only text naming the planned window.
  - All types: a Note box (optional, max 300 characters) and Save / Cancel buttons (FR-025).
- [ ] D-26 Saving calls `categorise`, `set_rework` and `set_note`. A blank note clears it. Any `EditError` is shown in plain words and nothing is saved. Then save the day (D-13).

### End of day

- [ ] D-27 Add a row of controls under the grid: "Extend 20 min" always, and "End day now" until the day has ended. Ending now sets the finish time to the current time, which lowers the target (FR-047a). Needs `Tracker::end_now(now)` in oee-core. Done when: ending at 17:00 on a 09:00 day gives a target of 18.
- [ ] D-28 Categorisation banners (FR-026):
  - During Planned Downtime: "N lost blocks need a category. Click the striped blocks below."
  - At end of day: "N lost blocks need a category. Anything left becomes System Down when you finish.", with a Finish day button that calls `finalise()` (FR-027).
  - If everything is already categorised: "Day ended. Everything is categorised." with Finish day.

## M2 — Week, Calendar and past days

- [ ] D-29 Add three tabs across the top: Today, Week, Calendar. Remember the last tab between launches.

### Calendar screen (data entry)

- [ ] D-30 Save the weekly calendar to `calendar.json`, and give each entry an id so it can be removed reliably. `WeeklyCalendar::remove` currently works by index (FR-030, FR-034, FR-035).
- [ ] D-31 List Mon–Sun as cards. Each entry shows "12:00–13:00 · Break · Lunch" with a Remove button. Empty days show "Nothing planned". Add a header note: "Repeats every week. Applies to days you start after a change."
- [ ] D-32 For entries that aren't on the 20-minute grid from 09:00, show how they'll count, e.g. "Counts as 09:20–10:00 (40 min) on a 09:00 start" (FR-038). Needs the private `snap` function in `timeline.rs` made public.
- [ ] D-33 Add-entry form with these fields: Day (Mon–Sun, or "Mon–Fri" to add five entries), From, To, Type (Meeting or Break), Name (optional, max 40 characters), and an Add button. Done when: an end time not after the start shows "The end time must be after the start time." and nothing is added.

### Week screen

- [ ] D-34 Add week navigation: ‹ and › buttons around a title like "19 Oct – 25 Oct", with a sub-line "Pooled from N working days, holidays left out" or "No working days this week" (FR-051, FR-052).
- [ ] D-35 Show four pooled tiles for the week using `week_report`, with the OEE tile captioned "Week, pooled" (FR-041, FR-053).
- [ ] D-36 Add a day table with columns Day, Status, Avail., Effic., Quality, OEE and Points ("18/20"), plus a bold Week total row. Status pills: Worked (green), Over capacity (blue), 0% day (red), Holiday (grey). Show Mon–Fri always, and Sat and Sun only when they have a record. Clicking a row opens that day (D-39).
- [ ] D-37 Add a loss breakdown: one horizontal bar per category (Change Over in amber) scaled to the largest, with minutes on the right, plus a Rework bar showing the block count (FR-042).
- [ ] D-38 Add an over-capacity review list of every flagged day across all saved records, newest first, with columns Day, Points, Efficiency and "Review →". Clicking opens that day. If there are none, show "No over-capacity days yet." (FR-043).

### Past day view

- [ ] D-39 Open any past date in the Today layout without the andon panel: a "← Back to today" button, a title like "Tue 20 Oct", a status pill, the tiles, and the shift log with the same block editor. If uncategorised blocks are left, show a banner with Finish day. Edits save to that day's file. A date with no record shows "Nothing logged on Tue 20 Oct. It counts as a holiday."

## M3 — Mac-only features (left out of the prototype)

- [ ] D-40 Register a system-wide shortcut that toggles downtime even when the app isn't the active window, using the `global-hotkey` crate. Default to ⌃⌥D, changeable in settings. Inside the window, **D** also works (FR-011a, NFR-002). Done when: the shortcut toggles downtime with Safari in front.
- [ ] D-41 Add a menu-bar item using the `tray-icon` crate, showing a coloured dot for the mode and the time left in the block, e.g. "● 12m". Its menu has Start/stop downtime, Open window and Quit.
- [ ] D-42 Send macOS notifications for each `Notice`: planned downtime started or ended, the categorisation reminder, and day ended (FR-026). Clicking a notification opens the window.
- [ ] D-43 Keep the timer running with the window closed, so the app lives in the menu bar and doesn't quit when the window closes. After the Mac wakes from sleep, call `tick(now)` straight away. The engine already handles sleeping through a planned window.
- [ ] D-44 Settings: default start time, default finish time, base target and the shortcut key. Save to `settings.json`.

## Optional

- [ ] D-45 Practice mode, a debug-only fake clock with +1 / +5 / +20 / +60 min buttons, matching the prototype. Useful for manual testing. Hidden in release builds.
- [ ] D-46 Bring the prototype's colours into the egui theme for light and dark mode, using the tokens below.

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

Each change needs tests added to `oee-core/tests/` before the app work that uses it.
