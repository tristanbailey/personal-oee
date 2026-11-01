//! `report …` and `export csv` (C-08): the numbers must equal the engine's.

use std::path::PathBuf;

use chrono::{Local, NaiveDate, TimeZone};
use oee_cli::{run, Env, Outcome};
use oee_core::{day_result, hm, week_report, DayRecord, Settings};
use oee_store::settings::AppSettings;
use oee_store::store::Store;
use serde_json::Value;

struct Sandbox {
    dir: PathBuf,
}

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

/// Mon 26 Oct: a clean day (26 Work blocks against a target of 20).
fn monday() -> DayRecord {
    DayRecord::new(hm(9, 0), hm(17, 40), vec![])
}

/// Tue 27 Oct: an hour of downtime (3 lost blocks) and 2 rework marks.
fn tuesday() -> DayRecord {
    let mut r = monday();
    r.add_downtime(hm(10, 0), hm(11, 0));
    r.set_rework(hm(9, 0), true).unwrap();
    r.set_rework(hm(9, 20), true).unwrap();
    r
}

impl Sandbox {
    /// Two saved days in the week of 26 Oct; Wed to Fri have nothing logged.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oee-cli-rep-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        store.save_day(d(26), &monday()).unwrap();
        store.save_day(d(27), &tuesday()).unwrap();
        Self { dir }
    }

    /// The system date is Mon 2 Nov 2026 12:00, so the week of 26 Oct is finished.
    fn go(&self, args: &[&str]) -> Outcome {
        let mut full = vec!["office-oee", "--data-dir", self.dir.to_str().unwrap()];
        full.extend_from_slice(args);
        let system = Local.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
        run(full, &Env::default(), system)
    }

    fn ok(&self, args: &[&str]) -> Value {
        let o = self.go(args);
        assert_eq!(o.exit, 0, "{args:?}: {}", o.stdout);
        serde_json::from_str(o.stdout.trim()).unwrap()
    }

    fn err(&self, args: &[&str], exit: i32, code: &str) {
        let o = self.go(args);
        assert_eq!(o.exit, exit, "{args:?}: {}", o.stdout);
        let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
        assert_eq!(v["error"]["code"], code, "{args:?}: {}", o.stdout);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn expected_week() -> oee_core::WeekReport {
    let s = Settings::default();
    let (m, t) = (monday().inputs(&s), tuesday().inputs(&s));
    week_report([Some(&m), Some(&t), None, None, None])
}

fn close(a: &Value, b: f64) {
    assert!((a.as_f64().unwrap() - b).abs() < 1e-6, "{a} vs {b}");
}

#[test]
fn report_day_matches_the_engine_and_a_missing_day_is_a_holiday() {
    let s = Sandbox::new("day");
    let v = s.ok(&["report", "day", "--date", "2026-10-27"]);
    let want = day_result(Some(&tuesday().inputs(&Settings::default()))).metrics;
    let m = &v["result"]["metrics"];
    close(&m["oee"], want.oee);
    close(&m["availability"], want.availability);
    close(&m["quality"], want.quality);
    assert_eq!(m["status"], "worked");
    assert_eq!(v["result"]["inputs"]["work_points"], 23);
    assert_eq!(v["result"]["inputs"]["rework_points"], 2);

    let v = s.ok(&["report", "day", "--date", "2026-10-28"]);
    assert_eq!(v["result"]["metrics"]["status"], "holiday");
    assert_eq!(v["result"]["metrics"]["oee"], 0.0);
    assert_eq!(v["result"]["inputs"], Value::Null);
}

#[test]
fn report_week_pools_saved_days_exactly_as_the_engine_does() {
    let s = Sandbox::new("week");
    let v = s.ok(&["report", "week", "--week-of", "2026-10-29"]);
    let r = &v["result"];
    let want = expected_week();
    assert_eq!(r["week_of"], "2026-10-26");
    assert_eq!(r["week_end"], "2026-11-01");
    assert_eq!(r["days_counted"], 2);
    assert_eq!(r["working_days"], 2);
    close(&r["pooled"]["metrics"]["oee"], want.metrics.oee);
    assert_eq!(
        r["pooled"]["metrics"]["display"]["oee"],
        format!("{:.1}%", want.metrics.oee * 100.0)
    );
    assert_eq!(r["pooled"]["inputs"]["target_points"], 40);
    assert_eq!(r["pooled"]["inputs"]["work_points"], 26 + 23);
    // Mon to Fri; no weekend rows because they have no record.
    let days = r["days"].as_array().unwrap();
    assert_eq!(days.len(), 5);
    assert_eq!(days[0]["weekday"], "Mon");
    assert_eq!(days[0]["points"], "26/20");
    assert_eq!(days[2]["status"], "holiday");
    assert_eq!(days[2]["points"], Value::Null);
    // Both days beat their target of 20 points.
    let flagged: Vec<_> = r["over_capacity_days"].as_array().unwrap().iter().map(|x| x["date"].as_str().unwrap()).collect();
    assert_eq!(flagged, ["2026-10-27", "2026-10-26"]);
}

#[test]
fn the_week_of_flag_and_the_date_flag_pick_the_same_week() {
    let s = Sandbox::new("weekof");
    let a = s.ok(&["report", "week", "--week-of", "2026-10-30"]);
    let b = s.ok(&["report", "week", "--date", "2026-10-28"]);
    assert_eq!(a["result"], b["result"]);
}

#[test]
fn the_goal_is_reported_with_status_and_streak() {
    let s = Sandbox::new("goal");
    // Whether the week meets 85% is decided by the engine's own figure.
    let want = expected_week().metrics.oee;
    let v = s.ok(&["report", "goal", "--week-of", "2026-10-26"]);
    let g = &v["result"]["goal"];
    assert_eq!(g["percent"], 85);
    let met = (want * 1000.0).round() >= 850.0;
    assert_eq!(g["status"], if met { "met" } else { "not-met" });
    assert_eq!(g["streak_weeks"], u32::from(met));
    close(&v["result"]["oee"], want);
}

#[test]
fn a_goal_that_is_off_is_null() {
    let s = Sandbox::new("nogoal");
    let mut settings = AppSettings::default();
    settings.goal.enabled = false;
    Store::new(&s.dir).save_settings(&settings).unwrap();
    assert_eq!(s.ok(&["report", "goal"])["result"]["goal"], Value::Null);
    assert_eq!(s.ok(&["report", "week"])["result"]["goal"], Value::Null);
    assert_eq!(s.ok(&["report", "trends"])["result"]["goal_percent"], Value::Null);
}

#[test]
fn trends_give_one_point_per_working_week() {
    let s = Sandbox::new("trends");
    let v = s.ok(&["report", "trends", "--weeks", "4"]);
    let points = v["result"]["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0]["week_of"], "2026-10-26");
    assert_eq!(points[0]["days_counted"], 2);
    close(&points[0]["metrics"]["oee"], expected_week().metrics.oee);
    assert_eq!(v["result"]["from"], "2026-10-12");
    s.err(&["report", "trends", "--weeks", "5"], 2, "bad_weeks");
}

#[test]
fn losses_pool_minutes_and_blocks_per_category() {
    let s = Sandbox::new("losses");
    let v = s.ok(&["report", "losses"]);
    let r = &v["result"];
    assert_eq!(r["days"], 2);
    assert_eq!(r["total_minutes"], 60);
    let cats = r["categories"].as_array().unwrap();
    assert_eq!(cats.len(), 5);
    // Uncategorised lost time counts as System Down (FR-027).
    assert_eq!((cats[0]["category"].as_str(), cats[0]["minutes"].as_u64(), cats[0]["blocks"].as_u64()), (Some("system-down"), Some(60), Some(3)));
    assert_eq!(r["rework_blocks"], 2);
    assert_eq!(r["rework_minutes"], 40);
    let v = s.ok(&["report", "losses", "--from", "2026-10-26", "--to", "2026-10-26"]);
    assert_eq!(v["result"]["total_minutes"], 0);
    assert_eq!(v["result"]["days"], 1);
}

#[test]
fn export_returns_csv_text_with_one_row_per_day() {
    let s = Sandbox::new("csv");
    let v = s.ok(&["export", "csv", "--from", "2026-10-26", "--to", "2026-10-30", "--stdout", "days"]);
    let csv = v["result"]["csv"].as_str().unwrap();
    let lines: Vec<_> = csv.lines().collect();
    // Header, two worked days, three holidays.
    assert_eq!(lines.len(), 6, "{csv}");
    assert!(lines[1].starts_with("2026-10-26"));
    assert!(lines[3].contains("holiday"));
    let blocks = s.ok(&["export", "csv", "--from", "2026-10-26", "--to", "2026-10-26", "--stdout", "blocks"]);
    assert_eq!(blocks["result"]["csv"].as_str().unwrap().lines().count(), 1 + 26);
}

#[test]
fn export_writes_both_files_and_never_overwrites() {
    let s = Sandbox::new("files");
    let out = s.dir.join("exports");
    let out = out.to_str().unwrap();
    let a = s.ok(&["export", "csv", "--all", "--out", out]);
    let days = a["result"]["files"]["days"].as_str().unwrap();
    let blocks = a["result"]["files"]["blocks"].as_str().unwrap();
    assert!(days.ends_with("oee-days-2026-10-26-2026-10-27.csv"), "{days}");
    assert!(std::fs::read_to_string(days).unwrap().starts_with('\u{feff}'));
    assert!(std::path::Path::new(blocks).exists());
    let b = s.ok(&["export", "csv", "--all", "--out", out]);
    assert!(b["result"]["files"]["days"].as_str().unwrap().ends_with("(2).csv"));
    // A dry run writes nothing.
    let c = s.ok(&["export", "csv", "--all", "--out", out, "--dry-run"]);
    assert_eq!(c["result"]["files"]["written"], false);
    assert_eq!(std::fs::read_dir(out).unwrap().count(), 4);
}

#[test]
fn export_needs_somewhere_to_put_the_csv_and_something_to_export() {
    let s = Sandbox::new("csverr");
    s.err(&["export", "csv"], 2, "output_required");
    s.err(&["export", "csv", "--from", "2026-10-27", "--to", "2026-10-26", "--stdout", "days"], 2, "bad_range");
    let empty = std::env::temp_dir().join(format!("oee-cli-rep-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty);
    let o = run(
        ["office-oee", "--data-dir", empty.to_str().unwrap(), "export", "csv", "--all", "--stdout", "days"],
        &Env::default(),
        Local::now(),
    );
    assert_eq!(o.exit, 3);
    assert!(o.stdout.contains("no_days"));
}

#[test]
fn reports_work_on_an_empty_folder() {
    let empty = std::env::temp_dir().join(format!("oee-cli-rep-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty);
    let run_it = |args: &[&str]| {
        let mut full = vec!["office-oee", "--data-dir", empty.to_str().unwrap()];
        full.extend_from_slice(args);
        run(full, &Env::default(), Local.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap())
    };
    for args in [&["report", "week"][..], &["report", "trends"][..], &["report", "goal"][..], &["report", "losses"][..], &["report", "day"][..]] {
        let o = run_it(args);
        assert_eq!(o.exit, 0, "{args:?}: {}", o.stdout);
    }
    let v: Value = serde_json::from_str(run_it(&["report", "week"]).stdout.trim()).unwrap();
    assert_eq!(v["result"]["days_counted"], 0);
    assert_eq!(v["result"]["goal"]["status"], Value::Null);
    assert!(!empty.exists(), "reading must not create the folder");
}
