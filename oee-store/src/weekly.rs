//! Weekly figures built from saved days (FR-051 to FR-053, FR-043): shared by
//! the Week tab and the command-line tool so both give the same numbers.

use chrono::{Datelike, Duration, NaiveDate};
use oee_core::{
    day_result, over_capacity_days, week_report, weekly_series, DayInputs, DayRecord, DayResult,
    Settings, WeekPoint, WeekReport,
};

/// The Monday of the week containing `date`.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
}

pub fn prev_week(monday: NaiveDate) -> NaiveDate {
    monday - Duration::days(7)
}

pub fn next_week(monday: NaiveDate) -> NaiveDate {
    monday + Duration::days(7)
}

/// A day with a saved record is a working day.
pub fn working_days(days: &[(NaiveDate, Option<DayRecord>)]) -> usize {
    days.iter().filter(|(_, r)| r.is_some()).count()
}

/// One day's totals. Today's unfinished day counts only what has happened by
/// `now`, including a stop that is still running; other days count in full.
pub fn day_inputs(record: &DayRecord, is_today: bool, now: u32, settings: &Settings) -> DayInputs {
    if !is_today {
        return record.inputs(settings);
    }
    let mut live = record.clone();
    if let Some(since) = live.down_since {
        live.add_downtime(since, now);
    }
    live.inputs_until(settings, now)
}

/// FR-051 to FR-053: the pooled report for the week's saved days.
pub fn report(
    days: &[(NaiveDate, Option<DayRecord>)],
    today: NaiveDate,
    now: u32,
    settings: &Settings,
) -> WeekReport {
    let inputs: Vec<Option<DayInputs>> = days
        .iter()
        .map(|(date, r)| r.as_ref().map(|r| day_inputs(r, *date == today, now, settings)))
        .collect();
    week_report(inputs.iter().map(|i| i.as_ref()))
}

/// Every working week from `first` (a Monday) up to today, oldest first.
pub fn series_from(
    all_days: &[(NaiveDate, DayRecord)],
    first: NaiveDate,
    today: NaiveDate,
    now: u32,
    settings: &Settings,
) -> Vec<WeekPoint<NaiveDate>> {
    weekly_series(
        all_days
            .iter()
            .filter(|(date, _)| week_start(*date) >= first && *date <= today)
            .map(|(date, r)| (week_start(*date), Some(day_inputs(r, *date == today, now, settings)))),
    )
}

/// The Monday of the oldest week in a chart of `weeks` weeks ending at `current_monday`.
pub fn first_monday(current_monday: NaiveDate, weeks: u32) -> NaiveDate {
    current_monday - Duration::days(7 * i64::from(weeks.saturating_sub(1)))
}

/// A saved day scored for review.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredDay {
    pub date: NaiveDate,
    pub inputs: DayInputs,
    pub result: DayResult,
}

/// FR-043: every saved day with Efficiency over 100%, newest first.
pub fn over_capacity(
    all_days: &[(NaiveDate, DayRecord)],
    today: NaiveDate,
    now: u32,
    settings: &Settings,
) -> Vec<ScoredDay> {
    let scored: Vec<ScoredDay> = all_days
        .iter()
        .map(|(date, r)| {
            let inputs = day_inputs(r, *date == today, now, settings);
            ScoredDay { date: *date, inputs, result: day_result(Some(&inputs)) }
        })
        .collect();
    let pairs: Vec<(usize, DayResult)> = scored.iter().enumerate().map(|(i, d)| (i, d.result)).collect();
    let mut rows: Vec<ScoredDay> = over_capacity_days(&pairs)
        .into_iter()
        .map(|&i| scored[i].clone())
        .collect();
    rows.sort_by(|a, b| b.date.cmp(&a.date));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn rec() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    #[test]
    fn weeks_start_on_monday() {
        assert_eq!(week_start(d(29)), d(26));
        assert_eq!(week_start(d(26)), d(26));
        assert_eq!(week_start(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()), d(26));
        assert_eq!(prev_week(d(26)), d(19));
        assert_eq!(next_week(d(26)), NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    }

    #[test]
    fn a_week_pools_only_saved_days() {
        let days = vec![(d(26), Some(rec())), (d(27), None), (d(28), Some(rec()))];
        assert_eq!(working_days(&days), 2);
        let r = report(&days, d(30), hm(12, 0), &Settings::default());
        assert_eq!(r.days_counted, 2);
        assert_eq!(r.totals.target_points, 40);
    }

    #[test]
    fn today_counts_only_up_to_now() {
        let settings = Settings::default();
        let r = rec();
        let live = day_inputs(&r, true, hm(10, 0), &settings);
        let full = day_inputs(&r, false, hm(10, 0), &settings);
        assert_eq!(live.work_points, 3);
        assert_eq!(full.work_points, 26);
    }

    #[test]
    fn the_first_monday_of_a_chart() {
        assert_eq!(first_monday(d(26), 1), d(26));
        assert_eq!(first_monday(d(26), 4), NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
    }

    #[test]
    fn one_point_per_working_week() {
        let all = vec![(d(19), rec()), (d(20), rec()), (d(26), rec())];
        let s = series_from(&all, d(19), d(30), hm(12, 0), &Settings::default());
        assert_eq!(s.iter().map(|p| (p.week, p.days_counted)).collect::<Vec<_>>(), vec![(d(19), 2), (d(26), 1)]);
        let s = series_from(&all, d(26), d(30), hm(12, 0), &Settings::default());
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn over_capacity_days_are_listed_newest_first() {
        // 08:00–20:00 is 36 blocks against a target of 30: all Work is 120%.
        let long = DayRecord::new(hm(8, 0), hm(20, 0), vec![]);
        // A day with six hours lost is well under its target.
        let mut slow = rec();
        slow.add_downtime(hm(9, 0), hm(15, 0));
        let all = vec![(d(20), long.clone()), (d(21), slow), (d(22), long)];
        let rows = over_capacity(&all, d(30), hm(12, 0), &Settings::default());
        assert_eq!(rows.iter().map(|r| r.date).collect::<Vec<_>>(), vec![d(22), d(20)]);
        assert_eq!((rows[0].inputs.work_points, rows[0].inputs.target_points), (36, 30));
    }
}
