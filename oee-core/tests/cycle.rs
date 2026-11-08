//! D-61: the cycle time is a setting, copied onto each day when it starts, and
//! clamped to 5–100 minutes when read from a file.

use oee_core::{hm, lost_blocks_in, BlockKind, DayRecord, LostCategory, Settings, Tracker};

/// 09:00–18:00 is 540 minutes, which 30 divides evenly.
fn thirty() -> Settings {
    Settings {
        default_end: hm(18, 0),
        cycle_minutes: 30,
        ..Settings::default()
    }
}

#[test]
fn an_old_day_file_without_the_field_loads_as_20_minutes() {
    let t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    let mut json: serde_json::Value = serde_json::to_value(&t.record).unwrap();
    json.as_object_mut().unwrap().remove("cycle_minutes");
    json["scored_with"].as_object_mut().unwrap().remove("cycle_minutes");
    let old: DayRecord = serde_json::from_value(json).unwrap();
    assert_eq!(old.cycle_minutes, 20);
    assert_eq!(old.slot_count(), 26);
    assert_eq!(old.scored_with.unwrap().cycle_minutes, 20);
}

#[test]
fn a_new_day_copies_the_setting() {
    let t = Tracker::start_day(hm(9, 0), vec![], thirty());
    assert_eq!(t.record.cycle_minutes, 30);
    let back: DayRecord = serde_json::from_str(&serde_json::to_string(&t.record).unwrap()).unwrap();
    assert_eq!(back.cycle_minutes, 30);
}

#[test]
fn changing_the_setting_leaves_earlier_days_alone_and_applies_to_the_next_new_day() {
    let mut today = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    // The setting changes while the day is running, and the app resumes it.
    let resumed = Tracker::resume(today.record.clone(), thirty(), hm(10, 0));
    assert_eq!(resumed.record.cycle_minutes, 20);
    assert_eq!(resumed.record.slot_count(), 26);
    today.tick(hm(17, 40));
    let tomorrow = Tracker::start_day(hm(9, 0), vec![], thirty());
    assert_eq!(tomorrow.record.cycle_minutes, 30);
    assert_eq!(today.record.cycle_minutes, 20);
}

#[test]
fn a_30_minute_day_builds_30_minute_blocks() {
    let t = Tracker::start_day(hm(9, 0), vec![], thirty());
    let blocks = t.record.blocks();
    assert_eq!(blocks.len(), 18);
    assert!(blocks.iter().all(|b| b.end - b.start == 30));
    assert_eq!(blocks[1].start, hm(9, 30));
}

#[test]
fn a_30_minute_day_counts_lost_blocks_and_change_over_by_the_cycle() {
    // Leeway is a quarter of the cycle: 7 minutes of a 30-minute block is ignored, 8 is not.
    assert_eq!(lost_blocks_in(7, 30), 0);
    assert_eq!(lost_blocks_in(8, 30), 1);
    assert_eq!(lost_blocks_in(30, 30), 1);
    assert_eq!(lost_blocks_in(37, 30), 1);
    assert_eq!(lost_blocks_in(40, 30), 2);
    assert_eq!(lost_blocks_in(45, 30), 2);
    assert_eq!(lost_blocks_in(60, 30), 2);
    assert_eq!(lost_blocks_in(5, 20), 0);
    assert_eq!(lost_blocks_in(6, 20), 1);

    let mut t = Tracker::start_day(hm(9, 0), vec![], thirty());
    t.toggle_downtime(hm(9, 30));
    t.toggle_downtime(hm(10, 0));
    let lost: Vec<u32> = t
        .record
        .blocks()
        .iter()
        .filter(|b| matches!(b.kind, BlockKind::Lost { .. }))
        .map(|b| b.start)
        .collect();
    assert_eq!(lost, vec![hm(9, 30)]);
    t.record.categorise(hm(9, 30), LostCategory::ChangeOver).unwrap();
    let i = t.record.inputs(&thirty());
    assert_eq!(i.day_minutes, 540);
    assert_eq!(i.losses.get(LostCategory::ChangeOver), 15);
    assert_eq!(i.downtime_minutes(), 15);
    assert_eq!(i.work_points, 17);
}

#[test]
fn the_target_uses_the_days_own_cycle_not_the_current_setting() {
    // A 20-minute day scored with a setting that now says 30 still gains 1 point per 20 minutes.
    let early = Tracker::start_day(hm(8, 40), vec![], Settings::default());
    assert_eq!(early.record.inputs(&thirty()).target_points, 21);
    // A 30-minute day starting 30 minutes early gains 1 point.
    let early30 = Tracker::start_day(hm(8, 30), vec![], thirty());
    assert_eq!(early30.record.inputs(&Settings::default()).target_points, 21);
}

#[test]
fn a_cycle_time_read_from_a_file_is_clamped_to_5_to_100() {
    let t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
    for (written, loaded) in [(0, 5), (3, 5), (5, 5), (30, 30), (100, 100), (101, 100), (u32::MAX, 100)] {
        let mut json: serde_json::Value = serde_json::to_value(&t.record).unwrap();
        json["cycle_minutes"] = written.into();
        json["scored_with"]["cycle_minutes"] = written.into();
        let day: DayRecord = serde_json::from_value(json).unwrap();
        assert_eq!(day.cycle_minutes, loaded, "{written}");
        assert_eq!(day.scored_with.unwrap().cycle_minutes, loaded, "{written}");
        // A clamped day still builds blocks without panicking.
        let _ = day.blocks();
    }
    let s: Settings = serde_json::from_str(r#"{"default_start":540,"default_end":1060,"base_target":20,"cycle_minutes":0}"#).unwrap();
    assert_eq!(s.cycle_minutes, 5);
}
