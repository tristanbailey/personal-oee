//! `sim run` (C-10): scripted days, including the PRD worked examples.

use std::path::PathBuf;

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use serde_json::Value;

fn sim(file: &str) -> Outcome {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    run(
        ["office-oee", "sim", "run", path.to_str().unwrap()],
        &Env::default(),
        Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap(),
    )
}

fn passes(file: &str) -> Value {
    let o = sim(file);
    assert_eq!(o.exit, 0, "{file}: {}", o.stdout);
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
    assert_eq!(v["result"]["passed"], true);
    v
}

fn scenario(name: &str, json: &str) -> Outcome {
    let path = std::env::temp_dir().join(format!("oee-sim-test-{name}-{}.json", std::process::id()));
    std::fs::write(&path, json).unwrap();
    let o = run(
        ["office-oee", "sim", "run", path.to_str().unwrap()],
        &Env::default(),
        Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap(),
    );
    let _ = std::fs::remove_file(&path);
    o
}

fn error_code(o: &Outcome, exit: i32) -> String {
    assert_eq!(o.exit, exit, "{}", o.stdout);
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
    v["error"]["code"].as_str().unwrap().to_string()
}

#[test]
fn prd_availability_example_is_91_3_percent() {
    let v = passes("scenarios/prd-availability.json");
    assert_eq!(v["result"]["final"]["metrics"]["display"]["availability"], "91.3%");
}

#[test]
fn prd_quality_example_is_92_9_percent() {
    let v = passes("scenarios/prd-quality.json");
    assert_eq!(v["result"]["final"]["metrics"]["display"]["quality"], "92.9%");
}

#[test]
fn prd_oee_example_is_59_3_percent() {
    let v = passes("scenarios/prd-oee.json");
    assert_eq!(v["result"]["final"]["metrics"]["display"]["oee"], "59.3%");
    assert_eq!(v["result"]["steps"].as_array().unwrap().len(), 7);
    assert_eq!(v["result"]["steps"][4]["result"]["mode"]["kind"], "ended");
}

#[test]
fn a_failed_expectation_names_the_key_and_both_values() {
    let o = scenario(
        "expect",
        r#"{ "date": "2026-11-02", "steps": [ { "at": "09:00", "do": "day.start" }, { "at": "17:40", "do": "day.status" } ],
             "expect": { "work_points": 25, "lost_blocks": 0 } }"#,
    );
    assert_eq!(error_code(&o, 4), "expect_failed");
    assert!(o.stdout.contains("work_points: expected 25, got 26"), "{}", o.stdout);
}

#[test]
fn a_step_can_be_expected_to_fail() {
    let o = scenario(
        "expect-error",
        r#"{ "date": "2026-11-02", "steps": [
              { "at": "09:00", "do": "day.start" },
              { "at": "09:30", "do": "day.start", "expect_error": "day_exists" },
              { "at": "10:00", "do": "block.rework", "blocks": "09:00" } ],
             "expect": { "rework_points": 1 } }"#,
    );
    assert_eq!(o.exit, 0, "{}", o.stdout);
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
    assert_eq!(v["result"]["steps"][1]["ok"], false);
    assert_eq!(v["result"]["steps"][1]["error"]["code"], "day_exists");
}

#[test]
fn an_unexpected_step_failure_stops_the_run_with_its_exit_code() {
    let o = scenario("fail", r#"{ "date": "2026-11-02", "steps": [ { "at": "09:00", "do": "day.status" } ] }"#);
    assert_eq!(error_code(&o, 3), "step_failed");
    assert!(o.stdout.contains("step 1"));
}

#[test]
fn bad_files_and_steps_are_usage_errors() {
    assert_eq!(error_code(&sim("scenarios/nope.json"), 3), "no_scenario");
    assert_eq!(error_code(&scenario("junk", "{ not json"), 2), "bad_scenario_file");
    assert_eq!(error_code(&scenario("group", r#"{ "steps": [ { "do": "report.week" } ] }"#), 2), "bad_step");
    assert_eq!(error_code(&scenario("when", r#"{ "steps": [ { "at": "9am", "do": "day.start" } ] }"#), 2), "bad_step");
    assert_eq!(
        error_code(&scenario("key", r#"{ "date": "2026-11-02", "steps": [ { "at": "09:00", "do": "day.start" } ], "expect": { "luck": 1 } }"#), 2),
        "bad_expect"
    );
}

#[test]
fn a_run_touches_no_real_or_test_folder() {
    // No data folder is given and none may be needed: the run uses a private one.
    let o = run(
        ["office-oee", "sim", "run", PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios/prd-quality.json").to_str().unwrap()],
        &Env { home: None, ..Env::default() },
        Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap(),
    );
    assert_eq!(o.exit, 0, "{}", o.stdout);
}

#[test]
fn the_catalogue_describes_sim_run() {
    let o = run(["office-oee", "schema", "sim run"], &Env::default(), Local.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap());
    assert_eq!(o.exit, 0);
}
