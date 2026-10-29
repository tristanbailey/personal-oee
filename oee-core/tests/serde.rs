//! D-10: everything the app saves survives a JSON round trip.

use oee_core::*;

fn round_trip<T>(v: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

#[test]
fn day_record_keeps_categories_rework_and_notes() {
    let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
    let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch]);
    r.add_downtime(hm(10, 0), hm(10, 25));
    r.categorise(hm(10, 0), LostCategory::Wait).unwrap();
    r.set_rework(hm(9, 0), true).unwrap();
    r.set_note(hm(9, 20), Some("pairing".into())).unwrap();

    let back = round_trip(&r);
    assert_eq!(back, r);
    assert_eq!(back.blocks(), r.blocks());
    let b = back.blocks();
    assert_eq!(
        b[3].kind,
        BlockKind::Lost {
            category: Some(LostCategory::Wait)
        }
    );
    assert_eq!(b[0].kind, BlockKind::Work { rework: true });
    assert_eq!(b[1].note.as_deref(), Some("pairing"));
}

#[test]
fn calendar_and_settings_round_trip() {
    let mut c = WeeklyCalendar::default();
    c.add(
        Weekday::Tue,
        PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"),
    );
    assert_eq!(round_trip(&c), c);
    let s = Settings::default();
    assert_eq!(round_trip(&s), s);
}
