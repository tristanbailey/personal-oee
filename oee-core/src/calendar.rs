//! FR-030 to FR-035: weekly Planned Downtime calendar, repeating every 7 days.

use crate::category::PlannedKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

/// A planned break or meeting, `start..end` in minutes since midnight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWindow {
    pub start: u32,
    pub end: u32,
    pub kind: PlannedKind,
    pub label: String,
}

impl PlannedWindow {
    pub fn new(start: u32, end: u32, kind: PlannedKind, label: impl Into<String>) -> Self {
        assert!(end > start, "planned window must end after it starts");
        Self {
            start,
            end,
            kind,
            label: label.into(),
        }
    }

    pub fn contains(&self, minute: u32) -> bool {
        (self.start..self.end).contains(&minute)
    }
}

/// Entries are entered by hand (FR-035) and repeat every week (FR-034).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WeeklyCalendar {
    entries: Vec<(Weekday, PlannedWindow)>,
}

impl WeeklyCalendar {
    pub fn add(&mut self, day: Weekday, window: PlannedWindow) {
        self.entries.push((day, window));
    }

    pub fn remove(&mut self, day: Weekday, index: usize) -> Option<PlannedWindow> {
        let pos = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, (d, _))| *d == day)
            .nth(index)
            .map(|(i, _)| i)?;
        Some(self.entries.remove(pos).1)
    }

    /// The planned windows for a weekday, sorted by start time.
    pub fn windows_for(&self, day: Weekday) -> Vec<PlannedWindow> {
        let mut w: Vec<_> = self
            .entries
            .iter()
            .filter(|(d, _)| *d == day)
            .map(|(_, w)| w.clone())
            .collect();
        w.sort_by_key(|w| w.start);
        w
    }
}
