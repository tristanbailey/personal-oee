//! Known datasets for tests and demos: `data seed --scenario <name>`.
//!
//! Every scenario fills the Monday-to-Friday week containing a chosen date
//! (and earlier weeks where it says so) with days built from whole blocks.

use chrono::{Duration, NaiveDate};
use oee_core::{hm, DayRecord, LostCategory, PlannedKind, PlannedWindow, Weekday, WeeklyCalendar};
use oee_core::LostCategory::{ChangeOver, Idle, Management, SystemDown, Wait};

use crate::weekly::week_start;

pub const SCENARIOS: &[(&str, &str)] = &[
    ("empty", "No saved days at all"),
    ("week4", "PRD Appendix A, Week 4, rebuilt from whole blocks (not the exact 58.4%; see docs)"),
    ("holiday-week", "Wednesday has no record (a holiday); the other weekdays are normal days"),
    ("zero-work-day", "Monday has time logged but no Work (a 0% day); Tuesday is normal"),
    ("over-capacity", "Monday is a 12-hour day of all Work (120% Efficiency); Tuesday is normal"),
    ("uncategorised-day", "Monday is unfinished with three lost blocks waiting for a category"),
    ("example-week", "A realistic week: lunch and a meeting from the calendar, lost time with notes, Rework, a holiday, a long day and a short one"),
    ("streak-3", "Three weeks in a row over the default 85% goal, the last being the chosen week"),
];

/// The saved days a scenario creates, and the weekly calendar if it needs one.
/// Settings stay at their defaults.
pub struct Seed {
    pub days: Vec<(NaiveDate, DayRecord)>,
    pub calendar: WeeklyCalendar,
}

pub fn build(scenario: &str, date: NaiveDate) -> Option<Seed> {
    let monday = week_start(date);
    let at = |i: i64| monday + Duration::days(i);
    let mut days = Vec::new();
    let mut calendar = WeeklyCalendar::default();
    match scenario {
        "empty" => {}
        "week4" => {
            let rows: [(u32, &[(LostCategory, u32)], u32); 5] = [
                (8, &[(ChangeOver, 2), (Management, 4)], 0),
                (6, &[(SystemDown, 3), (ChangeOver, 1), (Management, 3)], 1),
                (6, &[(ChangeOver, 3), (Idle, 1), (Wait, 1)], 2),
                (5, &[(ChangeOver, 2)], 0),
                (5, &[(ChangeOver, 3), (Idle, 2), (Management, 1)], 0),
            ];
            for (i, (planned, lost, rework)) in rows.iter().enumerate() {
                days.push((at(i as i64), day(hm(9, 0), hm(17, 40), *planned, lost, *rework, true)));
            }
        }
        "holiday-week" => {
            for i in [0, 1, 3, 4] {
                days.push((at(i), normal()));
            }
        }
        "zero-work-day" => {
            days.push((at(0), day(hm(9, 0), hm(17, 40), 0, &[(SystemDown, 26)], 0, true)));
            days.push((at(1), normal()));
        }
        "over-capacity" => {
            days.push((at(0), day(hm(8, 0), hm(20, 0), 0, &[], 0, true)));
            days.push((at(1), normal()));
        }
        "uncategorised-day" => {
            let mut d = day(hm(9, 0), hm(17, 40), 0, &[], 0, false);
            for start in [hm(10, 0), hm(10, 20), hm(10, 40)] {
                d.set_type(start, Some(oee_core::BlockType::Lost)).expect("block exists");
            }
            days.push((at(0), d));
        }
        "example-week" => {
            let lunch = || PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
            let demo = || PlannedWindow::new(hm(14, 0), hm(14, 40), PlannedKind::Meeting, "Team demo");
            for d in [Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri] {
                calendar.add(d, lunch());
            }
            calendar.add(Weekday::Tue, demo());
            days.push((at(0), example_day(hm(9, 0), hm(17, 40), vec![lunch()], &[
                Edit::Lost(hm(10, 0), Wait, "Waiting on code review"),
                Edit::Rework(hm(11, 0), "Redo after review comments"),
                Edit::Lost(hm(14, 20), ChangeOver, "Switch to the release branch"),
                Edit::Lost(hm(15, 0), Management, "Planning ran over"),
                Edit::Lost(hm(15, 20), Management, ""),
            ])));
            days.push((at(1), example_day(hm(9, 0), hm(17, 40), vec![lunch(), demo()], &[
                Edit::Lost(hm(9, 40), SystemDown, "VPN down"),
                Edit::Lost(hm(10, 0), SystemDown, ""),
                Edit::Rework(hm(11, 20), ""),
                Edit::Rework(hm(11, 40), "Fixing the export"),
                Edit::Lost(hm(16, 20), Idle, ""),
            ])));
            // Wednesday is a holiday: no record.
            days.push((at(3), example_day(hm(9, 0), hm(19, 0), vec![lunch()], &[
                Edit::Lost(hm(10, 20), ChangeOver, ""),
                Edit::Rework(hm(15, 0), "Late fix before the release"),
                Edit::Lost(hm(17, 0), Wait, "Build queue"),
            ])));
            days.push((at(4), example_day(hm(9, 0), hm(15, 0), vec![lunch()], &[
                Edit::Lost(hm(9, 20), Idle, ""),
                Edit::Lost(hm(13, 20), Management, "1:1"),
            ])));
        }
        "streak-3" => {
            for week in [-14, -7, 0] {
                for i in 0..5 {
                    days.push((at(week + i), normal()));
                }
            }
        }
        _ => return None,
    }
    Some(Seed { days, calendar })
}

/// One hand-placed change to a day.
enum Edit {
    /// A lost block at a start time, with its category and a note ("" for none).
    Lost(u32, LostCategory, &'static str),
    /// A Rework block at a start time, with a note ("" for none).
    Rework(u32, &'static str),
}

/// A finished day with the given planned windows and hand-placed edits.
fn example_day(start: u32, end: u32, windows: Vec<PlannedWindow>, edits: &[Edit]) -> DayRecord {
    let mut record = DayRecord::new(start, end, windows);
    for edit in edits {
        let (at, note) = match edit {
            Edit::Lost(at, category, note) => {
                record.set_type(*at, Some(oee_core::BlockType::Lost)).expect("block exists");
                record.categorise(*at, *category).expect("lost block");
                (*at, *note)
            }
            Edit::Rework(at, note) => {
                record.set_rework(*at, true).expect("work block");
                (*at, *note)
            }
        };
        if !note.is_empty() {
            record.set_note(at, Some(note.to_string())).expect("block exists");
        }
    }
    record.finalise();
    record
}

/// A day that meets the default goal: three planned blocks, three lost to
/// Idle, twenty of Work.
fn normal() -> DayRecord {
    day(hm(9, 0), hm(17, 40), 3, &[(Idle, 3)], 0, true)
}

/// A day of `planned` break blocks around midday, then the lost blocks and
/// `rework` Rework blocks from the start of the day, and Work in the rest.
fn day(start: u32, end: u32, planned: u32, lost: &[(LostCategory, u32)], rework: u32, finalised: bool) -> DayRecord {
    let lunch = start + 9 * 20;
    let windows = if planned > 0 {
        vec![PlannedWindow::new(lunch, lunch + planned * 20, PlannedKind::Break, "Lunch")]
    } else {
        vec![]
    };
    let mut record = DayRecord::new(start, end, windows);
    let free: Vec<u32> = record
        .blocks()
        .iter()
        .filter(|b| !matches!(b.kind, oee_core::BlockKind::Planned(_)))
        .map(|b| b.start)
        .collect();
    let mut next = free.iter().copied();
    for (category, count) in lost {
        for _ in 0..*count {
            let s = next.next().expect("enough blocks");
            record.set_type(s, Some(oee_core::BlockType::Lost)).expect("block exists");
            record.categorise(s, *category).expect("lost block");
        }
    }
    for _ in 0..rework {
        let s = next.next().expect("enough blocks");
        record.set_rework(s, true).expect("work block");
    }
    if finalised {
        record.finalise();
    }
    record
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{day_result, week_report, DayStatus, Settings};

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    #[test]
    fn every_listed_scenario_builds() {
        for (name, _) in SCENARIOS {
            assert!(build(name, d(4)).is_some(), "{name}");
        }
        assert!(build("nope", d(4)).is_none());
    }

    #[test]
    fn a_normal_day_is_86_9_percent_available_and_100_percent_efficient() {
        let s = Settings::default();
        let r = day_result(Some(&normal().inputs(&s)));
        assert_eq!(r.status, DayStatus::Worked);
        assert!((r.metrics.availability - 400.0 / 460.0).abs() < 1e-9);
        assert_eq!(r.metrics.efficiency, 1.0);
    }

    #[test]
    fn week4_has_five_days_in_the_chosen_week() {
        let seed = build("week4", d(4)).unwrap();
        let dates: Vec<_> = seed.days.iter().map(|(date, _)| *date).collect();
        assert_eq!(dates, vec![d(2), d(3), d(4), d(5), d(6)]);
        let s = Settings::default();
        let inputs: Vec<_> = seed.days.iter().map(|(_, r)| r.inputs(&s)).collect();
        let w = week_report(inputs.iter().map(Some));
        assert_eq!(w.days_counted, 5);
    }

    #[test]
    fn the_example_week_is_varied_and_healthy() {
        let seed = build("example-week", d(4)).unwrap();
        let dates: Vec<_> = seed.days.iter().map(|(date, _)| *date).collect();
        assert_eq!(dates, vec![d(2), d(3), d(5), d(6)], "Wednesday is a holiday");
        assert_eq!(seed.calendar.windows_for(Weekday::Tue).len(), 2);
        let s = Settings::default();
        let inputs: Vec<_> = seed.days.iter().map(|(_, r)| r.inputs(&s)).collect();
        assert!(inputs[0].rework_points == 1 && inputs[1].rework_points == 2);
        assert!(inputs[2].day_minutes > 520 && inputs[3].day_minutes < 520);
        assert!(inputs.iter().all(|i| i.downtime_minutes() > 0 && i.planned_minutes > 0));
        let note = seed.days[0].1.blocks().iter().find(|b| b.start == hm(10, 0)).unwrap().note.clone();
        assert_eq!(note.as_deref(), Some("Waiting on code review"));
    }

    #[test]
    fn the_holiday_week_skips_wednesday() {
        let seed = build("holiday-week", d(4)).unwrap();
        assert_eq!(seed.days.len(), 4);
        assert!(!seed.days.iter().any(|(date, _)| *date == d(4)));
    }
}
