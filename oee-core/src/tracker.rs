//! The live state machine: driven by the downtime shortcut and a clock tick.
//!
//! The app calls [`Tracker::toggle_downtime`] when the global shortcut fires
//! (FR-011a) and [`Tracker::tick`] regularly (e.g. every 30 seconds). Both are
//! safe if the computer slept: a tick that jumps past a planned window or the
//! end of day still applies the right rules.

use crate::calendar::PlannedWindow;
use crate::category::PlannedKind;
use crate::metrics::DayInputs;
use crate::rules::Settings;
use crate::timeline::DayRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Work,
    /// Unplanned downtime running since this minute.
    Downtime {
        since: u32,
    },
    /// Inside a planned break or meeting, until this minute.
    Planned {
        kind: PlannedKind,
        until: u32,
    },
    /// Past the day's finish time.
    Ended,
}

/// A stop that has just ended (D-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopSummary {
    pub minutes: u32,
    /// FR-013, FR-014: 0 when the stop is short enough to be ignored.
    pub lost_blocks: u32,
}

/// The result of pressing the downtime toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggled {
    pub mode: Mode,
    /// Set only when this press ended a stop.
    pub stopped: Option<StopSummary>,
}

/// Things the app should show the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// FR-036: switched to Planned Downtime.
    PlannedStarted { kind: PlannedKind, label: String },
    /// FR-037: back to Work.
    PlannedEnded,
    /// FR-026: lost blocks waiting for a category.
    CategoriseReminder { blocks: usize },
    /// The day reached its finish time.
    DayEnded,
}

#[derive(Debug, Clone)]
pub struct Tracker {
    pub record: DayRecord,
    pub settings: Settings,
    mode: Mode,
}

impl Tracker {
    /// FR-003 to FR-005: start the day at `now` (which may be early), ending at
    /// the default finish time. Planned windows come from the weekly calendar.
    pub fn start_day(now: u32, planned: Vec<PlannedWindow>, settings: Settings) -> Self {
        let end = settings.default_end.max(now + crate::rules::BLOCK_MINUTES);
        Self {
            record: DayRecord::new(now, end, planned),
            settings,
            mode: Mode::Work,
        }
    }

    /// D-12: rebuild the tracker from a saved record. A stop that was running
    /// when the app closed is picked up with its original start; `tick(now)` then
    /// applies any planned window or end of day passed while the app was closed.
    pub fn resume(record: DayRecord, settings: Settings, now: u32) -> Self {
        let mode = match record.down_since {
            Some(since) => Mode::Downtime { since },
            None => Mode::Work,
        };
        let mut t = Self {
            record,
            settings,
            mode,
        };
        t.tick(now);
        t
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// FR-006: extend (or reopen) the day to a new finish time.
    pub fn extend_to(&mut self, end: u32, now: u32) {
        if end > self.record.end {
            self.record.end = end;
            if self.mode == Mode::Ended && now < end {
                self.mode = Mode::Work;
            }
            self.sync();
        }
    }

    /// FR-011: the shortcut. Work → Downtime, Downtime → Work.
    /// Ignored during Planned Downtime and after the day has ended.
    pub fn toggle_downtime(&mut self, now: u32) -> Mode {
        self.toggle_downtime_report(now).mode
    }

    /// Like [`Tracker::toggle_downtime`], also reporting the stop that ended.
    pub fn toggle_downtime_report(&mut self, now: u32) -> Toggled {
        self.tick(now);
        let mut stopped = None;
        self.mode = match self.mode {
            Mode::Work => Mode::Downtime { since: now },
            Mode::Downtime { since } => {
                self.record.add_downtime(since, now);
                let minutes = now.saturating_sub(since);
                stopped = Some(StopSummary {
                    minutes,
                    lost_blocks: crate::rules::lost_blocks(minutes),
                });
                Mode::Work
            }
            other => other,
        };
        self.sync();
        Toggled {
            mode: self.mode,
            stopped,
        }
    }

    /// Advance the clock. Returns notices for the app to show.
    pub fn tick(&mut self, now: u32) -> Vec<Notice> {
        let mut notices = Vec::new();
        loop {
            match self.mode {
                Mode::Ended => break,
                _ if now >= self.record.end => {
                    notices.extend(self.end_day());
                    break;
                }
                Mode::Planned { until, .. } if now >= until => {
                    // FR-037: back to Work when the planned window ends.
                    self.mode = Mode::Work;
                    notices.push(Notice::PlannedEnded);
                }
                Mode::Planned { .. } => break,
                Mode::Work | Mode::Downtime { .. } => {
                    let from = match self.mode {
                        Mode::Downtime { since } => since,
                        _ => now,
                    };
                    // The first planned window that has started by now.
                    let next = self
                        .record
                        .planned
                        .iter()
                        .find(|w| w.end > from && w.start <= now)
                        .cloned();
                    let Some(w) = next else { break };
                    // FR-036: end any running downtime where the window starts.
                    if let Mode::Downtime { since } = self.mode {
                        self.record.add_downtime(since, w.start.max(since));
                    }
                    self.mode = Mode::Planned {
                        kind: w.kind,
                        until: w.end,
                    };
                    notices.push(Notice::PlannedStarted {
                        kind: w.kind,
                        label: w.label,
                    });
                    notices.extend(self.reminder());
                }
            }
        }
        self.sync();
        notices
    }

    /// Keep the saved record's running-stop marker in step with the mode.
    fn sync(&mut self) {
        self.record.down_since = match self.mode {
            Mode::Downtime { since } => Some(since),
            _ => None,
        };
    }

    /// FR-026: remind if anything needs categorising.
    fn reminder(&self) -> Option<Notice> {
        let blocks = self.record.uncategorised().len();
        (blocks > 0).then_some(Notice::CategoriseReminder { blocks })
    }

    fn end_day(&mut self) -> Vec<Notice> {
        if let Mode::Downtime { since } = self.mode {
            self.record.add_downtime(since, self.record.end);
        }
        self.mode = Mode::Ended;
        let mut notices = vec![Notice::DayEnded];
        notices.extend(self.reminder());
        notices
    }

    /// D-27: end the day at `now`, which lowers the target (FR-047a). A running
    /// stop is closed there. The day keeps at least one block. Does nothing if
    /// the day has already ended.
    pub fn end_now(&mut self, now: u32) -> Vec<Notice> {
        let mut notices = self.tick(now);
        if self.mode == Mode::Ended {
            return notices;
        }
        self.record.end = now
            .max(self.record.start + crate::rules::BLOCK_MINUTES)
            .min(self.record.end);
        notices.extend(self.end_day());
        self.sync();
        notices
    }

    /// FR-027: after the end-of-day reminder, default anything left to System Down.
    pub fn finalise(&mut self) {
        self.record.finalise();
    }

    pub fn inputs(&self) -> DayInputs {
        self.record.inputs(&self.settings)
    }

    /// The record as it stands at `now`: a running stop is added as a
    /// provisional event `(since, now)`, so it shows in the log and metrics
    /// before it is toggled off (FR-013, FR-017). The stored record is unchanged.
    pub fn live_record(&self, now: u32) -> DayRecord {
        let mut record = self.record.clone();
        if let Mode::Downtime { since } = self.mode {
            record.add_downtime(since, now);
        }
        record
    }

    /// Totals so far: only time and blocks that have finished by `now` count,
    /// including a stop that is still running.
    pub fn inputs_at(&self, now: u32) -> DayInputs {
        self.live_record(now).inputs_until(&self.settings, now)
    }
}
