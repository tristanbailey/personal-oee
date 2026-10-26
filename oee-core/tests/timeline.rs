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
fn off_grid_meeting_tags_blocks_it_mostly_covers() {
    // 09:30–10:00: 10 min of the 09:20 block and all of the 09:40 block.
    let m = PlannedWindow::new(hm(9, 30), hm(10, 0), PlannedKind::Meeting, "1:1");
    let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![m]);
    let b = r.blocks();
    assert_eq!(b[0].kind, BlockKind::Work { rework: false });
    assert_eq!(b[1].kind, BlockKind::Planned(PlannedKind::Meeting));
    assert_eq!(b[2].kind, BlockKind::Planned(PlannedKind::Meeting));
    // Availability uses the exact planned minutes.
    assert_eq!(r.planned_minutes(), 30);
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
