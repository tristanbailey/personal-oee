//! `calendar …` and `settings …` commands (C-07).

use std::path::PathBuf;

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use serde_json::Value;

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oee-cli-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }
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

#[test]
fn settings_start_with_the_defaults() {
    let s = Sandbox::new("defaults");
    let v = s.ok(&["settings", "get"]);
    let r = &v["result"];
    assert_eq!((r["start"].as_str(), r["end"].as_str(), r["target"].as_u64()), (Some("09:00"), Some("17:40"), Some(20)));
    assert_eq!(r["shortcut"]["label"], "Ctrl+Option+D");
    assert_eq!(r["goal"]["enabled"], true);
}

#[test]
fn settings_set_changes_only_what_is_given_and_persists() {
    let s = Sandbox::new("set");
    let v = s.ok(&["settings", "set", "--target", "24", "--goal", "70", "--start", "08:00"]);
    assert_eq!(v["result"]["saved"], true);
    let r = &s.ok(&["settings", "get"])["result"];
    assert_eq!((r["start"].as_str(), r["end"].as_str(), r["target"].as_u64()), (Some("08:00"), Some("17:40"), Some(24)));
    assert_eq!(r["goal"]["percent"], 70);
    s.ok(&["settings", "set", "--goal", "off", "--shortcut", "cmd+shift+k"]);
    let r = &s.ok(&["settings", "get"])["result"];
    assert_eq!(r["goal"]["enabled"], false);
    assert_eq!(r["goal"]["percent"], 70);
    assert_eq!(r["shortcut"]["label"], "Shift+Cmd+K");
}

#[test]
fn the_cycle_time_is_a_setting_and_must_divide_the_day() {
    let s = Sandbox::new("cycle");
    assert_eq!(s.ok(&["settings", "get"])["result"]["cycle_minutes"], 20);
    // 09:00–17:40 is 520 minutes; 30 does not divide it, so nothing changes.
    s.err(&["settings", "set", "--cycle", "30"], 4, "bad_setting");
    s.err(&["settings", "set", "--cycle", "3"], 4, "bad_setting");
    assert_eq!(s.ok(&["settings", "get"])["result"]["cycle_minutes"], 20);
    s.ok(&["settings", "set", "--cycle", "30", "--end", "18:00"]);
    assert_eq!(s.ok(&["settings", "get"])["result"]["cycle_minutes"], 30);
}

#[test]
fn bad_settings_are_refused_and_change_nothing() {
    let s = Sandbox::new("bad");
    s.err(&["settings", "set", "--target", "0"], 4, "bad_setting");
    s.err(&["settings", "set", "--target", "30", "--end", "09:10"], 4, "bad_setting");
    s.err(&["settings", "set", "--goal", "101"], 4, "bad_setting");
    s.err(&["settings", "set", "--start", "nine"], 4, "bad_setting");
    s.err(&["settings", "set", "--shortcut", "d"], 4, "bad_setting");
    s.err(&["settings", "set", "--shortcut", "hyper+d"], 2, "bad_shortcut");
    assert_eq!(s.ok(&["settings", "get"])["result"]["target"], 20);
}

#[test]
fn calendar_add_list_remove() {
    let s = Sandbox::new("calendar");
    let v = s.ok(&["calendar", "add", "--day", "mon-fri", "--from", "12:00", "--to", "13:00", "--kind", "break", "--label", "Lunch"]);
    assert_eq!(v["result"]["added"].as_array().unwrap().len(), 5);
    let v = s.ok(&["calendar", "add", "--day", "Wed", "--from", "09:30", "--to", "10:00", "--kind", "meeting"]);
    // On a 09:00 start this counts as 09:20–10:00.
    assert_eq!(v["result"]["added"][0]["counts_as"]["minutes"], 40);
    let list = s.ok(&["calendar", "list"]);
    let entries = list["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 6);
    assert_eq!(entries[0]["day"], "mon");
    let wed_meeting = entries.iter().find(|e| e["kind"] == "meeting").unwrap();
    let id = wed_meeting["id"].as_u64().unwrap().to_string();
    let v = s.ok(&["calendar", "remove", "--id", &id]);
    assert_eq!(v["result"]["removed"]["kind"], "meeting");
    assert_eq!(s.ok(&["calendar", "list"])["result"]["entries"].as_array().unwrap().len(), 5);
    s.err(&["calendar", "remove", "--id", &id], 3, "no_such_entry");
}

#[test]
fn a_started_day_takes_its_planned_window_from_the_calendar() {
    let s = Sandbox::new("planned");
    // 2 Nov 2026 is a Monday.
    s.ok(&["calendar", "add", "--day", "mon", "--from", "12:00", "--to", "13:00", "--kind", "break", "--label", "Lunch"]);
    s.ok(&["day", "start", "--now", "09:00"]);
    let v = s.ok(&["day", "status", "--now", "12:10"]);
    assert_eq!(v["result"]["mode"]["kind"], "planned");
}

#[test]
fn calendar_input_is_checked() {
    let s = Sandbox::new("calbad");
    s.err(&["calendar", "add", "--day", "funday", "--from", "12:00", "--to", "13:00", "--kind", "break"], 2, "bad_day");
    s.err(&["calendar", "add", "--day", "mon", "--from", "12:00", "--to", "13:00", "--kind", "nap"], 2, "bad_kind");
    s.err(&["calendar", "add", "--day", "mon", "--from", "13:00", "--to", "12:00", "--kind", "break"], 4, "bad_entry");
    assert_eq!(s.ok(&["calendar", "list"])["result"]["entries"].as_array().unwrap().len(), 0);
}

#[test]
fn dry_run_and_location_rules_apply() {
    let s = Sandbox::new("dry");
    let v = s.ok(&["calendar", "add", "--day", "mon", "--from", "12:00", "--to", "13:00", "--kind", "break", "--dry-run"]);
    assert_eq!(v["result"]["saved"], false);
    assert_eq!(s.ok(&["calendar", "list"])["result"]["entries"].as_array().unwrap().len(), 0);
    let v = s.ok(&["settings", "set", "--target", "30", "--dry-run"]);
    assert_eq!(v["result"]["saved"], false);
    assert_eq!(s.ok(&["settings", "get"])["result"]["target"], 20);
    let o = run(
        ["office-oee", "settings", "set", "--target", "30"],
        &Env { home: Some(s.dir.clone()), ..Env::default() },
        Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap(),
    );
    assert_eq!(o.exit, 7);
}

#[test]
fn the_catalogue_describes_these_commands() {
    let s = Sandbox::new("catalogue");
    for n in ["calendar list", "calendar add", "calendar remove", "settings get", "settings set"] {
        assert_eq!(s.ok(&["schema", n])["ok"], true, "{n}");
    }
}
