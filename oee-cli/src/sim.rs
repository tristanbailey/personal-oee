//! `sim run <scenario.json>`: replay a scripted day in a private folder and
//! check the result (docs/CLI_API.md 5.6). Each step runs the real command,
//! so a scenario proves what the commands do, not a copy of their logic.
//!
//! A step is `{ "at": "10:05", "do": "downtime.start" }`. `do` is
//! `group.name` and every other key becomes a flag of that command, for
//! example `{ "do": "block.categorise", "blocks": "10:00", "category": "wait" }`.
//! A step without `at` reuses the previous time. `expect_error` names the
//! error code a step must fail with.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Local};
use serde_json::{json, Map, Value};

use crate::output::{CliError, Exit, Reply};
use crate::Env;

#[derive(clap::Subcommand, Debug)]
pub enum SimCommand {
    /// Replay a scenario file and check its `expect` block.
    Run {
        /// A scenario file; see docs/CLI_API.md 5.6.
        file: PathBuf,
    },
}

const DEFAULT_TOLERANCE: f64 = 0.0005;
const GROUPS: [&str; 3] = ["day", "downtime", "block"];

pub fn run(path: &Path, default_date: chrono::NaiveDate, system: DateTime<Local>) -> Result<Reply, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::new(Exit::NotFound, "no_scenario", format!("Couldn't read {}: {e}", path.display()))
    })?;
    let scenario: Value = serde_json::from_str(&text)
        .map_err(|e| CliError::usage("bad_scenario_file", format!("{} is not valid JSON: {e}", path.display())))?;
    let sandbox = Sandbox::new();
    let result = play(&scenario, &sandbox, default_date, system);
    drop(sandbox);
    result
}

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("oee-sim-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        Self { dir }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One command, run for real against the sandbox. Returns its parsed envelope.
struct Runner<'a> {
    sandbox: &'a Sandbox,
    date: String,
    system: DateTime<Local>,
}

impl Runner<'_> {
    fn call(&self, at: &str, args: &[String]) -> Result<Value, (Exit, Value)> {
        let mut full: Vec<String> = vec![
            "office-oee".into(),
            "--data-dir".into(),
            self.sandbox.dir.display().to_string(),
            "--date".into(),
            self.date.clone(),
            "--now".into(),
            format!("{}T{at}", self.date),
        ];
        full.extend(args.iter().cloned());
        let out = crate::run(full, &Env::default(), self.system);
        let v: Value = serde_json::from_str(out.stdout.trim()).unwrap_or(Value::Null);
        if out.exit == 0 {
            Ok(v)
        } else {
            let exit = Exit::ALL.into_iter().find(|e| e.code() == out.exit).unwrap_or(Exit::Other);
            Err((exit, v["error"].clone()))
        }
    }
}

fn bad(code: &str, message: impl Into<String>) -> CliError {
    CliError::usage(code, message)
}

fn time_of(value: &Value) -> Result<String, CliError> {
    value
        .as_str()
        .filter(|t| oee_store::start::parse_hhmm(t).is_some())
        .map(str::to_string)
        .ok_or_else(|| bad("bad_step", format!("\"at\" must be HH:MM, got {value}")))
}

fn flag_args(step: &Map<String, Value>) -> Result<Vec<String>, CliError> {
    let mut out = Vec::new();
    for (k, v) in step {
        if matches!(k.as_str(), "do" | "at" | "expect_error") {
            continue;
        }
        let flag = format!("--{}", k.replace('_', "-"));
        match v {
            Value::Bool(true) => out.push(flag),
            Value::Bool(false) | Value::Null => {}
            Value::String(s) => out.extend([flag, s.clone()]),
            Value::Number(n) => out.extend([flag, n.to_string()]),
            _ => return Err(bad("bad_step", format!("\"{k}\" must be text, a number or true/false"))),
        }
    }
    Ok(out)
}

fn command_args(do_: &str) -> Result<Vec<String>, CliError> {
    let (group, name) = do_
        .split_once('.')
        .ok_or_else(|| bad("bad_step", format!("\"do\" must look like day.start, got \"{do_}\"")))?;
    if !GROUPS.contains(&group) {
        return Err(bad("bad_step", format!("Unknown step \"{do_}\". Groups: day, downtime, block.")));
    }
    Ok(if group == "downtime" {
        vec!["day".into(), "downtime".into(), name.into()]
    } else {
        vec![group.into(), name.replace('_', "-")]
    })
}

fn play(scenario: &Value, sandbox: &Sandbox, default_date: chrono::NaiveDate, system: DateTime<Local>) -> Result<Reply, CliError> {
    let date = scenario["date"].as_str().map_or_else(|| default_date.to_string(), str::to_string);
    let runner = Runner { sandbox, date: date.clone(), system };
    let setup = |at: &str, args: Vec<&str>| -> Result<(), CliError> {
        let args: Vec<String> = args.into_iter().map(String::from).collect();
        runner.call(at, &args).map(|_| ()).map_err(|(exit, e)| {
            CliError::new(exit, "bad_setup", format!("{}", e["message"].as_str().unwrap_or("setup failed")))
        })
    };

    // Settings and calendar.
    if let Some(s) = scenario["settings"].as_object() {
        let mut args = vec!["settings", "set"];
        let mut owned = Vec::new();
        for (k, flag) in [("start", "--start"), ("end", "--end"), ("target", "--target"), ("goal", "--goal")] {
            if let Some(v) = s.get(k) {
                owned.push((flag, v.as_str().map_or_else(|| v.to_string(), str::to_string)));
            }
        }
        for (f, v) in &owned {
            args.push(f);
            args.push(v);
        }
        setup("00:00", args)?;
    }
    for entry in scenario["calendar"].as_array().into_iter().flatten() {
        let get = |k: &str| entry[k].as_str().unwrap_or("").to_string();
        let (day, from, to, kind, label) = (get("day"), get("from"), get("to"), get("kind"), get("label"));
        setup(
            "00:00",
            vec!["calendar", "add", "--day", &day, "--from", &from, "--to", &to, "--kind", &kind, "--label", &label],
        )?;
    }

    // Steps.
    let steps = scenario["steps"].as_array().ok_or_else(|| bad("bad_scenario_file", "\"steps\" must be a list"))?;
    let mut at = scenario["settings"]["start"].as_str().unwrap_or("09:00").to_string();
    let mut log = Vec::new();
    for (i, raw) in steps.iter().enumerate() {
        let n = i + 1;
        let step = raw.as_object().ok_or_else(|| bad("bad_step", format!("step {n} must be an object")))?;
        if let Some(t) = step.get("at") {
            at = time_of(t)?;
        }
        let do_ = step.get("do").and_then(Value::as_str).ok_or_else(|| bad("bad_step", format!("step {n} has no \"do\"")))?;
        let mut args = command_args(do_)?;
        args.extend(flag_args(step)?);
        let want_error = step.get("expect_error").and_then(Value::as_str);
        match (runner.call(&at, &args), want_error) {
            (Ok(v), None) => log.push(json!({ "step": n, "do": do_, "at": at, "ok": true, "result": v["result"], "notices": v["notices"] })),
            (Err((_, e)), Some(code)) if e["code"] == code => {
                log.push(json!({ "step": n, "do": do_, "at": at, "ok": false, "error": e }));
            }
            (Err((exit, e)), _) => {
                return Err(CliError::new(
                    exit,
                    "step_failed",
                    format!(
                        "step {n} ({do_} at {at}) failed with {}: {}",
                        e["code"].as_str().unwrap_or("?"),
                        e["message"].as_str().unwrap_or("")
                    ),
                ));
            }
            (Ok(_), Some(code)) => {
                return Err(CliError::new(Exit::Rule, "step_should_fail", format!("step {n} ({do_}) was expected to fail with {code} but worked")));
            }
        }
    }

    // Final state: the day's report plus lost and uncategorised block counts.
    let fin = |args: &[&str]| -> Result<Value, CliError> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        runner.call(&at, &args).map(|v| v["result"].clone()).map_err(|(exit, e)| {
            CliError::new(exit, "final_failed", e["message"].as_str().unwrap_or("couldn't read the final state").to_string())
        })
    };
    let mut final_ = fin(&["report", "day"])?;
    let blocks = fin(&["block", "list"])?;
    let lost = blocks["blocks"].as_array().into_iter().flatten().filter(|b| b["kind"] == "lost").count();
    let uncategorised = blocks["blocks"].as_array().into_iter().flatten().filter(|b| b["uncategorised"] == true).count();
    final_["lost_blocks"] = json!(lost);
    final_["uncategorised_blocks"] = json!(uncategorised);

    let (expect, failures) = check(&scenario["expect"], &final_)?;
    if !failures.is_empty() {
        return Err(CliError::new(Exit::Rule, "expect_failed", failures.join("; "))
            .hint("`sim run` prints every actual value on success; use a looser tolerance or fix the scenario."));
    }
    Ok(json!({ "date": date, "steps": log, "final": final_, "expect": expect, "passed": true }).into())
}

/// Compare `expect` with the final state. Returns per-key detail and the failures.
fn check(expect: &Value, fin: &Value) -> Result<(Value, Vec<String>), CliError> {
    let Some(map) = expect.as_object() else {
        return Ok((json!({}), vec![]));
    };
    let tolerance = map.get("tolerance").and_then(Value::as_f64).unwrap_or(DEFAULT_TOLERANCE);
    let mut detail = Map::new();
    let mut failures = Vec::new();
    for (key, want) in map {
        if key == "tolerance" {
            continue;
        }
        let (actual, approx) = match key.as_str() {
            "oee" | "availability" | "efficiency" | "quality" => (fin["metrics"][key].clone(), true),
            "over_capacity" => (fin["metrics"]["over_capacity"].clone(), false),
            "status" => (fin["metrics"]["status"].clone(), false),
            "lost_blocks" | "uncategorised_blocks" => (fin[key].clone(), false),
            "work_points" | "rework_points" | "target_points" | "planned_minutes" | "downtime_minutes" | "day_minutes" => {
                (fin["inputs"][key].clone(), false)
            }
            _ => return Err(bad("bad_expect", format!("Unknown expect key \"{key}\"."))),
        };
        let pass = if approx {
            matches!((want.as_f64(), actual.as_f64()), (Some(w), Some(a)) if (w - a).abs() <= tolerance)
        } else {
            *want == actual
        };
        if !pass {
            failures.push(format!("{key}: expected {want}, got {actual}"));
        }
        detail.insert(key.clone(), json!({ "expected": want, "actual": actual, "pass": pass }));
    }
    Ok((Value::Object(detail), failures))
}
