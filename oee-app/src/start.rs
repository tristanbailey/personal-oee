//! The Not started screen (D-19): pick a start time and begin the day.

use chrono::{Datelike, NaiveDate};
use oee_core::{PlannedKind, PlannedWindow, Weekday, WeeklyCalendar};

pub fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Parse `H:MM` or `HH:MM` into minutes since midnight.
pub fn parse_hhmm(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    if m.len() != 2 || h.is_empty() || h.len() > 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// FR-005: the day can start now or at an earlier time, never in the future.
pub fn validate_start(text: &str, now: u32) -> Result<u32, String> {
    let start = parse_hhmm(text).ok_or("Enter the start time as HH:MM, for example 08:40.")?;
    if start > now {
        return Err("The start time can't be later than now.".into());
    }
    Ok(start)
}

pub fn weekday_of(date: NaiveDate) -> Weekday {
    match date.weekday() {
        chrono::Weekday::Mon => Weekday::Mon,
        chrono::Weekday::Tue => Weekday::Tue,
        chrono::Weekday::Wed => Weekday::Wed,
        chrono::Weekday::Thu => Weekday::Thu,
        chrono::Weekday::Fri => Weekday::Fri,
        chrono::Weekday::Sat => Weekday::Sat,
        chrono::Weekday::Sun => Weekday::Sun,
    }
}

/// FR-030, FR-033: the calendar's windows for `date`'s weekday.
pub fn planned_for(calendar: &WeeklyCalendar, date: NaiveDate) -> Vec<PlannedWindow> {
    calendar.windows_for(weekday_of(date))
}

/// One line per window, or the empty-calendar hint.
pub fn planned_lines(windows: &[PlannedWindow]) -> Vec<String> {
    if windows.is_empty() {
        return vec!["No planned breaks or meetings for today. Add them in Calendar.".into()];
    }
    windows.iter().map(window_line).collect()
}

/// "12:00–13:00 · Break · Lunch". An unnamed window repeats its type.
pub fn window_line(w: &PlannedWindow) -> String {
    let kind = match w.kind {
        PlannedKind::Break => "Break",
        PlannedKind::Meeting => "Meeting",
    };
    let name = if w.label.is_empty() { kind } else { &w.label };
    format!("{}–{} · {kind} · {name}", hhmm(w.start), hhmm(w.end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, Settings, Tracker};

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("08:40"), Some(hm(8, 40)));
        assert_eq!(parse_hhmm(" 9:05 "), Some(hm(9, 5)));
        for bad in ["", "9", "0905", "24:00", "09:60", "09:5", "aa:bb", "-1:00"] {
            assert_eq!(parse_hhmm(bad), None, "{bad}");
        }
    }

    #[test]
    fn start_can_be_earlier_but_not_later_than_now() {
        let now = hm(9, 30);
        assert_eq!(validate_start("08:40", now), Ok(hm(8, 40)));
        assert_eq!(validate_start("09:30", now), Ok(now));
        assert!(validate_start("09:31", now).is_err());
        assert!(validate_start("nonsense", now).is_err());
    }

    #[test]
    fn early_start_gives_a_day_to_17_40_with_target_21() {
        let start = validate_start("08:40", hm(9, 0)).unwrap();
        let t = Tracker::start_day(start, vec![], Settings::default());
        assert_eq!((t.record.start, t.record.end), (hm(8, 40), hm(17, 40)));
        assert_eq!(t.inputs().target_points, 21);
    }

    #[test]
    fn weekdays_map_from_dates() {
        let d = |day| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        assert_eq!(weekday_of(d(26)), Weekday::Mon);
        assert_eq!(weekday_of(d(29)), Weekday::Thu);
        assert_eq!(weekday_of(d(31)), Weekday::Sat);
        assert_eq!(weekday_of(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()), Weekday::Sun);
    }

    #[test]
    fn a_calendar_lunch_switches_the_day_to_break_at_noon() {
        use oee_core::{Mode, PlannedKind};
        let mut cal = WeeklyCalendar::default();
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        cal.add(Weekday::Thu, lunch.clone());
        let thursday = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        let friday = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        assert!(planned_for(&cal, friday).is_empty());

        let planned = planned_for(&cal, thursday);
        assert_eq!(planned, vec![lunch]);
        let mut t = Tracker::start_day(hm(9, 0), planned, Settings::default());
        assert_eq!(t.mode(), Mode::Work);
        t.tick(hm(12, 0));
        assert_eq!(
            t.mode(),
            Mode::Planned {
                kind: PlannedKind::Break,
                until: hm(13, 0)
            }
        );
    }

    #[test]
    fn planned_lines_list_windows_or_the_hint() {
        assert_eq!(
            planned_lines(&[]),
            vec!["No planned breaks or meetings for today. Add them in Calendar."]
        );
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let unnamed = PlannedWindow::new(hm(14, 0), hm(14, 20), PlannedKind::Meeting, "");
        assert_eq!(
            planned_lines(&[lunch, unnamed]),
            vec![
                "12:00–13:00 · Break · Lunch",
                "14:00–14:20 · Meeting · Meeting"
            ]
        );
    }
}
