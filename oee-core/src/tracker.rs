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
        }
    }

    /// FR-011: the shortcut. Work → Downtime, Downtime → Work.
    /// Ignored during Planned Downtime and after the day has ended.
    pub fn toggle_downtime(&mut self, now: u32) -> Mode {
        self.tick(now);
        self.mode = match self.mode {
            Mode::Work => Mode::Downtime { since: now },
            Mode::Downtime { since } => {
                self.record.add_downtime(since, now);
                Mode::Work
            }
            other => other,
        };
        self.mode
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
        notices
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

    /// FR-027: after the end-of-day reminder, default anything left to System Down.
    pub fn finalise(&mut self) {
        self.record.finalise();
    }

    pub fn inputs(&self) -> DayInputs {
        self.record.inputs(&self.settings)
    }
}
