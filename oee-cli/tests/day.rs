//! `day …` commands (C-05), driven through `run` against a throwaway folder.

use std::path::PathBuf;

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use oee_core::{hm, PlannedKind, PlannedWindow, Weekday, WeeklyCalendar};
use oee_store::store::Store;
use serde_json::Value;

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oee-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }

    /// Run with this sandbox as the data folder. The system date is Mon 2 Nov 2026.
    fn go(&self, args: &[&str]) -> Outcome {
        let mut full = vec!["office-oee", "--data-dir", self.dir.to_str().unwrap()];
        full.extend_from_slice(args);
        let system = Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap();
        run(full, &Env::default(), system)
    }

    fn ok(&self, args: &[&str]) -> Value {
        let o = self.go(args);
        assert_eq!(o.exit, 0, "{args:?}: {}", o.stdout);
        serde_json::from_str(o.stdout.trim()).unwrap()
    }

    fn err(&self, args: &[&str], exit: i32, code: &str) -> Value {
        let o = self.go(args);
        assert_eq!(o.exit, exit, "{args:?}: {}", o.stdout);
        let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
        assert_eq!(v["error"]["code"], code, "{args:?}: {}", o.stdout);
        v
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_day_runs_from_start_to_finish() {
    let s = Sandbox::new("lifecycle");
    let v = s.ok(&["day", "start", "--now", "09:00"]);
    assert_eq!(v["command"], "day start");
    assert_eq!(v["result"]["mode"]["kind"], "work");
    assert_eq!(v["result"]["start"], "09:00");
    assert_eq!(v["result"]["end"], "17:40");
    assert_eq!(v["result"]["saved"], true);
    assert_eq!(v["result"]["block"]["number"], 1);
    assert_eq!(v["result"]["block"]["of"], 26);

    let v = s.ok(&["day", "downtime", "start", "--now", "10:05"]);
    assert_eq!(v["result"]["mode"]["kind"], "downtime");
    assert_eq!(v["result"]["mode"]["since"], "10:05");

    // 25 minutes: one lost block (FR-017).
    let v = s.ok(&["day", "downtime", "stop", "--now", "10:30"]);
    assert_eq!(v["result"]["stopped"]["minutes"], 25);
    assert_eq!(v["result"]["stopped"]["lost_blocks"], 1);
    assert_eq!(
        v["result"]["stopped"]["message"],
        "25 min stop logged as 1 lost block. Categorise it later."
    );

    // 11:00: six blocks done, one lost, so 5 Work points and 20 min down.
    let v = s.ok(&["day", "status", "--now", "11:00"]);
    let r = &v["result"];
    assert_eq!(r["inputs"]["downtime_minutes"], 20);
    assert_eq!(r["inputs"]["work_points"], 5);
    assert_eq!(r["inputs"]["target_points"], 20);
    assert_eq!(r["uncategorised_blocks"], 1);
    assert_eq!(r["metrics"]["display"]["efficiency"], "25.0%");
    assert_eq!(r["metrics"]["status"], "worked");

    let v = s.ok(&["day", "finish", "--now", "17:40"]);
    assert_eq!(v["result"]["finalised"], true);
    assert_eq!(v["result"]["defaulted_to_system_down"], 1);
    assert_eq!(v["result"]["uncategorised_blocks"], 0);
}

#[test]
fn a_stop_of_five_minutes_or_less_is_ignored() {
    let s = Sandbox::new("short");
    s.ok(&["day", "start", "--now", "09:00"]);
    s.ok(&["day", "toggle", "--now", "10:00"]);
    let v = s.ok(&["day", "toggle", "--now", "10:03"]);
    assert_eq!(v["result"]["stopped"]["lost_blocks"], 0);
    assert_eq!(v["result"]["stopped"]["message"], "3 min stop ignored. It counts as Work.");
    assert_eq!(v["result"]["mode"]["kind"], "work");
}

#[test]
fn toggle_without_a_stop_has_null_stopped() {
    let s = Sandbox::new("toggle");
    s.ok(&["day", "start", "--now", "09:00"]);
    let v = s.ok(&["day", "toggle", "--now", "10:00"]);
    assert_eq!(v["result"]["stopped"], Value::Null);
    assert_eq!(v["result"]["mode"]["kind"], "downtime");
}

#[test]
fn the_state_survives_between_calls_because_it_is_saved() {
    let s = Sandbox::new("persist");
    s.ok(&["day", "start", "--now", "09:00"]);
    s.ok(&["day", "toggle", "--now", "10:05"]);
    let v = s.ok(&["day", "status", "--now", "10:20"]);
    assert_eq!(v["result"]["mode"]["since"], "10:05");
    assert_eq!(v["result"]["block"]["start"], "10:20");
    assert_eq!(v["result"]["block"]["minutes_left"], 20);
}

#[test]
fn refusals_use_the_documented_codes() {
    let s = Sandbox::new("refusals");
    s.err(&["day", "status", "--now", "09:00"], 3, "no_day");
    s.err(&["day", "toggle", "--now", "09:00"], 3, "no_day");
    s.ok(&["day", "start", "--now", "09:00"]);
    s.err(&["day", "start", "--now", "09:05"], 4, "day_exists");
    s.err(&["day", "downtime", "stop", "--now", "09:10"], 4, "not_in_downtime");
    s.ok(&["day", "downtime", "start", "--now", "09:10"]);
    s.err(&["day", "downtime", "start", "--now", "09:15"], 4, "already_in_downtime");
    s.err(&["day", "start", "--at", "25:00", "--now", "09:15", "--date", "2026-11-03"], 4, "future_date");
}

#[test]
fn a_start_time_later_than_now_is_a_usage_error() {
    let s = Sandbox::new("badstart");
    s.err(&["day", "start", "--at", "10:00", "--now", "09:00"], 2, "bad_start");
}

#[test]
fn an_early_start_raises_the_target() {
    let s = Sandbox::new("early");
    let v = s.ok(&["day", "start", "--at", "08:40", "--now", "09:00"]);
    assert_eq!(v["result"]["start"], "08:40");
    assert_eq!(v["result"]["inputs"]["target_points"], 21);
}

#[test]
fn a_write_needs_a_data_location() {
    let o = run(["office-oee", "day", "start", "--now", "09:00"], &Env::default(), Local::now());
    assert_eq!(o.exit, 7);
    assert!(o.stdout.contains("data_location_required"));
}

#[test]
fn dry_run_shows_the_result_and_saves_nothing() {
    let s = Sandbox::new("dry");
    let v = s.ok(&["day", "start", "--now", "09:00", "--dry-run"]);
    assert_eq!(v["result"]["saved"], false);
    s.err(&["day", "status", "--now", "09:00"], 3, "no_day");
}

#[test]
fn the_calendar_puts_the_day_into_a_planned_break_and_says_so() {
    let s = Sandbox::new("lunch");
    let mut cal = WeeklyCalendar::default();
    cal.add(Weekday::Mon, PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"));
    Store::new(&s.dir).save_calendar(&cal).unwrap();
    s.ok(&["day", "start", "--now", "09:00"]);
    let v = s.ok(&["day", "status", "--now", "12:10"]);
    assert_eq!(v["result"]["mode"]["kind"], "planned");
    assert_eq!(v["result"]["mode"]["until"], "13:00");
    assert_eq!(v["notices"][0]["type"], "planned_started");
    assert_eq!(v["notices"][0]["label"], "Lunch");
    s.err(&["day", "toggle", "--now", "12:10"], 4, "in_planned_downtime");
}

#[test]
fn extend_and_end() {
    let s = Sandbox::new("extend");
    s.ok(&["day", "start", "--now", "09:00"]);
    s.err(&["day", "extend", "--to", "17:00", "--now", "10:00"], 4, "not_later");
    s.err(&["day", "extend", "--to", "later", "--now", "10:00"], 2, "bad_time");
    let v = s.ok(&["day", "extend", "--to", "18:20", "--now", "10:00"]);
    assert_eq!(v["result"]["end"], "18:20");
    assert_eq!(v["result"]["inputs"]["target_points"], 22);

    // Ending at 15:00 shortens the day and the target (FR-047a).
    let v = s.ok(&["day", "end", "--now", "15:00"]);
    assert_eq!(v["result"]["mode"]["kind"], "ended");
    assert_eq!(v["result"]["end"], "15:00");
    s.err(&["day", "end", "--now", "15:10"], 4, "already_ended");
    s.err(&["day", "toggle", "--now", "15:10"], 4, "day_ended");
}

#[test]
fn trim_removes_blocks_before_a_time() {
    let s = Sandbox::new("trim");
    s.ok(&["day", "start", "--now", "01:00"]);
    s.err(&["day", "trim", "--from", "01:00", "--now", "10:00"], 4, "not_later");
    s.err(&["day", "trim", "--from", "09:10", "--now", "10:00"], 4, "off_grid");
    s.err(&["day", "trim", "--from", "soon", "--now", "10:00"], 2, "bad_time");
    let v = s.ok(&["day", "trim", "--from", "09:00", "--now", "10:00"]);
    assert_eq!(v["result"]["start"], "09:00");
}

#[test]
fn show_includes_every_block_and_the_record() {
    let s = Sandbox::new("show");
    s.ok(&["day", "start", "--now", "09:00"]);
    s.ok(&["day", "toggle", "--now", "10:05"]);
    s.ok(&["day", "toggle", "--now", "10:30"]);
    let v = s.ok(&["day", "show", "--now", "17:40"]);
    let blocks = v["result"]["record"]["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 26);
    assert_eq!(blocks[0]["start"], "09:00");
    assert_eq!(blocks[0]["kind"], "work");
    let lost: Vec<_> = blocks.iter().filter(|b| b["kind"] == "lost").collect();
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0]["uncategorised"], true);
    assert_eq!(lost[0]["category"], Value::Null);
    assert_eq!(v["result"]["record"]["downtime"][0]["start"], "10:05");
}

#[test]
fn list_and_delete() {
    let s = Sandbox::new("list");
    s.ok(&["day", "start", "--now", "09:00", "--date", "2026-11-02"]);
    // A past day is started at its default hours and, being past, is complete.
    s.ok(&["day", "start", "--date", "2026-10-30", "--now", "09:00"]);
    let v = s.ok(&["day", "list", "--now", "09:30"]);
    let days = v["result"]["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["date"], "2026-10-30");
    assert_eq!(days[0]["mode"], "ended");
    assert_eq!(days[1]["date"], "2026-11-02");
    assert_eq!(days[1]["mode"], "work");
    let v = s.ok(&["day", "list", "--from", "2026-11-01", "--now", "09:30"]);
    assert_eq!(v["result"]["days"].as_array().unwrap().len(), 1);

    s.err(&["day", "delete", "--date", "2026-10-30"], 2, "confirmation_required");
    let v = s.ok(&["day", "delete", "--date", "2026-10-30", "--yes"]);
    assert_eq!(v["result"]["deleted"], true);
    s.err(&["day", "delete", "--date", "2026-10-30", "--yes"], 3, "no_day");
    assert_eq!(s.ok(&["day", "list", "--now", "09:30"])["result"]["days"].as_array().unwrap().len(), 1);
}

#[test]
fn toggling_a_past_day_is_refused() {
    let s = Sandbox::new("past");
    s.ok(&["day", "start", "--date", "2026-10-30", "--now", "09:00"]);
    s.err(&["day", "toggle", "--date", "2026-10-30", "--now", "09:00"], 4, "not_today");
}

#[test]
fn the_same_script_gives_the_same_output() {
    let run_once = |name: &str| {
        let s = Sandbox::new(name);
        let mut out = String::new();
        for args in [
            &["day", "start", "--now", "09:00"][..],
            &["day", "toggle", "--now", "10:05"][..],
            &["day", "toggle", "--now", "10:30"][..],
            &["day", "status", "--now", "11:00"][..],
        ] {
            out.push_str(&s.go(args).stdout);
        }
        out
    };
    assert_eq!(run_once("det-a"), run_once("det-b"));
}

#[test]
fn a_write_is_refused_while_the_app_holds_the_lock_but_reads_and_force_work() {
    let s = Sandbox::new("lock");
    s.ok(&["day", "start", "--now", "09:00"]);
    // The test runner's parent process stands in for the running app.
    std::fs::write(s.dir.join(".lock"), std::os::unix::process::parent_id().to_string()).unwrap();
    s.err(&["day", "downtime", "start", "--now", "10:00"], 6, "app_running");
    s.err(&["block", "note", "--block", "09:00", "--note", "x", "--now", "10:00"], 6, "app_running");
    // Reads are always allowed.
    s.ok(&["day", "status", "--now", "10:00"]);
    s.ok(&["report", "day", "--now", "10:00"]);
    // --force writes anyway.
    s.ok(&["day", "downtime", "start", "--now", "10:00", "--force"]);
    // A lock left by a process that is gone does not block.
    std::fs::write(s.dir.join(".lock"), "4000000").unwrap();
    s.ok(&["day", "downtime", "stop", "--now", "10:30"]);
}
