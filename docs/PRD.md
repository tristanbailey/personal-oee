# Personal OEE Work System — PRD

**Owner:** Tristan
**Status:** Draft — all requirements captured, no open questions
**Last updated:** 2026-10-26

---

## 1. Overview
_What the system is and the problem it solves._

- A Pomodoro-style work tracker that measures personal output in OEE terms.
- Work runs in fixed **20-minute blocks** (the default; the cycle time is a setting, D-61), back to back, across the whole working day.
- Every block is either **Work**, **Lost time** (one of five categories), or **Planned Downtime** (breaks and meetings).
- Work blocks earn **points**. Rework is tracked separately as a Quality loss.

## 2. Goals & Success Measures
_What "working" looks like._

- The aim is **100% Efficiency**: 20 Work points on a default day.
- Current baseline: **12–14 points** a day, around 60–70% Efficiency.

## 3. OEE Model
_How Overall Equipment Effectiveness is applied to personal work._

| Factor | Personal definition | How it's measured |
|---|---|---|
| Availability | Share of planned time not lost to downtime | See FR-044 |
| Performance (Efficiency) | Work points against a daily target | Work points ÷ Target (default 20) (FR-046 to FR-048) |
| Quality | Work not lost to errors | (Work − 0.5 × Rework) ÷ Work (FR-049) |

**OEE = Availability × Efficiency × Quality** (FR-050)

## 4. Users & Context
_Who uses it, when, on what device._

- Single user (Tristan), during the working day.
- Runs as a **desktop app**, used alongside normal work.

## 5. Functional Requirements
_Grouped by area. IDs stay stable so they can map to app features later._

### 5.1 Timer & Day
- **FR-001** — Timer runs in work blocks of one cycle time: 20 minutes by default, changeable in Settings. A change applies from the next day started, and each day keeps the cycle time it started with (D-61). The other rules below say 20 minutes for the default; with a different cycle time they scale (Change Over counts as half a block, and the part-block leeway is a quarter of a block).
- **FR-002** — All blocks are a fixed **20 minutes**.
- **FR-003** — Once the day starts, the timer runs continuously. Each 20-minute cycle rolls straight into the next until the day ends.
- **FR-003a** — There are **no breaks between cycles**.
- **FR-004** — Each day has a start and finish time. Default working day: **09:00–17:40** (520 min = 26 whole blocks).
- **FR-005** — Optional early start: the day can begin before 09:00.
- **FR-006** — Optional extension: extra time can be added after 17:40.
- **FR-006a** — Early-start and extension time is **added to the day's total available time**.

### 5.2 Categories & Downtime
- **FR-007** — Each completed block is tagged with a **work category**.
- **FR-008** — The default category is **Work**. Every other category is a type of **lost time**.
- **FR-009** — Lost time falls into one of five **fixed** categories:

| Lost-time category | Description | OEE factor |
|---|---|---|
| System Down | The computer or system isn't accessible, or work isn't possible. Also the default for uncategorised downtime (FR-026) | Availability |
| Change Over | A slight loss of time from context switching between tasks. Counts as ½ downtime, 10 min (FR-015) | Availability |
| Idle | Doing a non-work task, or taking an unscheduled break | Availability |
| Management | Managing others, or in an unplanned meeting to discuss work rather than do it | Availability |
| Wait | Waiting for other people or a dependency to be ready | Availability |

_All lost time is a time loss (FR-023), so every category reduces Availability through Downtime minutes (FR-044)._

- **FR-024** — The category list is **fixed** and can't be edited by the user.
- **FR-025** — Each block can have free-text **notes**.
- **FR-011** — A **keyboard shortcut** starts downtime. The same or a second shortcut stops downtime and returns to Work.
- **FR-011a** — The shortcut is **global**: it works across the whole computer, even when the app isn't in focus.
- **FR-012** — Downtime blocks are **categorised later**. There is no prompt at the moment downtime starts.
- **FR-013** — Downtime is always recorded in whole 20-minute blocks, even if the start-to-stop period is shorter.
- **FR-014** — **5-minute leeway:** downtime of 5 minutes or less is ignored, and the block counts as Work.
- **FR-015** — **Change Over** blocks count as only **10 minutes of downtime (½ value)**, but still display as a 20-minute block.
- **FR-016** — Downtime that crosses the boundary between two 20-minute cycles counts as **1 lost block**, not 2.
- **FR-017** — Lost blocks are counted from the **duration** of the stop, not from how many cycles it touches. Every full 20 minutes is 1 block, and a remainder over 5 minutes adds 1 more block.

  `lost_blocks = floor(d / 20) + (1 if d mod 20 > 5 else 0)`

| Stop duration | Lost blocks | Source |
|---|---|---|
| ≤ 5 min | 0 (counts as Work) | FR-014 |
| 10 min (crossing a boundary) | 1 | FR-016 |
| 30 min | 2 | Stated |
| 45 min | 2 | Stated |

- **FR-026** — **Categorisation reminders:** the app prompts the user to categorise downtime blocks at each Planned Downtime and at end of day.
- **FR-027** — Any downtime block still uncategorised at end of day **defaults to System Down**.

### 5.3 Scoring
- **FR-010** — A completed Work block = **1 Work point**.
- **FR-018** — **Rework** marks work spent on errors or corrections.
- **FR-019** — Rework is a **manual, optional** mark on a **Work** block. Each Work block can carry at most one.
- **FR-020** — Each mark counts as **1 rework** in tallies and reports.
- **FR-021** — In the Quality calculation, each rework counts as **×0.5, half a loss**.
- **FR-022** — Points are tallied in **separate columns**: a Work block scores **1 Work point**, and a Rework mark scores **1 Rework point** in its own column.
- **FR-023** — Downtime blocks score **no points**, including Change Over. All downtime is purely a **time loss**.

- **FR-049** — **Quality formula (confirmed):**

  `Quality = (Work points − Errors) / Work points`, where `Errors = 0.5 × Rework points` (FR-021)

  - _Example:_ 14 Work points, 2 Rework → (14 − 1) / 14 = **92.9%**
- **FR-050** — **OEE = Availability × Efficiency × Quality**
  - _Example:_ 91.3% × 70% × 92.9% = **59.3%** (calculated from unrounded values)

- **FR-044** — **Availability formula:**

  `Availability = (Day minutes − Planned Downtime minutes − Downtime minutes) / (Day minutes − Planned Downtime minutes)`

  - _Day minutes_ = every minute from day start to finish, including early start and extension (FR-006a).
  - _Planned Downtime minutes_ = breaks and meetings (FR-032).
  - _Downtime minutes_ = lost blocks × 20, with each Change Over block counting as 10 (FR-015).
  - _Example:_ 520-minute day, 60 min planned, 2 lost blocks (40 min) → (520 − 60 − 40) / (520 − 60) = 420 / 460 = **91.3%**
- **FR-045** — **Holiday rule:** a day with **nothing logged at all** (the timer never started) counts as a **holiday**. All values show **0**, the maths is **skipped**, and the day is excluded from weekly figures (FR-052).
- **FR-045a** — **Zero-work day:** a day with time logged but **0 Work points**, such as a full day of System Down, is a **0% working day**, not a holiday. **All values** (Availability, Efficiency, Quality, OEE) show **0**. The day **counts** in weekly figures: its minutes and target are added to the week's totals.
- **FR-046** — **Efficiency** (the Performance factor) uses a daily constant: **target Work points per day**, with a default of **20**.
- **FR-047** — On a day with extra time, from an **early start or a late finish**, the target **rises by 1 point per extra 20 minutes**. For example, starting at 08:40 and finishing at 18:20 (+60 min) gives a target of 23.
- **FR-047a** — On a **shorter** day, from a **late start or an early finish**, the target **drops by 1 point per 20 minutes missed**. For example, 09:20–17:00 (−60 min) gives a target of 17. Part-blocks don't count either way, and gains and losses net off (08:40–17:20 stays at 20).
- **FR-048** — Efficiency **can exceed 100%**. When it does, the day gets an **over-capacity flag**.

  **Formula (confirmed):** `Efficiency = Work points / Target points`

  - 100% means hitting the target, 20 points on a default day.
  - Typical days achieve **12–14 points**, which is **60–70%** Efficiency.

### 5.4 Planned Downtime Calendar
- **FR-030** — A **weekly calendar** sets **Planned Downtime**.
- **FR-031** — Planned Downtime types: **breaks** and **meetings**.
- **FR-032** — Planned Downtime is **removed from available time**. It's planned, so it doesn't count as a loss.
- **FR-033** — During Planned Downtime the timer **keeps running** in 20-minute blocks, and those blocks are **auto-tagged** as Planned Downtime.
- **FR-034** — The weekly calendar **repeats every 7 days**.
- **FR-035** — Calendar entries are **entered by hand** in this version.
- **FR-036** — When a Planned Downtime period starts, the app **auto-switches** to Planned Downtime, ending any unplanned downtime in progress.
- **FR-037** — When Planned Downtime ends, the app **returns to Work**.
- **FR-038** — Planned Downtime is **counted in whole 20-minute blocks**. Any block a planned window touches is planned, and planned minutes are those blocks × 20. For example, on a 09:00 start a 09:30–10:00 meeting covers the 09:20 and 09:40 blocks, so it counts as **40 minutes**. Running downtime ends at the start of the first planned block (FR-036).

### 5.5 Reporting
- **FR-040** — **Daily OEE** report: Availability, Efficiency, Quality and overall OEE for the day.
- **FR-041** — **Weekly trends** report: OEE and its factors over time.
- **FR-042** — **Loss breakdown** report: lost time by category, plus Rework counts.
- **FR-043** — **Over-capacity** report: lists every flagged day (FR-048) so it can be reviewed and corrected, for example by recategorising blocks.
- **FR-051** — **Weekly figures are pooled, not averaged.** Minutes and points from all non-holiday days are added together, then the formulas are applied once:
  - Weekly Availability uses the summed Day, Planned Downtime and Downtime minutes.
  - Weekly Efficiency = summed Work points ÷ summed daily targets.
  - Weekly Quality uses the summed Work and Rework points.
- **FR-052** — **Holidays are excluded** from weekly figures, so the week simply has one day fewer.
- **FR-053** — If **every day** in the week is a holiday, all weekly values are **0**.
- **FR-054** — **Weekly OEE goal.** The user sets a weekly OEE goal in Settings (default **85%**) or turns it off. The week's pooled OEE (FR-051) is compared with it at the one decimal shown, so 79.96% reads as 80.0% and meets an 80% goal.
  - **Met** — pooled OEE is at or above the goal.
  - **On track** — not met yet, the week is still in progress (it contains today), and pooled OEE is within 5 percentage points of the goal.
  - **Behind** — not met yet, the week is in progress, and pooled OEE is more than 5 points under the goal.
  - **Not met** — the week has finished under the goal.
  - A week with no counted days has no status (FR-052, FR-053).
  - **Streak** — the number of weeks in a row that met the goal, counting back from the latest week. Weeks with no counted days are skipped: they neither count nor break the streak. The current week counts when it is Met; while it is not met it is ignored, because it isn't finished. Any finished week that is Not met ends the streak.
  - With the goal off, nothing about it is shown: no status, streak or goal line on the trend chart (FR-041). Settings files saved before this requirement load with the default goal.

## 6. Non-Functional Requirements
_Speed, privacy, offline use, platforms._

- **NFR-001** — Desktop application written in **Rust**.
- **NFR-002** — Must register a **system-wide keyboard shortcut** (FR-011a).
- **NFR-003** — **Versioning.** The version lives in the workspace `Cargo.toml` (shown by `office-oee version`) and in the app bundle (`scripts/bundle.sh`). After each future task or change is completed, **raise the patch number by 0.0.1** (1.0.0 → 1.0.1 → 1.0.2) in the same commit; `scripts/bump-version.sh` does it. A minor or major release (1.1.0, 2.0.0) is decided by the owner and is never made automatically. The saved-file format (`data_format`) has its own number and changes only when the file layout does.

## 7. Data Model (draft)
_Entities and key fields._

- **Day** — date, start time, finish time (default 09:00–17:40), early-start / extension minutes, total available time, target Work points (default 20), holiday flag, over-capacity flag
- **Block** — day, start time, length (20 min), category, Work points, Rework (yes/no, Work blocks only), notes, auto-tagged flag
- **Category** — fixed list. Work · Lost time (System Down, Change Over, Idle, Management, Wait) · Planned Downtime (break, meeting)
- **Downtime Event** — start, stop, duration → resulting lost blocks (FR-017)
- **Planned Downtime** — weekday, start, end, type (break / meeting); repeats weekly

## 8. Out of Scope (this version)
- Syncing with a work calendar (e.g. Outlook / Google). Planned Downtime is entered by hand (FR-035).
- User-defined categories (FR-024).
- Mobile app.

## 9. Open Questions
**Open**
- _None at present._

**Implementation notes (engine v0.1)**
- Lost blocks from a stop are placed on the blocks the stop overlaps most; ties go to the earlier block.
- The downtime shortcut is ignored during Planned Downtime and after the day ends.
- Downtime still running at the finish time is closed at the finish time.
- If the computer sleeps through a planned window, the rules still apply: running downtime ends at the window start, and the app returns to Work afterwards.
- A leftover part-block at the end of a day that isn't a multiple of 20 minutes counts in day minutes but isn't a block.

**Resolved**
- Q33: Planned Downtime counts in whole 20-minute blocks, for now (FR-038).
- Q34: A shorter day drops the target by 1 per 20 minutes missed (FR-047a).
- Q32: Test data uses a 520-min day; "down time" is Planned Downtime; Change Over already halved (Appendix A).
- Q8: Lost-time categories defined; all reduce Availability (FR-009).
- Q25: Change Over counts as 10 minutes of downtime (FR-015).
- Q31: A day with time logged but 0 Work points is a 0% working day. Only a day with nothing logged is a holiday (FR-045, FR-045a).
- Q20: Performance = Efficiency against a daily target of Work points (FR-046).
- Q21: The downtime shortcut is global (FR-011a, NFR-002).
- Q24: Holidays are excluded; weekly figures are pooled from summed minutes and points (FR-051 to FR-053).
- Q28: Early start and late finish both raise the target (FR-047).
- Q29: Over 100% sets an over-capacity flag with its own review report (FR-048, FR-043).
- Q1: Blocks are fixed at 20 minutes (FR-002).
- Q2: No breaks between cycles (FR-003a).
- Q3: Work scores 1, Rework scores 1 in a separate column, downtime scores nothing (FR-022, FR-023).
- Q5: Default day extended to 17:40, giving 26 whole blocks (FR-004).
- Q6: Extra time adds to the day's available time (FR-006a).
- Q7: Breaks and meetings are removed from available time (FR-032).
- Q9: Reminders at Planned Downtime and end of day. Uncategorised blocks default to System Down (FR-026, FR-027).
- Q10: Calendar repeats every 7 days (FR-034).
- Q11: Timer keeps running and auto-tags Planned Downtime blocks (FR-033).
- Q12: Categories are fixed; every block can have notes (FR-024, FR-025).
- Q13 / Q17: Downtime of 5 minutes or less is ignored and counts as Work (FR-014).
- Q14: Downtime crossing a cycle boundary counts as 1 block (FR-016).
- Q15: Change Over scores nothing (FR-023).
- Q16: 30 min = 2 blocks, 45 min = 2 blocks (FR-017).
- Q19: A Rework block scores 1 Work and 1 Rework (FR-022).
- Q22: Planned Downtime auto-switches over any running downtime (FR-036).
- Q23: After Planned Downtime, the app returns to Work (FR-037).
- Q27: Target is 20 by default, plus 1 per 20 minutes of extended time (FR-047).
- Q4 / Q18: All OEE maths defined (FR-044, FR-048, FR-049, FR-050).
- Q30: Superseded by Q31 (FR-045, FR-045a).
- Q26: Efficiency = Work points ÷ Target (FR-048).

## 10. Appendix A — Worked Test Data (Week 4)
_Sample week supplied by Tristan. Results calculated with FR-044, FR-048, FR-049, FR-050 and FR-051. Intended as the first set of unit tests._

**Data notes (confirmed):**
- The "Planned" column is **Planned Downtime** (breaks and meetings). The five category columns add up to unplanned downtime.
- Category values are **counted minutes**, so Change Over is already halved (10 min each, FR-015).
- Day length is **520 min** (09:00–17:40, FR-004), and the target is **20** each day.

**Input**

| Date | Week | Day min | Planned | System Down | Change Over | Idle | Management | Wait | Points | Rework |
|---|---|---|---|---|---|---|---|---|---|---|
| Jan 20 | 4 | 520 | 150 | 0 | 20 | 0 | 80 | 0 | 14 | 0 |
| Jan 21 | 4 | 520 | 110 | 60 | 10 | 0 | 50 | 0 | 11 | 1 |
| Jan 22 | 4 | 520 | 110 | 0 | 30 | 20 | 0 | 20 | 15 | 2 |
| Jan 23 | 4 | 520 | 90 | 0 | 20 | 0 | 0 | 0 | 18 | 0 |
| Jan 24 | 4 | 520 | 90 | 0 | 30 | 40 | 20 | 0 | 16 | 0 |

**Expected output**

| Date | Downtime min | Planned run time | Availability | Efficiency | Quality | OEE |
|---|---|---|---|---|---|---|
| Jan 20 | 100 | 370 | 73.0% | 70.0% | 100.0% | 51.1% |
| Jan 21 | 120 | 410 | 70.7% | 55.0% | 95.5% | 37.1% |
| Jan 22 | 70 | 410 | 82.9% | 75.0% | 93.3% | 58.0% |
| Jan 23 | 20 | 430 | 95.3% | 90.0% | 100.0% | 85.8% |
| Jan 24 | 90 | 430 | 79.1% | 80.0% | 100.0% | 63.3% |
| **Week 4 (pooled)** | **400** | **2050** | **80.5%** | **74.0%** | **98.0%** | **58.4%** |

_Weekly totals: 2,600 day min, 550 planned, 400 downtime, 74 points against a target of 100, 3 rework._

## 11. Changelog
- 2026-10-25 — Skeleton created
- 2026-10-25 — Timer, day window, categories, downtime shortcut, leeway and block-counting rules (FR-001 to FR-017)
- 2026-10-25 — Rework and scoring columns (FR-018 to FR-023)
- 2026-10-25 — Resolved Q2, Q5–Q7 and Q9–Q15. Day now 09:00–17:40. Added notes, reminders, System Down default, Planned Downtime rules (FR-032 to FR-035), reports (FR-040 to FR-042), Rust desktop platform and Out of Scope. Restructured sections.
- 2026-10-25 — Planned Downtime auto-switch (FR-036)
- 2026-10-25 — Return to Work after Planned Downtime (FR-037)
- 2026-10-25 — Availability formula with worked example, and holiday rule (FR-044, FR-045)
- 2026-10-25 — Efficiency daily target constant, default 20 Work points (FR-046)
- 2026-10-25 — Efficiency formula added as stated; interpretation pending (Q26)
- 2026-10-25 — Target scales with extended time; Efficiency can exceed 100% with a warning (FR-047, FR-048)
- 2026-10-25 — Efficiency formula confirmed (Work ÷ Target). Added goals and the 12–14 point baseline.
- 2026-10-25 — Quality formula confirmed; full OEE formula and worked example (FR-049, FR-050)
- 2026-10-25 — Holiday rule now triggers on 0 Work points; all values 0, maths skipped (FR-045)
- 2026-10-25 — Early start raises target too (FR-047); global shortcut (FR-011a, NFR-002); over-capacity flag and report (FR-048, FR-043); pooled weekly figures excluding holidays (FR-051 to FR-053)
- 2026-10-25 — Defined the five lost-time categories; confirmed Change Over at 10 min; split the holiday rule into unlogged days (holiday) and zero-work days (0%) (FR-009, FR-045, FR-045a)
- 2026-10-26 — Zero-work day shows all values as 0 (FR-045a). Confirmed Management covers unplanned meetings (FR-009).
- 2026-10-26 — Added Appendix A: Week 4 worked test data with expected daily and weekly results
- 2026-10-26 — Appendix A recalculated on a 520-min day; data notes confirmed
- 2026-10-26 — Engine v0.1 built and tested. Corrected FR-050 example to 59.3%. Added Q33, Q34 and implementation notes.
- 2026-10-26 — Planned Downtime counted in whole 20-minute blocks (FR-038); engine updated
- 2026-10-26 — Shorter day drops the target by 1 per 20 minutes missed (FR-047a); engine updated
- 2026-11-01 — Weekly OEE goal, status and streak (FR-054)
- 2026-11-01 — Versioning rule: patch version +0.0.1 after each completed change; the owner decides bigger releases (NFR-003)
