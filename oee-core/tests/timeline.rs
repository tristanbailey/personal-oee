//! Blocks built from a day's downtime, planned windows and edits; the live tracker.

use oee_core::*;

fn day() -> DayRecord {
    DayRecord::new(hm(9, 0), hm(17, 40), vec![])
}

fn lunch() -> PlannedWindow {
    PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch")
}

fn lost_starts(r: &DayRecord) -> Vec<u32> {
    r.blocks()
        .iter()
        .filter(|b| matches!(b.kind, BlockKind::Lost { .. }))
        .map(|b| b.start)
        .collect()
}

#[test]
fn clean_day_is_26_work_blocks() {
    let r = day();
    let blocks = r.blocks();
    assert_eq!(blocks.len(), 26);
    assert!(blocks
        .iter()
        .all(|b| b.kind == BlockKind::Work { rework: false }));
    let i = r.inputs(&Settings::default());
    assert_eq!(
        (i.day_minutes, i.work_points, i.target_points),
        (520, 26, 20)
    );
}

#[test]
fn planned_blocks_are_auto_tagged() {
    // FR-033: lunch 12:00–13:00 covers three whole blocks.
    let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch()]);
    let planned: Vec<_> = r
        .blocks()
        .into_iter()
        .filter(|b| matches!(b.kind, BlockKind::Planned(PlannedKind::Break)))
        .map(|b| b.start)
        .collect();
    assert_eq!(planned, vec![hm(12, 0), hm(12, 20), hm(12, 40)]);
    let i = r.inputs(&Settings::default());
    assert_eq!((i.planned_minutes, i.work_points), (60, 23));
}

#[test]
fn off_grid_meeting_counts_in_whole_blocks() {
    // FR-038: 09:30–10:00 touches the 09:20 and 09:40 blocks, so it counts as 40 min.
    let m = PlannedWindow::new(hm(9, 30), hm(10, 0), PlannedKind::Meeting, "1:1");
    let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![m]);
    let b = r.blocks();
    assert_eq!(b[0].kind, BlockKind::Work { rework: false });
    assert_eq!(b[1].kind, BlockKind::Planned(PlannedKind::Meeting));
    assert_eq!(b[2].kind, BlockKind::Planned(PlannedKind::Meeting));
    assert_eq!(b[3].kind, BlockKind::Work { rework: false });
    assert_eq!(r.planned_minutes(), 40);
    assert_eq!(r.planned[0].start, hm(9, 20));

    // Grid follows the day's start: on an 08:50 start, 09:30–10:00 → 09:30–10:10 (40 min).
    let m = PlannedWindow::new(hm(9, 30), hm(10, 0), PlannedKind::Meeting, "1:1");
    let r = DayRecord::new(hm(8, 50), hm(17, 40), vec![m]);
    assert_eq!(r.planned_minutes(), 40);
    assert_eq!(r.planned[0].start, hm(9, 30));
}

#[test]
fn off_grid_meeting_ends_downtime_at_its_block() {
    // The 09:30 meeting's block starts at 09:20, so running downtime ends there.
    let m = PlannedWindow::new(hm(9, 30), hm(10, 0), PlannedKind::Meeting, "1:1");
    let mut t = Tracker::start_day(hm(9, 0), vec![m], Settings::default());
    t.toggle_downtime(hm(9, 5));
    t.tick(hm(9, 21));
    assert_eq!(
        t.record.downtime,
        vec![DowntimeEvent {
            start: hm(9, 5),
            end: hm(9, 20)
        }]
    );
    assert!(matches!(t.mode(), Mode::Planned { until, .. } if until == hm(10, 0)));
}

#[test]
fn short_stops_are_ignored() {
    // FR-014.
    let mut r = day();
    r.add_downtime(hm(10, 2), hm(10, 7));
    assert!(lost_starts(&r).is_empty());
}

#[test]
fn stop_crossing_a_boundary_is_one_block() {
    // FR-016: 09:15–09:25 crosses 09:20 but is one block.
    let mut r = day();
    r.add_downtime(hm(9, 15), hm(9, 25));
    assert_eq!(lost_starts(&r), vec![hm(9, 0)]);

    // Most overlap wins: 09:17–09:30 sits mainly in the 09:20 block.
    let mut r = day();
    r.add_downtime(hm(9, 17), hm(9, 30));
    assert_eq!(lost_starts(&r), vec![hm(9, 20)]);
}

#[test]
fn long_stops_use_duration() {
    // FR-017: 30 min → 2 blocks, 45 min → 2 blocks.
    let mut r = day();
    r.add_downtime(hm(9, 0), hm(9, 30));
    assert_eq!(lost_starts(&r), vec![hm(9, 0), hm(9, 20)]);

    let mut r = day();
    r.add_downtime(hm(10, 0), hm(10, 45));
    assert_eq!(lost_starts(&r), vec![hm(10, 0), hm(10, 20)]);
}

#[test]
fn categorising_and_defaults() {
    let mut r = day();
    r.add_downtime(hm(9, 0), hm(9, 40)); // two blocks
    let s = Settings::default();

    // FR-027: uncategorised counts as System Down in the maths.
    assert_eq!(r.inputs(&s).losses.get(LostCategory::SystemDown), 40);
    assert_eq!(r.uncategorised().len(), 2);

    // FR-015: Change Over counts as 10 min.
    r.categorise(hm(9, 0), LostCategory::ChangeOver).unwrap();
    let i = r.inputs(&s);
    assert_eq!(i.losses.get(LostCategory::ChangeOver), 10);
    assert_eq!(i.downtime_minutes(), 30);
    assert_eq!(i.work_points, 24, "Change Over scores nothing (FR-023)");

    // FR-027: finalise fills in the rest.
    r.finalise();
    assert!(r.uncategorised().is_empty());
    assert_eq!(
        r.blocks()[1].kind,
        BlockKind::Lost {
            category: Some(LostCategory::SystemDown)
        }
    );
}

#[test]
fn edits_are_validated() {
    let mut r = day();
    r.add_downtime(hm(9, 0), hm(9, 20));
    assert_eq!(r.set_rework(hm(9, 0), true), Err(EditError::NotWork));
    assert_eq!(
        r.categorise(hm(9, 20), LostCategory::Idle),
        Err(EditError::NotLostTime)
    );
    assert_eq!(
        r.set_note(hm(9, 5), Some("x".into())),
        Err(EditError::NoSuchBlock)
    );

    // FR-019, FR-022, FR-025.
    r.set_rework(hm(9, 20), true).unwrap();
    r.set_note(hm(9, 20), Some("Fixed the failing migration".into()))
        .unwrap();
    let i = r.inputs(&Settings::default());
    assert_eq!((i.work_points, i.rework_points), (25, 1));
    assert_eq!(
        r.blocks()[1].note.as_deref(),
        Some("Fixed the failing migration")
    );
}

#[test]
fn tracker_shortcut_and_planned_auto_switch() {
    let mut t = Tracker::start_day(hm(9, 0), vec![lunch()], Settings::default());
    assert_eq!(t.mode(), Mode::Work);

    // FR-011: shortcut on at 11:50.
    assert_eq!(
        t.toggle_downtime(hm(11, 50)),
        Mode::Downtime { since: hm(11, 50) }
    );

    // FR-036: lunch starts at 12:00 and ends the downtime there.
    let n = t.tick(hm(12, 1));
    assert_eq!(
        t.mode(),
        Mode::Planned {
            kind: PlannedKind::Break,
            until: hm(13, 0)
        }
    );
    assert_eq!(
        t.record.downtime,
        vec![DowntimeEvent {
            start: hm(11, 50),
            end: hm(12, 0)
        }]
    );
    assert!(n.contains(&Notice::PlannedStarted {
        kind: PlannedKind::Break,
        label: "Lunch".into()
    }));
    // FR-026: reminder at Planned Downtime (the 10-min stop is one lost block).
    assert!(n.contains(&Notice::CategoriseReminder { blocks: 1 }));

    // Shortcut is ignored during Planned Downtime.
    assert_eq!(
        t.toggle_downtime(hm(12, 30)),
        Mode::Planned {
            kind: PlannedKind::Break,
            until: hm(13, 0)
        }
    );

    // FR-037: back to Work after lunch.
    assert_eq!(t.tick(hm(13, 0)), vec![Notice::PlannedEnded]);
    assert_eq!(t.mode(), Mode::Work);

    // End of day: reminder, then FR-027 default.
    let n = t.tick(hm(17, 40));
    assert_eq!(
        n,
        vec![Notice::DayEnded, Notice::CategoriseReminder { blocks: 1 }]
    );
    t.finalise();
    let i = t.inputs();
    assert_eq!(i.losses.get(LostCategory::SystemDown), 20);
    assert_eq!((i.planned_minutes, i.work_points), (60, 22));
}

#[test]
fn tracker_handles_sleep_through_a_planned_window() {
    let mut t = Tracker::start_day(hm(9, 0), vec![lunch()], Settings::default());
    t.toggle_downtime(hm(11, 0));
    // Laptop asleep until 14:00: downtime still ends at 12:00, and we're back at Work.
    let n = t.tick(hm(14, 0));
    assert_eq!(t.mode(), Mode::Work);
    assert_eq!(
        t.record.downtime,
        vec![DowntimeEvent {
            start: hm(11, 0),
            end: hm(12, 0)
        }]
    );
    assert!(n.contains(&Notice::PlannedEnded));
}

#[test]
fn tracker_early_start_and_extension() {
    // FR-005, FR-006, FR-047: 08:40 start, extended to 18:20.
    let mut t = Tracker::start_day(hm(8, 40), vec![], Settings::default());
    t.tick(hm(17, 40));
    assert_eq!(t.mode(), Mode::Ended);
    t.extend_to(hm(18, 20), hm(17, 45));
    assert_eq!(t.mode(), Mode::Work);
    let i = t.inputs();
    assert_eq!(
        (i.day_minutes, i.target_points, i.work_points),
        (580, 23, 29)
    );
}

#[test]
fn downtime_running_at_end_of_day_is_closed() {
    let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    t.toggle_downtime(hm(17, 0));
    t.tick(hm(18, 0));
    assert_eq!(
        t.record.downtime,
        vec![DowntimeEvent {
            start: hm(17, 0),
            end: hm(17, 40)
        }]
    );
    assert_eq!(t.inputs().work_points, 24);
}

// D-01: live figures only count what has already happened.

#[test]
fn inputs_until_ignores_future_blocks() {
    let r = day();
    let i = r.inputs_until(&Settings::default(), hm(10, 0));
    assert_eq!(
        (i.day_minutes, i.work_points, i.target_points),
        (60, 3, 20)
    );
}

#[test]
fn inputs_until_end_matches_inputs() {
    let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch()]);
    r.add_downtime(hm(10, 0), hm(10, 25));
    let s = Settings::default();
    assert_eq!(r.inputs_until(&s, hm(17, 40)), r.inputs(&s));
    assert_eq!(r.inputs_until(&s, hm(23, 0)), r.inputs(&s));
}

#[test]
fn inputs_until_clips_planned_minutes() {
    let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch()]);
    let s = Settings::default();
    assert_eq!(r.inputs_until(&s, hm(12, 20)).planned_minutes, 20);
    assert_eq!(r.inputs_until(&s, hm(11, 0)).planned_minutes, 0);
}

#[test]
fn metrics_at_ten_on_a_fresh_day_are_not_inflated() {
    let r = day();
    let m = Metrics::from_inputs(&r.inputs_until(&Settings::default(), hm(10, 0)));
    assert!((m.efficiency - 0.15).abs() < 1e-9);
    assert!(!m.over_capacity);
}

// D-02: a running stop shows up before it is toggled off.

#[test]
fn live_record_shows_running_downtime() {
    let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    t.toggle_downtime(hm(10, 5));
    let now = hm(10, 30);
    let live = t.live_record(now);
    let lost = live
        .blocks_until(now)
        .iter()
        .filter(|b| matches!(b.kind, BlockKind::Lost { .. }))
        .count();
    assert_eq!(lost, 1);
    // The real record is untouched until the stop ends.
    assert!(t.record.downtime.is_empty());

    let before = Metrics::from_inputs(&t.record.inputs_until(&t.settings, now));
    let after = Metrics::from_inputs(&t.inputs_at(now));
    assert!(after.availability < before.availability);
}

#[test]
fn live_record_ignores_stop_of_five_minutes_or_less() {
    let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    t.toggle_downtime(hm(10, 5));
    let live = t.live_record(hm(10, 10));
    assert!(live
        .blocks()
        .iter()
        .all(|b| matches!(b.kind, BlockKind::Work { .. })));
}

#[test]
fn live_record_without_a_stop_is_the_record() {
    let t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    assert_eq!(t.live_record(hm(10, 0)), t.record);
}

// D-11: finalising is remembered.

#[test]
fn finalise_sets_the_flag_even_with_nothing_to_categorise() {
    let mut r = day();
    assert!(!r.finalised);
    r.finalise();
    assert!(r.finalised);
}

#[test]
fn finalise_defaults_uncategorised_and_sets_the_flag() {
    let mut r = day();
    r.add_downtime(hm(10, 0), hm(10, 25));
    r.finalise();
    assert!(r.finalised);
    assert!(r.uncategorised().is_empty());
}

#[test]
fn old_saved_records_load_as_not_finalised() {
    let r = day();
    let mut v: serde_json::Value = serde_json::to_value(&r).unwrap();
    v.as_object_mut().unwrap().remove("finalised");
    let back: DayRecord = serde_json::from_value(v).unwrap();
    assert!(!back.finalised);
}

// D-12: rebuild the tracker from a saved record.

fn saved(t: &Tracker) -> DayRecord {
    serde_json::from_str(&serde_json::to_string(&t.record).unwrap()).unwrap()
}

#[test]
fn resume_mid_downtime_keeps_the_original_start() {
    let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    t.toggle_downtime(hm(10, 5));
    let t2 = Tracker::resume(saved(&t), Settings::default(), hm(10, 20));
    assert_eq!(t2.mode(), Mode::Downtime { since: hm(10, 5) });
}

#[test]
fn resume_in_work_after_a_finished_stop() {
    let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    t.toggle_downtime(hm(10, 5));
    t.toggle_downtime(hm(10, 30));
    let t2 = Tracker::resume(saved(&t), Settings::default(), hm(11, 0));
    assert_eq!(t2.mode(), Mode::Work);
    assert_eq!(t2.record.downtime.len(), 1);
}

#[test]
fn resume_after_the_day_ended() {
    let t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    let t2 = Tracker::resume(saved(&t), Settings::default(), hm(18, 0));
    assert_eq!(t2.mode(), Mode::Ended);
}

#[test]
fn resume_closes_downtime_that_ran_past_the_end_or_into_a_planned_window() {
    let mut t = Tracker::start_day(hm(9, 0), vec![lunch()], Settings::default());
    t.toggle_downtime(hm(11, 0));
    let t2 = Tracker::resume(saved(&t), Settings::default(), hm(12, 30));
    assert!(matches!(t2.mode(), Mode::Planned { .. }));
    assert_eq!(
        t2.record.downtime,
        vec![DowntimeEvent {
            start: hm(11, 0),
            end: hm(12, 0)
        }]
    );
    // And the stop is no longer recorded as running.
    let t3 = Tracker::resume(saved(&t2), Settings::default(), hm(13, 30));
    assert_eq!(t3.mode(), Mode::Work);
}

#[test]
fn resume_inside_a_planned_window() {
    let t = Tracker::start_day(hm(9, 0), vec![lunch()], Settings::default());
    let t2 = Tracker::resume(saved(&t), Settings::default(), hm(12, 10));
    assert!(matches!(t2.mode(), Mode::Planned { until, .. } if until == hm(13, 0)));
}
