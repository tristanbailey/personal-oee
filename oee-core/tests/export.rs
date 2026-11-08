//! CSV export (D-51): one row per day and one row per block; D-60 spreadsheet totals.

use oee_core::export::{blocks_csv, date_label, days_csv, totals_csv, ExportDay};
use oee_core::*;

fn day(date: &str, record: DayRecord) -> ExportDay {
    let until = record.end;
    ExportDay {
        date: date.into(),
        record: Some(record),
        until,
    }
}

fn holiday(date: &str) -> ExportDay {
    ExportDay {
        date: date.into(),
        record: None,
        until: 0,
    }
}

fn clean() -> DayRecord {
    DayRecord::new(hm(9, 0), hm(17, 40), vec![])
}

fn lines(csv: &str) -> Vec<&str> {
    csv.split("\r\n").filter(|l| !l.is_empty()).collect()
}

fn cells(line: &str) -> Vec<&str> {
    line.split(',').collect()
}

#[test]
fn days_header_names_every_column() {
    let csv = days_csv(&[], &Settings::default());
    assert_eq!(
        lines(&csv),
        vec![
            "date,status,start,end,day_minutes,planned_minutes,lost_minutes,work_points,\
             rework_points,target_points,availability,efficiency,quality,oee,over_capacity,\
             system_down_minutes,change_over_minutes,idle_minutes,management_minutes,wait_minutes"
        ]
    );
}

#[test]
fn a_clean_day_is_one_worked_row() {
    let csv = days_csv(&[day("2026-10-26", clean())], &Settings::default());
    let rows = lines(&csv);
    assert_eq!(rows.len(), 2);
    let c = cells(rows[1]);
    assert_eq!(&c[0..4], ["2026-10-26", "worked", "09:00", "17:40"]);
    assert_eq!(&c[4..10], ["520", "0", "0", "26", "0", "20"]);
    // 26 work blocks against a target of 20: over capacity.
    assert_eq!(c[14], "true");
    assert_eq!(&c[15..], ["0", "0", "0", "0", "0"]);
}

#[test]
fn percentages_are_plain_numbers_between_0_and_1() {
    let mut r = clean();
    r.add_downtime(hm(10, 0), hm(10, 20));
    let csv = days_csv(&[day("2026-10-26", r)], &Settings::default());
    let c = cells(lines(&csv)[1]).into_iter().map(String::from).collect::<Vec<_>>();
    for col in &c[10..14] {
        let v: f64 = col.parse().expect("a number, not '87%'");
        assert!((0.0..=2.0).contains(&v), "{col}");
    }
}

#[test]
fn a_holiday_is_a_zero_row() {
    let csv = days_csv(&[holiday("2026-10-27")], &Settings::default());
    let c = cells(lines(&csv)[1]).into_iter().map(String::from).collect::<Vec<_>>();
    assert_eq!(&c[0..2], ["2026-10-27", "holiday"]);
    assert_eq!(&c[2..4], ["", ""]);
    assert!(c[4..14].iter().all(|v| v == "0" || v == "0.0000"), "{c:?}");
    assert_eq!(c[14], "false");
}

#[test]
fn a_day_of_only_lost_time_is_a_zero_work_day() {
    let mut r = DayRecord::new(hm(9, 0), hm(10, 0), vec![]);
    r.add_downtime(hm(9, 0), hm(10, 0));
    let csv = days_csv(&[day("2026-10-28", r)], &Settings::default());
    let c = cells(lines(&csv)[1]).into_iter().map(String::from).collect::<Vec<_>>();
    assert_eq!(c[1], "zero_work");
    assert_eq!(c[7], "0");
    assert_eq!(c[13], "0.0000");
    // Uncategorised lost time counts as System Down (FR-027).
    assert_eq!(c[15], "60");
}

#[test]
fn loss_minutes_follow_the_category_and_change_over_counts_ten() {
    let mut r = clean();
    r.add_downtime(hm(10, 0), hm(10, 20));
    r.add_downtime(hm(11, 0), hm(11, 20));
    r.categorise(hm(10, 0), LostCategory::ChangeOver).unwrap();
    r.categorise(hm(11, 0), LostCategory::Wait).unwrap();
    let csv = days_csv(&[day("2026-10-26", r)], &Settings::default());
    let c = cells(lines(&csv)[1]).into_iter().map(String::from).collect::<Vec<_>>();
    assert_eq!(c[6], "30");
    assert_eq!(&c[15..], ["0", "10", "0", "0", "20"]);
}

#[test]
fn a_day_in_progress_is_scored_only_up_to_now() {
    let mut e = day("2026-10-30", clean());
    e.until = hm(10, 0);
    let csv = days_csv(&[e], &Settings::default());
    let c = cells(lines(&csv)[1]).into_iter().map(String::from).collect::<Vec<_>>();
    assert_eq!(c[4], "60");
    assert_eq!(c[7], "3");
}

#[test]
fn blocks_has_one_row_per_block() {
    let csv = blocks_csv(&[day("2026-10-26", clean()), holiday("2026-10-27")], &Settings::default());
    let rows = lines(&csv);
    assert_eq!(rows[0], "date,block_start,block_end,type,category,rework,note,uncategorised");
    assert_eq!(rows.len(), 1 + 26);
    assert_eq!(rows[1], "2026-10-26,09:00,09:20,work,,false,,false");
    assert_eq!(rows[26], "2026-10-26,17:20,17:40,work,,false,,false");
}

#[test]
fn block_types_categories_rework_and_uncategorised() {
    let lunch = PlannedWindow::new(hm(12, 0), hm(12, 20), PlannedKind::Break, "Lunch");
    let mut r = DayRecord::new(hm(9, 0), hm(13, 0), vec![lunch]);
    r.add_downtime(hm(9, 0), hm(9, 20));
    r.add_downtime(hm(10, 0), hm(10, 20));
    r.categorise(hm(10, 0), LostCategory::Idle).unwrap();
    r.set_rework(hm(11, 0), true).unwrap();
    let csv = blocks_csv(&[day("2026-10-26", r)], &Settings::default());
    let rows = lines(&csv);
    // Uncategorised lost time shows as System Down, and is flagged.
    assert_eq!(rows[1], "2026-10-26,09:00,09:20,lost,System Down,false,,true");
    assert_eq!(rows[4], "2026-10-26,10:00,10:20,lost,Idle,false,,false");
    assert_eq!(rows[7], "2026-10-26,11:00,11:20,work,,true,,false");
    assert_eq!(rows[10], "2026-10-26,12:00,12:20,break,,false,,false");
}

#[test]
fn a_day_in_progress_lists_only_finished_blocks() {
    let mut e = day("2026-10-30", clean());
    e.until = hm(10, 0);
    let rows_csv = blocks_csv(&[e], &Settings::default());
    assert_eq!(lines(&rows_csv).len(), 1 + 3);
}

#[test]
fn notes_are_quoted_when_they_hold_commas_quotes_or_line_breaks() {
    let mut r = clean();
    r.set_note(hm(9, 0), Some("a, b".into())).unwrap();
    r.set_note(hm(9, 20), Some("say \"hi\"".into())).unwrap();
    r.set_note(hm(9, 40), Some("two\nlines".into())).unwrap();
    let csv = blocks_csv(&[day("2026-10-26", r)], &Settings::default());
    assert!(csv.contains(",false,\"a, b\",false\r\n"));
    assert!(csv.contains(",false,\"say \"\"hi\"\"\",false\r\n"));
    assert!(csv.contains(",false,\"two\nlines\",false\r\n"));
}

#[test]
fn notes_that_look_like_formulas_are_defused() {
    let mut r = clean();
    r.set_note(hm(9, 0), Some("=SUM(A1:A9)".into())).unwrap();
    r.set_note(hm(9, 20), Some("-2+3".into())).unwrap();
    r.set_note(hm(9, 40), Some("@cmd".into())).unwrap();
    r.set_note(hm(10, 0), Some("+1".into())).unwrap();
    let csv = blocks_csv(&[day("2026-10-26", r)], &Settings::default());
    assert!(csv.contains(",false,'=SUM(A1:A9),false\r\n"));
    assert!(csv.contains(",false,'-2+3,false\r\n"));
    assert!(csv.contains(",false,'@cmd,false\r\n"));
    assert!(csv.contains(",false,'+1,false\r\n"));
}

// D-60: spreadsheet totals.

const TOTALS_HEADER: &str = "Start Date,Week num,Total downtime,Total shift,Planned Non-Productive,\
                             System down,Change over,Idle,Manage,Wait,Total points,Rework points,Cycle time";

fn totals_row(e: ExportDay) -> String {
    let csv = totals_csv(&[e], &Settings::default());
    let rows = lines(&csv);
    assert_eq!(rows[0], TOTALS_HEADER);
    assert_eq!(rows.len(), 2);
    rows[1].to_string()
}

#[test]
fn totals_header_is_in_the_spreadsheet_order() {
    assert_eq!(lines(&totals_csv(&[], &Settings::default())), vec![TOTALS_HEADER]);
}

#[test]
fn totals_for_a_normal_day_match_the_spreadsheet_example() {
    // Like the backlog's example row: 100 minutes of breaks and meetings, and
    // Change Over counted at 10 minutes a block.
    let planned = vec![
        PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"),
        PlannedWindow::new(hm(15, 0), hm(15, 40), PlannedKind::Meeting, "Review"),
    ];
    let mut r = DayRecord::new(hm(9, 0), hm(17, 40), planned);
    for s in [hm(9, 0), hm(9, 20), hm(9, 40), hm(10, 0)] {
        r.add_downtime(s, s + 20);
        r.categorise(s, LostCategory::ChangeOver).unwrap();
    }
    for s in [hm(10, 20), hm(10, 40), hm(11, 0), hm(11, 20), hm(11, 40)] {
        r.add_downtime(s, s + 20);
        r.categorise(s, LostCategory::Idle).unwrap();
    }
    r.set_rework(hm(14, 0), true).unwrap();
    // 26 slots: 5 planned, 4 Change Over, 5 Idle, 12 Work (1 Rework).
    let row = totals_row(day("2025-08-18", r));
    assert_eq!(row, "Aug 18,34,140,520,100,0,40,100,0,0,12,1,20");
}

#[test]
fn totals_for_a_holiday_are_a_row_of_zeros() {
    assert_eq!(totals_row(holiday("2026-10-27")), "Oct 27,44,0,0,0,0,0,0,0,0,0,0,0");
}

#[test]
fn totals_for_a_zero_work_day() {
    let mut r = DayRecord::new(hm(9, 0), hm(10, 0), vec![]);
    r.add_downtime(hm(9, 0), hm(10, 0));
    // Uncategorised lost time counts as System Down (FR-027).
    assert_eq!(totals_row(day("2026-10-28", r)), "Oct 28,44,60,60,0,60,0,0,0,0,0,0,20");
}

#[test]
fn totals_count_change_over_at_half_a_block() {
    let mut r = clean();
    r.add_downtime(hm(10, 0), hm(10, 20));
    r.add_downtime(hm(11, 0), hm(11, 20));
    r.categorise(hm(10, 0), LostCategory::ChangeOver).unwrap();
    r.categorise(hm(11, 0), LostCategory::Management).unwrap();
    // Downtime 10 + 20; 24 Work blocks.
    assert_eq!(totals_row(day("2026-10-26", r)), "Oct 26,44,30,520,0,0,10,0,20,0,24,0,20");
}

#[test]
fn totals_for_a_day_in_progress_count_only_up_to_now() {
    let mut r = clean();
    r.add_downtime(hm(9, 20), hm(9, 40));
    let mut e = day("2026-10-30", r);
    e.until = hm(10, 0);
    assert_eq!(totals_row(e), "Oct 30,44,20,60,0,20,0,0,0,0,2,0,20");
}

#[test]
fn totals_use_each_days_own_cycle_time() {
    // Week 44 at 20 minutes, week 45 at 30 minutes, each with one Wait block.
    let mut a = clean();
    a.add_downtime(hm(9, 0), hm(9, 20));
    a.categorise(hm(9, 0), LostCategory::Wait).unwrap();
    let mut b = DayRecord::with_cycle(hm(9, 0), hm(18, 0), vec![], 30);
    b.add_downtime(hm(9, 0), hm(9, 30));
    b.categorise(hm(9, 0), LostCategory::Wait).unwrap();
    let csv = totals_csv(&[day("2026-10-30", a), day("2026-11-02", b)], &Settings::default());
    let rows = lines(&csv);
    assert_eq!(rows[1], "Oct 30,44,20,520,0,0,0,0,0,20,25,0,20");
    assert_eq!(rows[2], "Nov 2,45,30,540,0,0,0,0,0,30,17,0,30");
}

#[test]
fn date_labels_and_iso_weeks_match_chrono_for_every_day_of_a_decade() {
    use chrono::{Datelike, NaiveDate};
    let mut d = NaiveDate::from_ymd_opt(2019, 12, 1).unwrap();
    while d.year() < 2031 {
        let (label, week) = date_label(&d.to_string()).unwrap();
        assert_eq!(label, d.format("%b %-d").to_string(), "{d}");
        assert_eq!(week, d.iso_week().week(), "{d}");
        d = d.succ_opt().unwrap();
    }
    assert_eq!(date_label("2026-02-30"), None);
    assert_eq!(date_label("not a date"), None);
}
