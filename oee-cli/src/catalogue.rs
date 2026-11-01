//! The self-description an agent reads: every command, flag and result shape
//! (`office-oee commands`, `office-oee schema <command>`).
//!
//! Add each new command here when it is built, so the list is never ahead of
//! what works.

use serde_json::{json, Value};

use crate::output::exit_code_list;

/// One command: its name as typed, what it does, whether it writes.
pub struct Command {
    pub name: &'static str,
    pub summary: &'static str,
    pub writes: bool,
    /// Flags and arguments specific to this command.
    pub args: &'static [(&'static str, &'static str)],
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "version",
        summary: "CLI and engine versions",
        writes: false,
        args: &[],
    },
    Command {
        name: "commands",
        summary: "List every command, flag and exit code as JSON",
        writes: false,
        args: &[],
    },
    Command {
        name: "schema",
        summary: "JSON Schema of one command's result",
        writes: false,
        args: &[("<command>", "name as listed by `commands`")],
    },
    Command {
        name: "day start",
        summary: "Start the day; planned windows come from the weekly calendar",
        writes: true,
        args: &[("--at <HH:MM>", "start time, not later than now (default: now)")],
    },
    Command {
        name: "day status",
        summary: "Mode, current block, time left, metrics so far",
        writes: false,
        args: &[],
    },
    Command {
        name: "day toggle",
        summary: "Work becomes Downtime and back; refused in Planned or after the day ends",
        writes: true,
        args: &[],
    },
    Command {
        name: "day downtime start",
        summary: "Start downtime; refused if already in downtime",
        writes: true,
        args: &[],
    },
    Command {
        name: "day downtime stop",
        summary: "Stop downtime and report lost blocks; refused if none is running",
        writes: true,
        args: &[],
    },
    Command {
        name: "day extend",
        summary: "Move the finish later; reopens an ended day",
        writes: true,
        args: &[("--to <HH:MM>", "new finish time, later than the current one")],
    },
    Command {
        name: "day end",
        summary: "End the day now (lowers the target)",
        writes: true,
        args: &[],
    },
    Command {
        name: "day finish",
        summary: "Finalise: uncategorised lost blocks become System Down",
        writes: true,
        args: &[],
    },
    Command {
        name: "day show",
        summary: "The whole record: planned windows, stops, every block, metrics",
        writes: false,
        args: &[],
    },
    Command {
        name: "day list",
        summary: "Saved days with status and OEE",
        writes: false,
        args: &[("--from <DATE>", "first date"), ("--to <DATE>", "last date")],
    },
    Command {
        name: "day delete",
        summary: "Remove a saved day",
        writes: true,
        args: &[("--yes", "required to confirm")],
    },
];

pub const GLOBAL_FLAGS: &[(&str, &str)] = &[
    ("--data-dir <path>", "test data folder (env OEE_DATA_DIR); required for writes unless --real"),
    ("--real", "use the app's real data folder; required for writes without --data-dir"),
    ("--now <when>", "the clock: HH:MM or YYYY-MM-DDTHH:MM (env OEE_NOW)"),
    ("--date <YYYY-MM-DD>", "the day to work on; defaults to the date of --now"),
    ("--text", "short human summary instead of JSON"),
    ("--dry-run", "run a write command fully, print the result, save nothing"),
    ("--yes", "confirm a destructive command"),
];

pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.name == name)
}

pub fn commands_result() -> Value {
    let commands: Vec<Value> = COMMANDS
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "summary": c.summary,
                "writes": c.writes,
                "args": c.args.iter().map(|(n, d)| json!({ "name": n, "description": d })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "commands": commands,
        "global_flags": GLOBAL_FLAGS.iter().map(|(n, d)| json!({ "name": n, "description": d })).collect::<Vec<_>>(),
        "exit_codes": exit_code_list(),
    })
}

/// The JSON Schema of `result` for a command, or `None` if it has no such command.
pub fn schema_for(name: &str) -> Option<Value> {
    let result = match name {
        "version" => object(&[
            ("cli", string()),
            ("engine", string()),
        ]),
        "commands" => object(&[
            ("commands", array_of(object(&[
                ("name", string()),
                ("summary", string()),
                ("writes", json!({ "type": "boolean" })),
                ("args", array_of(object(&[("name", string()), ("description", string())]))),
            ]))),
            ("global_flags", array_of(object(&[("name", string()), ("description", string())]))),
            ("exit_codes", array_of(object(&[("code", json!({ "type": "integer" })), ("meaning", string())]))),
        ]),
        "schema" => json!({ "type": "object", "description": "A JSON Schema document" }),
        "day start" | "day status" | "day toggle" | "day downtime start" | "day downtime stop"
        | "day extend" | "day end" | "day finish" | "day show" => day_state_schema(name),
        "day list" => object(&[("days", array_of(object(&[
            ("date", string()),
            ("start", string()),
            ("end", string()),
            ("finalised", json!({ "type": "boolean" })),
            ("mode", string()),
            ("status", string()),
            ("oee", json!({ "type": "number" })),
            ("oee_display", string()),
        ])))]),
        "day delete" => object(&[
            ("date", string()),
            ("deleted", json!({ "type": "boolean" })),
            ("saved", json!({ "type": "boolean" })),
        ]),
        _ => return None,
    };
    Some(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("office-oee {name}: result"),
        "description": "The `result` field of the success envelope {ok, command, date, now, result, notices}.",
        "type": "object",
        "properties": result["properties"],
        "required": result["required"],
    }))
}

fn string() -> Value {
    json!({ "type": "string" })
}

fn array_of(items: Value) -> Value {
    json!({ "type": "array", "items": items })
}

fn object(fields: &[(&str, Value)]) -> Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (k, v) in fields {
        props.insert((*k).to_string(), v.clone());
        required.push(json!(k));
    }
    json!({ "type": "object", "properties": props, "required": required })
}

/// The state of a day, as returned by `day status` and every write that changes it.
fn day_state_schema(name: &str) -> Value {
    let mut fields = vec![
        ("date", string()),
        ("mode", json!({ "type": "object", "description": "kind: work | downtime (since) | planned (planned_kind, until) | ended" })),
        ("start", string()),
        ("end", string()),
        ("finalised", json!({ "type": "boolean" })),
        ("block", json!({ "type": ["object", "null"], "description": "number, of, start, end, minutes_left; null when the day is not running" })),
        ("uncategorised_blocks", json!({ "type": "integer" })),
        ("metrics", json!({ "type": "object", "description": "availability, efficiency, quality, oee (1.0 = 100%), over_capacity, status, display" })),
        ("inputs", json!({ "type": "object", "description": "day_minutes, planned_minutes, downtime_minutes, work_points, rework_points, target_points, losses" })),
    ];
    if !matches!(name, "day status" | "day show") {
        fields.push(("saved", json!({ "type": "boolean", "description": "false with --dry-run" })));
    }
    if name.starts_with("day downtime") || name == "day toggle" {
        fields.push(("stopped", json!({ "type": ["object", "null"], "description": "minutes, lost_blocks, message; set when this call ended a stop" })));
    }
    if name == "day finish" {
        fields.push(("defaulted_to_system_down", json!({ "type": "integer" })));
    }
    if name == "day show" {
        fields.push(("record", json!({ "type": "object", "description": "start, end, finalised, planned, downtime, blocks" })));
    }
    object(&fields)
}
