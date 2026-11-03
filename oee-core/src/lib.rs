//! Personal OEE engine.
//!
//! Pure logic with no I/O, so every rule in the PRD (`docs/PRD.md`) can be unit
//! tested on its own. Requirement IDs (FR-xxx) in the docs refer to that PRD.
//!
//! Times are minutes since midnight (`u32`), e.g. `9 * 60` is 09:00.
//! Use [`rules::hm`] to write them readably.
//!
//! Layers:
//! - [`rules`]: constants, block counting, daily target.
//! - [`category`]: lost-time categories and per-category loss minutes.
//! - [`metrics`]: daily Availability, Efficiency, Quality, OEE.
//! - [`export`]: CSV text for days and blocks.
//! - [`week`]: pooled weekly figures and report helpers.
//! - [`goal`]: the weekly OEE goal's status and streak.
//! - [`trends`]: one pooled result per week, for the trend chart.
//! - [`calendar`]: the repeating weekly Planned Downtime calendar.
//! - [`timeline`]: a day's 20-minute blocks, built from downtime events and edits.
//! - [`tracker`]: the live state machine driven by the shortcut and the clock.

pub mod calendar;
pub mod category;
pub mod export;
pub mod goal;
pub mod metrics;
pub mod rules;
pub mod timeline;
pub mod trends;
pub mod tracker;
pub mod week;

pub use calendar::{CalendarEntry, EntryId, PlannedWindow, Weekday, WeeklyCalendar};
pub use category::{LossMinutes, LostCategory, PlannedKind};
pub use goal::{goal_met, goal_status, goal_streak, GoalStatus};
pub use metrics::{day_result, DayInputs, DayResult, DayStatus, Metrics};
pub use rules::{hm, lost_blocks, Settings, BLOCK_MINUTES};
pub use timeline::{Block, BlockKind, BlockType, BulkChange, BulkOutcome, DayRecord, DowntimeEvent, EditError, TrimError};
pub use tracker::{Mode, Notice, StopSummary, Toggled, Tracker};
pub use trends::{weekly_series, WeekPoint};
pub use week::{over_capacity_days, week_report, WeekReport};
