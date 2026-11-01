//! D-55: weekly results over time.

use oee_core::*;

/// A full day: 26 blocks, `work` of them Work, `rework` marked.
fn day(work: u32, rework: u32) -> DayInputs {
    let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
    let lost = 26 - work;
    for i in 0..lost {
        r.add_downtime(hm(9, 0) + i * 20, hm(9, 0) + (i + 1) * 20);
    }
    for b in r.blocks().into_iter().filter(|b| matches!(b.kind, BlockKind::Work { .. })).take(rework as usize) {
        r.set_rework(b.start, true).unwrap();
    }
    r.inputs(&Settings::default())
}

#[test]
fn one_point_per_working_week_oldest_first() {
    let days = vec![
        (2, Some(day(20, 0))),
        (1, Some(day(20, 0))),
        (1, Some(day(10, 0))),
        (3, Some(day(18, 0))),
    ];
    let s = weekly_series(days);
    assert_eq!(s.iter().map(|p| p.week).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(s.iter().map(|p| p.days_counted).collect::<Vec<_>>(), vec![2, 1, 1]);
}

#[test]
fn a_week_is_pooled_not_averaged() {
    let (a, b) = (day(20, 0), day(10, 0));
    let s = weekly_series(vec![(1, Some(a)), (1, Some(b))]);
    let pooled = week_report([Some(&a), Some(&b)]).metrics;
    assert_eq!(s[0].metrics, pooled);
    let mean = (Metrics::from_inputs(&a).oee + Metrics::from_inputs(&b).oee) / 2.0;
    assert!((s[0].metrics.oee - mean).abs() > 0.001);
}

#[test]
fn holidays_are_left_out_and_an_all_holiday_week_is_skipped() {
    let s = weekly_series(vec![
        (1, Some(day(20, 0))),
        (1, None),
        (2, None),
        (2, None),
        (3, Some(day(20, 0))),
    ]);
    assert_eq!(s.iter().map(|p| p.week).collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(s[0].days_counted, 1);
    assert_eq!(s[0].metrics, Metrics::from_inputs(&day(20, 0)));
}

#[test]
fn over_capacity_weeks_are_flagged() {
    let s = weekly_series(vec![(1, Some(day(26, 0))), (2, Some(day(18, 0)))]);
    assert!(s[0].metrics.over_capacity);
    assert!(!s[1].metrics.over_capacity);
}

#[test]
fn no_days_gives_no_points() {
    assert!(weekly_series(Vec::<(u32, Option<DayInputs>)>::new()).is_empty());
}
