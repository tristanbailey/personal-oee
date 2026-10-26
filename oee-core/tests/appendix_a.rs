//! PRD Appendix A: Week 4 worked test data, 520-minute days, target 20.

use oee_core::{day_result, week_report, DayInputs, DayStatus, LossMinutes};

/// Percentage to one decimal place, as shown in the PRD.
fn pct(x: f64) -> f64 {
    (x * 1000.0).round() / 10.0
}

#[allow(clippy::too_many_arguments)]
fn day(
    planned: u32,
    sd: u32,
    co: u32,
    idle: u32,
    mg: u32,
    wait: u32,
    points: u32,
    rework: u32,
) -> DayInputs {
    DayInputs {
        day_minutes: 520,
        planned_minutes: planned,
        losses: LossMinutes::new(sd, co, idle, mg, wait),
        work_points: points,
        rework_points: rework,
        target_points: 20,
    }
}

fn week4() -> Vec<(&'static str, DayInputs)> {
    vec![
        ("Jan 20", day(150, 0, 20, 0, 80, 0, 14, 0)),
        ("Jan 21", day(110, 60, 10, 0, 50, 0, 11, 1)),
        ("Jan 22", day(110, 0, 30, 20, 0, 20, 15, 2)),
        ("Jan 23", day(90, 0, 20, 0, 0, 0, 18, 0)),
        ("Jan 24", day(90, 0, 30, 40, 20, 0, 16, 0)),
    ]
}

#[test]
fn daily_results_match_appendix_a() {
    // (availability, efficiency, quality, oee)
    let expected = [
        (73.0, 70.0, 100.0, 51.1),
        (70.7, 55.0, 95.5, 37.1),
        (82.9, 75.0, 93.3, 58.0),
        (95.3, 90.0, 100.0, 85.8),
        (79.1, 80.0, 100.0, 63.3),
    ];
    for ((date, inputs), exp) in week4().iter().zip(expected) {
        let r = day_result(Some(inputs));
        assert_eq!(r.status, DayStatus::Worked, "{date}");
        let m = r.metrics;
        assert_eq!(
            (
                pct(m.availability),
                pct(m.efficiency),
                pct(m.quality),
                pct(m.oee)
            ),
            exp,
            "{date}"
        );
        assert!(!m.over_capacity, "{date}");
    }
}

#[test]
fn weekly_result_matches_appendix_a() {
    let days = week4();
    let w = week_report(days.iter().map(|(_, d)| Some(d)));
    assert_eq!(w.days_counted, 5);
    assert_eq!(w.totals.day_minutes, 2600);
    assert_eq!(w.totals.planned_minutes, 550);
    assert_eq!(w.totals.downtime_minutes(), 400);
    assert_eq!(w.totals.work_points, 74);
    assert_eq!(w.totals.target_points, 100);
    assert_eq!(w.totals.rework_points, 3);
    let m = w.metrics;
    assert_eq!(
        (
            pct(m.availability),
            pct(m.efficiency),
            pct(m.quality),
            pct(m.oee)
        ),
        (80.5, 74.0, 98.0, 58.4)
    );
}
