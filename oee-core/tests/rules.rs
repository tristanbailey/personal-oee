//! Rules, metrics and weekly pooling.

use oee_core::*;

fn pct(x: f64) -> f64 {
    (x * 1000.0).round() / 10.0
}

#[test]
fn lost_block_counting() {
    // FR-014, FR-016, FR-017 plus boundary cases.
    for (minutes, blocks) in [
        (0, 0),
        (3, 0),
        (5, 0),
        (6, 1),
        (10, 1),
        (20, 1),
        (25, 1),
        (26, 2),
        (30, 2),
        (45, 2),
        (46, 3),
        (60, 3),
    ] {
        assert_eq!(lost_blocks(minutes), blocks, "{minutes} min");
    }
}

#[test]
fn change_over_counts_ten_minutes() {
    assert_eq!(LostCategory::ChangeOver.downtime_minutes(), 10);
    for c in LostCategory::ALL
        .into_iter()
        .filter(|c| *c != LostCategory::ChangeOver)
    {
        assert_eq!(c.downtime_minutes(), 20, "{}", c.label());
    }
}

#[test]
fn daily_target() {
    let s = Settings::default();
    assert_eq!(s.target_points(hm(9, 0), hm(17, 40)), 20);
    // FR-047: late finish only.
    assert_eq!(s.target_points(hm(9, 0), hm(18, 20)), 22);
    // FR-047: early start and late finish, +60 min.
    assert_eq!(s.target_points(hm(8, 40), hm(18, 20)), 23);
    // Partial extra block doesn't count.
    assert_eq!(s.target_points(hm(9, 0), hm(17, 50)), 20);
}

#[test]
fn prd_examples() {
    // FR-044 example: 520-min day, 60 planned, 40 downtime → 91.3%.
    let i = DayInputs {
        day_minutes: 520,
        planned_minutes: 60,
        losses: LossMinutes::new(40, 0, 0, 0, 0),
        work_points: 14,
        rework_points: 2,
        target_points: 20,
    };
    let m = Metrics::from_inputs(&i);
    assert_eq!(pct(m.availability), 91.3);
    // FR-048: 14 / 20.
    assert_eq!(pct(m.efficiency), 70.0);
    // FR-049 example: (14 − 1) / 14 → 92.9%.
    assert_eq!(pct(m.quality), 92.9);
    // FR-050 example → 59.3% (exact; multiplying the rounded figures gives 59.4).
    assert_eq!(pct(m.oee), 59.3);
}

#[test]
fn over_capacity_flag() {
    let i = DayInputs {
        day_minutes: 520,
        planned_minutes: 0,
        work_points: 21,
        target_points: 20,
        ..Default::default()
    };
    let r = day_result(Some(&i));
    assert!(r.metrics.over_capacity);
    assert_eq!(pct(r.metrics.efficiency), 105.0);

    let normal = day_result(Some(&DayInputs {
        work_points: 20,
        ..i
    }));
    assert!(
        !normal.metrics.over_capacity,
        "exactly 100% is not over capacity"
    );

    let days = [("Mon", r), ("Tue", normal)];
    assert_eq!(over_capacity_days(&days), vec![&"Mon"]);
}

#[test]
fn holiday_and_zero_work_days() {
    // FR-045: nothing logged.
    let holiday = day_result(None);
    assert_eq!(holiday.status, DayStatus::Holiday);
    assert_eq!(holiday.metrics, Metrics::ZERO);

    // FR-045a: a full day of System Down is a 0% day, not a holiday.
    let down = DayInputs {
        day_minutes: 520,
        planned_minutes: 60,
        losses: LossMinutes::new(460, 0, 0, 0, 0),
        work_points: 0,
        rework_points: 0,
        target_points: 20,
    };
    let r = day_result(Some(&down));
    assert_eq!(r.status, DayStatus::ZeroWork);
    assert_eq!(r.metrics, Metrics::ZERO);
}

fn normal_day() -> DayInputs {
    DayInputs {
        day_minutes: 520,
        planned_minutes: 60,
        losses: LossMinutes::new(0, 20, 40, 0, 0),
        work_points: 15,
        rework_points: 1,
        target_points: 20,
    }
}

#[test]
fn week_excludes_holidays() {
    // FR-052: one holiday means the week simply has one day fewer.
    let d = normal_day();
    let with_holiday = week_report([Some(&d), Some(&d), None, Some(&d), Some(&d)]);
    let four_days = week_report([Some(&d), Some(&d), Some(&d), Some(&d)]);
    assert_eq!(with_holiday.days_counted, 4);
    assert_eq!(with_holiday, four_days);
}

#[test]
fn week_counts_zero_work_days() {
    // FR-045a: a zero-work day's minutes and target still count in the week.
    let d = normal_day();
    let zero = DayInputs {
        losses: LossMinutes::new(460, 0, 0, 0, 0),
        work_points: 0,
        rework_points: 0,
        ..d
    };
    let w = week_report([Some(&d), Some(&zero)]);
    assert_eq!(w.days_counted, 2);
    assert_eq!(w.totals.target_points, 40);
    assert_eq!(pct(w.metrics.efficiency), 37.5);
}

#[test]
fn week_of_holidays_is_zero() {
    // FR-053.
    let w = week_report([None, None, None, None, None]);
    assert_eq!(w.days_counted, 0);
    assert_eq!(w.metrics, Metrics::ZERO);
}

#[test]
fn weekly_calendar_repeats_by_weekday() {
    let mut cal = WeeklyCalendar::default();
    cal.add(
        Weekday::Mon,
        PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"),
    );
    cal.add(
        Weekday::Mon,
        PlannedWindow::new(hm(9, 0), hm(9, 20), PlannedKind::Meeting, "Stand-up"),
    );
    cal.add(
        Weekday::Tue,
        PlannedWindow::new(hm(12, 0), hm(12, 40), PlannedKind::Break, "Lunch"),
    );
    let mon = cal.windows_for(Weekday::Mon);
    assert_eq!(mon.len(), 2);
    assert_eq!(mon[0].label, "Stand-up", "sorted by start time");
    assert_eq!(cal.windows_for(Weekday::Wed), vec![]);
}
