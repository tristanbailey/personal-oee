//! `block …` commands (C-06), driven through `run` against a throwaway folder.

use std::path::PathBuf;

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use serde_json::Value;

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oee-cli-block-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }

    /// The system date is Mon 2 Nov 2026.
    fn go(&self, args: &[&str]) -> Outcome {
        let mut full = vec!["office-oee", "--data-dir", self.dir.to_str().unwrap()];
        full.extend_from_slice(args);
        run(full, &Env::default(), Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap())
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

    /// A day from 09:00 with one 25-minute stop (10:05–10:30): block 10:00 is lost.
    fn day_with_a_stop(&self) {
        self.ok(&["day", "start", "--now", "09:00"]);
        self.ok(&["day", "downtime", "start", "--now", "10:05"]);
        self.ok(&["day", "downtime", "stop", "--now", "10:30"]);
    }

    fn block(&self, start: &str) -> Value {
        let v = self.ok(&["block", "list", "--now", "11:00"]);
        v["result"]["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["start"] == start)
            .unwrap_or_else(|| panic!("no block {start}"))
            .clone()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn list_shows_every_block_with_its_state() {
    let s = Sandbox::new("list");
    s.day_with_a_stop();
    let v = s.ok(&["block", "list", "--now", "11:00"]);
    let blocks = v["result"]["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 26);
    assert_eq!(blocks[0]["start"], "09:00");
    assert_eq!(blocks[0]["kind"], "work");
    assert_eq!(blocks[0]["finished"], true);
    let lost = &blocks[3];
    assert_eq!((lost["start"].as_str(), lost["kind"].as_str()), (Some("10:00"), Some("lost")));
    assert_eq!(lost["uncategorised"], true);
    assert_eq!(blocks[25]["finished"], false);
}

#[test]
fn categorise_a_lost_block_and_see_it_leave_the_waiting_list() {
    let s = Sandbox::new("categorise");
    s.day_with_a_stop();
    let v = s.ok(&["block", "uncategorised", "--now", "11:00"]);
    assert_eq!(v["result"]["count"], 1);
    assert_eq!(v["result"]["blocks"][0]["start"], "10:00");

    let v = s.ok(&["block", "categorise", "--blocks", "10:00", "--category", "wait", "--now", "11:00"]);
    assert_eq!(v["result"]["saved"], true);
    assert_eq!(v["result"]["changed"][0]["category"], "wait");
    assert_eq!(v["result"]["uncategorised_blocks"], 0);
    assert_eq!(s.block("10:00")["category"], "wait");
    assert_eq!(s.ok(&["block", "uncategorised", "--now", "11:00"])["result"]["count"], 0);
}

#[test]
fn each_edit_error_has_its_own_code() {
    let s = Sandbox::new("errors");
    s.day_with_a_stop();
    // Work block: can't be categorised.
    s.err(&["block", "categorise", "--blocks", "09:00", "--category", "idle", "--now", "11:00"], 4, "not_lost_time");
    // Lost block: can't be Rework.
    s.err(&["block", "rework", "--blocks", "10:00", "--now", "11:00"], 4, "not_work");
    // Off the grid.
    s.err(&["block", "note", "--block", "10:05", "--text-not-a-flag", "x", "--now", "11:00"], 2, "usage");
    s.err(&["block", "set-type", "--block", "10:05", "--type", "work", "--now", "11:00"], 3, "no_such_block");
    s.err(&["block", "set-type", "--block", "10:00", "--type", "nap", "--now", "11:00"], 2, "bad_type");
    s.err(&["block", "categorise", "--blocks", "10:00", "--category", "lunch", "--now", "11:00"], 2, "bad_category");
    s.err(&["block", "list", "--date", "2026-10-01"], 3, "no_day");
}

#[test]
fn rework_marks_and_unmarks_work_blocks() {
    let s = Sandbox::new("rework");
    s.day_with_a_stop();
    let v = s.ok(&["block", "rework", "--blocks", "09:00..09:40", "--now", "11:00"]);
    assert_eq!(v["result"]["changed"].as_array().unwrap().len(), 3);
    assert_eq!(v["result"]["inputs"]["rework_points"], 3);
    assert_eq!(s.block("09:20")["rework"], true);
    let v = s.ok(&["block", "rework", "--blocks", "09:00,09:20", "--off", "--now", "11:00"]);
    assert_eq!(v["result"]["inputs"]["rework_points"], 1);
}

#[test]
fn a_strict_edit_changes_all_or_none() {
    let s = Sandbox::new("atomic");
    s.day_with_a_stop();
    // 09:00 is fine for Rework but 10:00 is lost: nothing may change.
    s.err(&["block", "rework", "--blocks", "09:00,10:00", "--now", "11:00"], 4, "not_work");
    assert_eq!(s.block("09:00")["rework"], false);
}

#[test]
fn set_type_and_reset_type_round_trip() {
    let s = Sandbox::new("type");
    s.day_with_a_stop();
    let before = s.ok(&["day", "status", "--now", "11:00"])["result"]["inputs"].clone();
    let v = s.ok(&["block", "set-type", "--block", "09:00", "--type", "meeting", "--now", "11:00"]);
    assert_eq!(v["result"]["changed"][0]["kind"], "planned");
    assert_eq!(v["result"]["changed"][0]["planned_kind"], "meeting");
    assert_eq!(v["result"]["inputs"]["planned_minutes"], 20);
    let v = s.ok(&["block", "reset-type", "--block", "09:00", "--now", "11:00"]);
    assert_eq!(v["result"]["changed"][0]["kind"], "work");
    assert_eq!(v["result"]["inputs"], before);
}

#[test]
fn a_note_is_set_and_cleared() {
    let s = Sandbox::new("note");
    s.day_with_a_stop();
    let v = s.ok(&["block", "note", "--block", "10:00", "--note", "waiting on the build", "--now", "11:00"]);
    assert_eq!(v["result"]["changed"][0]["note"], "waiting on the build");
    assert_eq!(s.block("10:00")["note"], "waiting on the build");
    s.ok(&["block", "note", "--block", "10:00", "--now", "11:00"]);
    assert_eq!(s.block("10:00")["note"], Value::Null);
}

#[test]
fn bulk_lists_what_it_skipped() {
    let s = Sandbox::new("bulk");
    s.day_with_a_stop();
    let v = s.ok(&["block", "bulk", "--blocks", "09:40..10:20", "--category", "idle", "--now", "11:00"]);
    // 09:40 and 10:20 are work; only 10:00 is lost.
    assert_eq!(v["result"]["applied"], 1);
    let skipped = v["result"]["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 2);
    assert!(skipped.iter().all(|x| x["code"] == "not_lost_time"));
    assert_eq!(s.block("10:00")["category"], "idle");

    let v = s.ok(&["block", "bulk", "--blocks", "09:00,09:20,10:05", "--rework", "on", "--now", "11:00"]);
    assert_eq!(v["result"]["applied"], 2);
    assert_eq!(v["result"]["skipped"][0]["code"], "no_such_block");

    s.err(&["block", "bulk", "--blocks", "09:00", "--now", "11:00"], 2, "bulk_needs_one_change");
    s.err(&["block", "bulk", "--blocks", "09:00", "--rework", "on", "--type", "work"], 2, "bulk_needs_one_change");
}

#[test]
fn a_past_day_can_be_back_filled() {
    let s = Sandbox::new("past");
    s.ok(&["day", "start", "--now", "09:00"]);
    // Next day: the earlier day is complete, so every block is finished.
    let v = s.ok(&["block", "set-type", "--block", "13:00", "--type", "lost", "--date", "2026-11-02", "--now", "2026-11-03T09:00"]);
    assert_eq!(v["date"], "2026-11-02");
    let v = s.ok(&["block", "categorise", "--blocks", "13:00", "--category", "management", "--date", "2026-11-02", "--now", "2026-11-03T09:00"]);
    assert_eq!(v["result"]["changed"][0]["category"], "management");
    let v = s.ok(&["block", "list", "--date", "2026-11-02", "--now", "2026-11-03T09:00"]);
    assert!(v["result"]["blocks"].as_array().unwrap().iter().all(|b| b["finished"] == true));
}

#[test]
fn dry_run_saves_nothing_and_writes_need_a_location() {
    let s = Sandbox::new("dry");
    s.day_with_a_stop();
    let v = s.ok(&["block", "categorise", "--blocks", "10:00", "--category", "wait", "--dry-run", "--now", "11:00"]);
    assert_eq!(v["result"]["saved"], false);
    assert_eq!(v["result"]["changed"][0]["category"], "wait");
    assert_eq!(s.block("10:00")["category"], Value::Null);

    let o = run(
        ["office-oee", "block", "note", "--block", "10:00", "--note", "x", "--now", "11:00"],
        &Env { home: Some(s.dir.clone()), ..Env::default() },
        Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap(),
    );
    assert_eq!(o.exit, 7);
}

#[test]
fn the_catalogue_describes_every_block_command() {
    let s = Sandbox::new("catalogue");
    let v = s.ok(&["commands"]);
    let names: Vec<&str> = v["result"]["commands"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    for n in [
        "block list", "block uncategorised", "block set-type", "block reset-type",
        "block categorise", "block rework", "block note", "block bulk",
    ] {
        assert!(names.contains(&n), "{n} missing");
        assert_eq!(s.ok(&["schema", n])["ok"], true, "{n} has no schema");
    }
}
