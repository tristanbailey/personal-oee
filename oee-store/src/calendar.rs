//! Adding weekly break and meeting entries, with the Calendar tab's checks.
//! Shared by the Calendar tab and the command-line tool.

use oee_core::{EntryId, PlannedKind, PlannedWindow, Weekday, WeeklyCalendar};

use crate::start::parse_hhmm;

pub const NAME_LIMIT: usize = 40;

/// Which day(s) a new entry applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayChoice {
    Day(Weekday),
    /// Adds the same entry to Monday to Friday.
    MonToFri,
}

impl DayChoice {
    pub const ALL: [(DayChoice, &'static str); 8] = [
        (DayChoice::MonToFri, "Mon–Fri"),
        (DayChoice::Day(Weekday::Mon), "Monday"),
        (DayChoice::Day(Weekday::Tue), "Tuesday"),
        (DayChoice::Day(Weekday::Wed), "Wednesday"),
        (DayChoice::Day(Weekday::Thu), "Thursday"),
        (DayChoice::Day(Weekday::Fri), "Friday"),
        (DayChoice::Day(Weekday::Sat), "Saturday"),
        (DayChoice::Day(Weekday::Sun), "Sunday"),
    ];

    pub fn label(self) -> &'static str {
        Self::ALL.iter().find(|(c, _)| *c == self).map_or("", |(_, l)| l)
    }

    pub fn days(self) -> Vec<Weekday> {
        match self {
            DayChoice::Day(d) => vec![d],
            DayChoice::MonToFri => {
                vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri]
            }
        }
    }

    /// `mon`…`sun` (or the full name) and `mon-fri`, in any case.
    pub fn parse(text: &str) -> Option<Self> {
        let t = text.trim().to_ascii_lowercase();
        if matches!(t.as_str(), "mon-fri" | "mon–fri" | "weekdays") {
            return Some(DayChoice::MonToFri);
        }
        Self::ALL
            .iter()
            .skip(1)
            .map(|(c, _)| *c)
            .find(|c| {
                let full = c.label().to_ascii_lowercase();
                t == full || t == full[..3]
            })
    }
}

/// Add the entry, one per day chosen, and return the new ids. On any problem
/// nothing is added.
pub fn add_entries(
    calendar: &mut WeeklyCalendar,
    day: DayChoice,
    from: &str,
    to: &str,
    kind: PlannedKind,
    name: &str,
) -> Result<Vec<EntryId>, String> {
    let time = "Enter times as HH:MM, for example 12:00.";
    let start = parse_hhmm(from).ok_or(time)?;
    let end = parse_hhmm(to).ok_or(time)?;
    if end <= start {
        return Err("The end time must be after the start time.".into());
    }
    let name = name.trim();
    if name.chars().count() > NAME_LIMIT {
        return Err(format!("Names can be at most {NAME_LIMIT} characters."));
    }
    Ok(day
        .days()
        .into_iter()
        .map(|d| calendar.add(d, PlannedWindow::new(start, end, kind, name)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_names_parse_short_and_long() {
        assert_eq!(DayChoice::parse("Mon"), Some(DayChoice::Day(Weekday::Mon)));
        assert_eq!(DayChoice::parse("sunday"), Some(DayChoice::Day(Weekday::Sun)));
        assert_eq!(DayChoice::parse("MON-FRI"), Some(DayChoice::MonToFri));
        assert_eq!(DayChoice::parse("funday"), None);
    }

    #[test]
    fn nothing_is_added_on_error() {
        let mut c = WeeklyCalendar::default();
        assert!(add_entries(&mut c, DayChoice::MonToFri, "13:00", "12:00", PlannedKind::Break, "").is_err());
        assert_eq!(c, WeeklyCalendar::default());
        let ids = add_entries(&mut c, DayChoice::MonToFri, "12:00", "13:00", PlannedKind::Break, "Lunch").unwrap();
        assert_eq!(ids.len(), 5);
    }
}
