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
