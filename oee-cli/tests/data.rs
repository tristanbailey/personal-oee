//! `data seed`, `data reset` and `check` (C-09).

use std::path::PathBuf;

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use serde_json::Value;

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oee-cli-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }
    /// The clock is Fri 6 Nov 2026 17:45, so the seeded week is 2–6 Nov, all past.
    fn go(&self, args: &[&str]) -> Outcome {
        let mut full = vec!["office-oee", "--data-dir", self.dir.to_str().unwrap(), "--now", "2026-11-06T17:45"];
        full.extend_from_slice(args);
        run(full, &Env::default(), Local.with_ymd_and_hms(2026, 11, 6, 8, 0, 0).unwrap())
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
    fn week(&self) -> Value {
        self.ok(&["report", "week"])["result"].clone()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn week4_seeds_five_days_and_the_week_report_reads_them() {
    let s = Sandbox::new("week4");
    let v = s.ok(&["data", "seed", "--scenario", "week4"]);
    assert_eq!(v["result"]["days"].as_array().unwrap().len(), 5);
    assert_eq!(v["result"]["saved"], true);
    let w = s.week();
    assert_eq!(w["days_counted"], 5);
    // Appendix A has planned times that aren't whole blocks (150 min), so the exact
    // 58.4% is proved by oee-core/tests/appendix_a.rs. Whole blocks give the same
    // points (74 of 100), Rework (3) and Quality (98.0%), and OEE a little lower.
    let p = &w["pooled"];
    assert_eq!(p["inputs"]["day_minutes"], 2600);
    assert_eq!((p["inputs"]["work_points"].as_u64(), p["inputs"]["target_points"].as_u64(), p["inputs"]["rework_points"].as_u64()), (Some(74), Some(100), Some(3)));
    assert_eq!(p["metrics"]["display"]["quality"], "98.0%");
    assert_eq!(p["metrics"]["display"]["efficiency"], "74.0%");
    assert_eq!(p["metrics"]["display"]["oee"], "57.6%");
}

#[test]
fn the_other_scenarios_have_the_shape_they_name() {
    let s = Sandbox::new("shapes");
    s.ok(&["data", "seed", "--scenario", "holiday-week"]);
    let w = s.week();
    assert_eq!(w["days_counted"], 4);
    let wed = w["days"].as_array().unwrap().iter().find(|d| d["weekday"] == "Wed" || d["date"] == "2026-11-04").unwrap().clone();
    assert_eq!(wed["status"], "holiday");

    s.ok(&["data", "seed", "--scenario", "zero-work-day", "--yes"]);
    let d = s.ok(&["report", "day", "--date", "2026-11-02"])["result"].clone();
    assert_eq!(d["metrics"]["status"], "zero-work");
    assert_eq!(d["metrics"]["oee"], 0.0);

    s.ok(&["data", "seed", "--scenario", "over-capacity", "--yes"]);
    let w = s.week();
    assert_eq!(w["over_capacity_days"].as_array().unwrap().len(), 1);

    s.ok(&["data", "seed", "--scenario", "uncategorised-day", "--yes"]);
    let v = s.ok(&["block", "uncategorised", "--date", "2026-11-02"]);
    assert_eq!(v["result"]["count"], 3);

    s.ok(&["data", "seed", "--scenario", "streak-3", "--yes"]);
    let g = s.ok(&["report", "goal"])["result"].clone();
    assert_eq!(g["goal"]["streak_weeks"], 3);

    s.ok(&["data", "seed", "--scenario", "empty", "--yes"]);
    assert_eq!(s.ok(&["day", "list"])["result"]["days"].as_array().unwrap().len(), 0);
}

#[test]
fn seeding_over_data_needs_yes_and_a_known_scenario() {
    let s = Sandbox::new("yes");
    s.ok(&["data", "seed", "--scenario", "holiday-week"]);
    s.err(&["data", "seed", "--scenario", "week4"], 4, "data_not_empty");
    s.err(&["data", "seed", "--scenario", "nope", "--yes"], 2, "bad_scenario");
    let v = s.ok(&["data", "seed", "--scenario", "week4", "--yes", "--dry-run"]);
    assert_eq!(v["result"]["saved"], false);
    assert_eq!(s.week()["days_counted"], 4);
}

#[test]
fn reset_needs_yes_and_clears_everything() {
    let s = Sandbox::new("reset");
    s.ok(&["data", "seed", "--scenario", "week4"]);
    s.err(&["data", "reset"], 2, "confirmation_required");
    let v = s.ok(&["data", "reset", "--yes"]);
    assert!(v["result"]["removed_files"].as_u64().unwrap() >= 6);
    assert_eq!(s.ok(&["day", "list"])["result"]["days"].as_array().unwrap().len(), 0);
}

#[test]
fn writes_need_a_location() {
    let o = run(
        ["office-oee", "data", "reset", "--yes"],
        &Env { home: Some(PathBuf::from("/nowhere")), ..Env::default() },
        Local.with_ymd_and_hms(2026, 11, 6, 8, 0, 0).unwrap(),
    );
    assert_eq!(o.exit, 7);
}

#[test]
fn check_passes_on_seeded_data_and_names_a_broken_file() {
    let s = Sandbox::new("check");
    s.ok(&["data", "seed", "--scenario", "streak-3"]);
    let v = s.ok(&["check"]);
    assert_eq!(v["result"]["healthy"], true);
    assert_eq!(v["result"]["days_checked"], 15);
    std::fs::write(s.dir.join("days/2026-11-02.json"), "{ nope").unwrap();
    let v = s.err(&["check"], 5, "check_failed");
    assert!(v["error"]["message"].as_str().unwrap().contains("days/2026-11-02.json"));
    std::fs::write(s.dir.join("settings.json"), "[").unwrap();
    s.err(&["check"], 5, "check_failed");
}

#[test]
fn the_catalogue_describes_these_commands() {
    let s = Sandbox::new("catalogue");
    for n in ["data seed", "data reset", "check"] {
        assert_eq!(s.ok(&["schema", n])["ok"], true, "{n}");
    }
}

#[test]
fn the_committed_example_week_is_healthy_and_matches_the_seed() {
    let committed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/week");
    let s = Sandbox::new("example");
    s.ok(&["data", "seed", "--scenario", "example-week"]);
    // Same files, same bytes: regenerate with the command in examples/README.md after changing the seed.
    let files = |root: &std::path::Path| {
        let mut v: Vec<String> = Vec::new();
        for entry in ["calendar.json", "settings.json"] {
            v.push(entry.into());
        }
        for e in std::fs::read_dir(root.join("days")).unwrap().flatten() {
            v.push(format!("days/{}", e.file_name().to_string_lossy()));
        }
        v.sort();
        v
    };
    assert_eq!(files(&committed), files(&s.dir));
    for f in files(&s.dir) {
        assert_eq!(
            std::fs::read_to_string(committed.join(&f)).unwrap(),
            std::fs::read_to_string(s.dir.join(&f)).unwrap(),
            "{f} is out of date"
        );
    }
    // And the app's own checks pass on the committed folder.
    let o = run(
        ["office-oee", "--data-dir", committed.to_str().unwrap(), "check"],
        &Env::default(),
        Local.with_ymd_and_hms(2026, 11, 6, 8, 0, 0).unwrap(),
    );
    assert_eq!(o.exit, 0, "{}", o.stdout);
}
