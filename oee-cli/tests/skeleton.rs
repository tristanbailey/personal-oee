//! The CLI skeleton (C-04): envelope, exit codes, self-description, text mode.

use chrono::{Local, TimeZone};
use oee_cli::{run, Env, Outcome};
use serde_json::Value;

fn go(args: &[&str]) -> Outcome {
    go_env(args, &Env::default())
}

fn go_env(args: &[&str], env: &Env) -> Outcome {
    let system = Local.with_ymd_and_hms(2026, 11, 1, 15, 12, 40).unwrap();
    let mut full = vec!["office-oee"];
    full.extend_from_slice(args);
    run(full, env, system)
}

fn json(o: &Outcome) -> Value {
    serde_json::from_str(o.stdout.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {}", o.stdout))
}

#[test]
fn version_is_a_success_envelope_stamped_with_the_clock() {
    let o = go(&["version"]);
    assert_eq!(o.exit, 0);
    let v = json(&o);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "version");
    assert_eq!(v["date"], "2026-11-01");
    assert_eq!(v["now"], "15:12");
    assert!(v["result"]["cli"].is_string());
    assert_eq!(v["notices"], Value::Array(vec![]));
    assert_eq!(o.stdout.lines().count(), 1, "one line of JSON");
}

#[test]
fn now_and_date_flags_set_the_stamp() {
    let v = json(&go(&["version", "--now", "2026-11-02T10:25"]));
    assert_eq!((v["date"].as_str(), v["now"].as_str()), (Some("2026-11-02"), Some("10:25")));
    let v = json(&go(&["--now", "10:25", "--date", "2026-10-26", "version"]));
    assert_eq!((v["date"].as_str(), v["now"].as_str()), (Some("2026-10-26"), Some("10:25")));
}

#[test]
fn oee_now_is_read_from_the_environment_and_the_flag_beats_it() {
    let env = Env { oee_now: Some("08:00".into()), ..Env::default() };
    assert_eq!(json(&go_env(&["version"], &env))["now"], "08:00");
    assert_eq!(json(&go_env(&["version", "--now", "09:30"], &env))["now"], "09:30");
}

#[test]
fn the_same_command_gives_byte_identical_output() {
    let a = go(&["commands", "--now", "10:25"]);
    let b = go(&["commands", "--now", "10:25"]);
    assert_eq!(a, b);
}

#[test]
fn a_bad_time_or_date_is_a_usage_error_with_a_code() {
    for (args, code) in [
        (&["version", "--now", "25:00"][..], "bad_time"),
        (&["version", "--date", "yesterday"][..], "bad_date"),
    ] {
        let o = go(args);
        assert_eq!(o.exit, 2, "{args:?}");
        let v = json(&o);
        assert_eq!((v["ok"].as_bool(), v["error"]["code"].as_str()), (Some(false), Some(code)));
        assert!(v["error"]["message"].as_str().unwrap().contains("Use "));
    }
}

#[test]
fn an_unknown_command_or_flag_is_a_usage_error_in_json() {
    for args in [&["nonsense"][..], &["version", "--bogus"][..], &[][..]] {
        let o = go(args);
        if args.is_empty() {
            // No command: clap shows help, which is not an error envelope.
            continue;
        }
        assert_eq!(o.exit, 2, "{args:?}");
        let v = json(&o);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "usage");
        assert!(v["error"]["hint"].as_str().unwrap().contains("office-oee commands"));
    }
}

#[test]
fn commands_lists_commands_flags_and_all_eight_exit_codes() {
    let v = json(&go(&["commands"]));
    let names: Vec<_> = v["result"]["commands"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(&names[..3], ["version", "commands", "schema"]);
    assert!(names.contains(&"day start") && names.contains(&"day delete"));
    let codes: Vec<_> = v["result"]["exit_codes"].as_array().unwrap().iter().map(|c| c["code"].as_i64().unwrap()).collect();
    assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6, 7]);
    let flags: Vec<_> = v["result"]["global_flags"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
    for want in ["--data-dir <path>", "--real", "--now <when>", "--text", "--dry-run", "--yes"] {
        assert!(flags.iter().any(|f| f == want), "{want}");
    }
}

#[test]
fn every_listed_command_has_a_schema() {
    let v = json(&go(&["commands"]));
    for c in v["result"]["commands"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let o = go(&["schema", name]);
        assert_eq!(o.exit, 0, "{name}");
        assert_eq!(json(&o)["result"]["type"], "object", "{name}");
    }
}

#[test]
fn schema_of_an_unknown_command_is_not_found() {
    let o = go(&["schema", "teleport"]);
    assert_eq!(o.exit, 3);
    assert_eq!(json(&o)["error"]["code"], "no_such_command");
}

#[test]
fn text_mode_prints_a_short_summary_not_json() {
    let o = go(&["version", "--text"]);
    assert_eq!(o.exit, 0);
    assert!(o.stdout.starts_with("cli: "), "{}", o.stdout);
    assert!(serde_json::from_str::<Value>(o.stdout.trim()).is_err());
    let e = go(&["schema", "teleport", "--text"]);
    assert_eq!(e.exit, 3);
    assert!(e.stdout.starts_with("error: "));
    assert!(e.stdout.contains("hint: "));
}

#[test]
fn help_is_plain_text_and_succeeds() {
    let o = go(&["--help"]);
    assert_eq!(o.exit, 0);
    assert!(o.stdout.contains("office-oee"));
}
