//! FR-030 to FR-035: weekly Planned Downtime calendar, repeating every 7 days.

use serde::{Deserialize, Serialize};

use crate::category::PlannedKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Identifies one calendar entry. Ids are never reused, so removing by id
/// stays correct however the list is sorted or filtered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntryId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: EntryId,
    pub day: Weekday,
    pub window: PlannedWindow,
}

/// Entries are entered by hand (FR-035) and repeat every week (FR-034).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyCalendar {
    entries: Vec<CalendarEntry>,
    next_id: u32,
}

impl WeeklyCalendar {
    pub fn add(&mut self, day: Weekday, window: PlannedWindow) -> EntryId {
        let id = EntryId(self.next_id);
        self.next_id += 1;
        self.entries.push(CalendarEntry { id, day, window });
        id
    }

    /// Remove an entry by id. `None` if there is no such entry.
    pub fn remove(&mut self, id: EntryId) -> Option<CalendarEntry> {
        let pos = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(pos))
    }

    /// The entries for a weekday, sorted by start time.
    pub fn entries_for(&self, day: Weekday) -> Vec<&CalendarEntry> {
        let mut v: Vec<_> = self.entries.iter().filter(|e| e.day == day).collect();
        v.sort_by_key(|e| (e.window.start, e.id));
        v
    }

    /// The planned windows for a weekday, sorted by start time.
    pub fn windows_for(&self, day: Weekday) -> Vec<PlannedWindow> {
        self.entries_for(day)
            .into_iter()
            .map(|e| e.window.clone())
            .collect()
    }
}
