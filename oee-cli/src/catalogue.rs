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
        summary: "CLI, engine and data-file format versions",
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
    Command {
        name: "block list",
        summary: "Every block: start, end, kind, category, rework, note, uncategorised, finished",
        writes: false,
        args: &[],
    },
    Command {
        name: "block uncategorised",
        summary: "Lost blocks still waiting for a category",
        writes: false,
        args: &[],
    },
    Command {
        name: "block set-type",
        summary: "Turn a block into work, lost time, a break or a meeting",
        writes: true,
        args: &[("--block <HH:MM>", "the block's start"), ("--type <work|lost|break|meeting>", "new type")],
    },
    Command {
        name: "block reset-type",
        summary: "Go back to what the day's events and calendar make the block",
        writes: true,
        args: &[("--block <HH:MM>", "the block's start")],
    },
    Command {
        name: "block categorise",
        summary: "Categorise lost blocks; all or none are changed",
        writes: true,
        args: &[
            ("--blocks <T,..|T..T>", "start times, a comma list or a range"),
            ("--category <system-down|change-over|idle|management|wait>", "the category"),
        ],
    },
    Command {
        name: "block rework",
        summary: "Mark Work blocks as Rework (--off unmarks); all or none are changed",
        writes: true,
        args: &[("--blocks <T,..|T..T>", "start times"), ("--off", "unmark instead")],
    },
    Command {
        name: "block note",
        summary: "Set a block's note; no --note clears it",
        writes: true,
        args: &[("--block <HH:MM>", "the block's start"), ("--note <TEXT>", "the note")],
    },
    Command {
        name: "block bulk",
        summary: "One change to many blocks; blocks that can't take it are listed as skipped",
        writes: true,
        args: &[
            ("--blocks <T,..|T..T>", "start times"),
            ("--type <work|lost|break|meeting>", "or"),
            ("--category <name>", "or"),
            ("--rework <on|off>", "exactly one of the three"),
        ],
    },
    Command {
        name: "calendar list",
        summary: "Weekly breaks and meetings with ids; off-grid ones say how they count",
        writes: false,
        args: &[],
    },
    Command {
        name: "calendar add",
        summary: "Add a weekly break or meeting (applies to days started afterwards)",
        writes: true,
        args: &[
            ("--day <mon..sun|mon-fri>", "which day(s)"),
            ("--from <HH:MM> --to <HH:MM>", "the window"),
            ("--kind <break|meeting>", "type"),
            ("--label <NAME>", "optional, at most 40 characters"),
        ],
    },
    Command {
        name: "calendar remove",
        summary: "Remove an entry by id",
        writes: true,
        args: &[("--id <N>", "from `calendar list`")],
    },
    Command {
        name: "settings get",
        summary: "Day start, finish, daily target, shortcut and weekly goal",
        writes: false,
        args: &[],
    },
    Command {
        name: "settings set",
        summary: "Change settings with the Settings tab's checks; nothing changes on error",
        writes: true,
        args: &[
            ("--start <HH:MM> --end <HH:MM>", "default day hours"),
            ("--target <N>", "daily target, 1 to 100"),
            ("--goal <N|off>", "weekly OEE goal percent, or off"),
            ("--shortcut <KEYS>", "for example ctrl+alt+D"),
        ],
    },
    Command {
        name: "sim run",
        summary: "Replay a scripted day (settings, calendar, steps, expect) in a private folder; touches no real data",
        writes: false,
        args: &[("<file>", "scenario JSON; steps are {at, do: group.name, flags…}; see docs/CLI_API.md 5.6")],
    },
    Command {
        name: "data seed",
        summary: "Replace the data folder with a known dataset (needs --yes if it holds data)",
        writes: true,
        args: &[("--scenario <NAME>", "empty, week4, holiday-week, zero-work-day, over-capacity, uncategorised-day or streak-3")],
    },
    Command {
        name: "data reset",
        summary: "Delete every saved day, the calendar, settings and UI state",
        writes: true,
        args: &[("--yes", "required to confirm")],
    },
    Command {
        name: "check",
        summary: "Read every file and test the invariants; exit 5 with the findings if any fail",
        writes: false,
        args: &[],
    },
    Command {
        name: "report day",
        summary: "Availability, Efficiency, Quality, OEE for one day; no record is a holiday",
        writes: false,
        args: &[],
    },
    Command {
        name: "report week",
        summary: "The Week tab: per-day rows, pooled week, over-capacity days, goal",
        writes: false,
        args: &[("--week-of <DATE>", "any date in the week (default: week of --date)")],
    },
    Command {
        name: "report trends",
        summary: "One pooled point per working week",
        writes: false,
        args: &[("--weeks <4|12|26>", "weeks ending with the current one (default 12)")],
    },
    Command {
        name: "report goal",
        summary: "The weekly goal's status and streak",
        writes: false,
        args: &[("--week-of <DATE>", "any date in the week")],
    },
    Command {
        name: "report losses",
        summary: "Minutes and blocks lost per category, pooled over a range",
        writes: false,
        args: &[("--from <DATE>", "first date"), ("--to <DATE>", "last date")],
    },
    Command {
        name: "export csv",
        summary: "The days and blocks CSV files (reads data; writes only to --out)",
        writes: false,
        args: &[
            ("--from <DATE> --to <DATE>", "range (default: the week of --date)"),
            ("--all", "every saved day"),
            ("--out <DIR>", "write both files here, never overwriting"),
            ("--stdout days|blocks", "return one file's text in the result"),
        ],
    },
];

pub const GLOBAL_FLAGS: &[(&str, &str)] = &[
    ("--data-dir <path>", "test data folder (env OEE_DATA_DIR); required for writes unless --real"),
    ("--real", "use the app's real data folder; required for writes without --data-dir"),
    ("--now <when>", "the clock: HH:MM or YYYY-MM-DDTHH:MM (env OEE_NOW)"),
    ("--date <YYYY-MM-DD>", "the day to work on; defaults to the date of --now"),
    ("--text", "short human summary instead of JSON"),
    ("--dry-run", "run a write command fully, print the result, save nothing"),
    ("--force", "write even if the desktop app is open on the same folder (otherwise exit 6)"),
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
            ("data_format", json!({ "type": "integer", "description": "the file format version written to day, calendar and settings files" })),
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
        "block list" => object(&[
            ("date", string()),
            ("blocks", json!({ "type": "array", "description": "start, end, kind (work|lost|planned), rework, category, planned_kind, uncategorised, note, finished" })),
        ]),
        "block uncategorised" => object(&[
            ("date", string()),
            ("count", json!({ "type": "integer" })),
            ("blocks", json!({ "type": "array", "description": "the same block objects as `block list`" })),
        ]),
        "block set-type" | "block reset-type" | "block categorise" | "block rework" | "block note" | "block bulk" => {
            block_edit_schema(name)
        }
        "calendar list" => object(&[("entries", json!({ "type": "array", "description": "id, day, from, to, kind, label, counts_as ({from,to,minutes} or null)" }))]),
        "calendar add" => object(&[
            ("added", json!({ "type": "array", "description": "the new entries, as in `calendar list`" })),
            ("saved", json!({ "type": "boolean" })),
        ]),
        "calendar remove" => object(&[("removed", json!({ "type": "object" })), ("saved", json!({ "type": "boolean" }))]),
        "settings get" | "settings set" => object(&[
            ("start", string()),
            ("end", string()),
            ("target", json!({ "type": "integer" })),
            ("shortcut", json!({ "type": "object", "description": "label, ctrl, alt, shift, cmd, key" })),
            ("goal", json!({ "type": "object", "description": "enabled, percent" })),
        ]),
        "sim run" => object(&[
            ("date", string()),
            ("steps", json!({ "type": "array", "description": "step, do, at, ok, result, notices (or error)" })),
            ("final", json!({ "type": "object", "description": "the `report day` result plus lost_blocks and uncategorised_blocks" })),
            ("expect", json!({ "type": "object", "description": "per key: expected, actual, pass" })),
            ("passed", json!({ "type": "boolean" })),
        ]),
        "data seed" => object(&[("scenario", string()), ("days", json!({ "type": "array", "description": "the dates written" })), ("saved", json!({ "type": "boolean" }))]),
        "data reset" => object(&[("removed_files", json!({ "type": "integer" })), ("saved", json!({ "type": "boolean" }))]),
        "check" => object(&[
            ("healthy", json!({ "type": "boolean" })),
            ("files_checked", json!({ "type": "integer" })),
            ("days_checked", json!({ "type": "integer" })),
            ("problems", json!({ "type": "array" })),
        ]),
        "report day" => object(&[
            ("date", string()),
            ("metrics", json!({ "type": "object", "description": "availability, efficiency, quality, oee, over_capacity, status, display" })),
            ("inputs", json!({ "type": ["object", "null"], "description": "null for a holiday" })),
        ]),
        "report week" => object(&[
            ("week_of", string()),
            ("week_end", string()),
            ("working_days", json!({ "type": "integer" })),
            ("days_counted", json!({ "type": "integer" })),
            ("pooled", json!({ "type": "object", "description": "metrics and inputs for the pooled week" })),
            ("days", json!({ "type": "array", "description": "date, weekday, status, points, metrics; Mon-Fri always, Sat/Sun only with a record" })),
            ("over_capacity_days", json!({ "type": "array", "description": "date, points, efficiency_display" })),
            ("goal", json!({ "type": ["object", "null"], "description": "percent, status, label, streak_weeks; null when the goal is off" })),
        ]),
        "report trends" => object(&[
            ("weeks", json!({ "type": "integer" })),
            ("from", string()),
            ("goal_percent", json!({ "type": ["integer", "null"] })),
            ("points", json!({ "type": "array", "description": "week_of, days_counted, metrics" })),
        ]),
        "report goal" => object(&[
            ("week_of", string()),
            ("oee", json!({ "type": "number" })),
            ("oee_display", string()),
            ("days_counted", json!({ "type": "integer" })),
            ("goal", json!({ "type": ["object", "null"] })),
        ]),
        "report losses" => object(&[
            ("from", json!({ "type": ["string", "null"] })),
            ("to", json!({ "type": ["string", "null"] })),
            ("days", json!({ "type": "integer" })),
            ("total_minutes", json!({ "type": "integer" })),
            ("categories", json!({ "type": "array", "description": "category, label, minutes, blocks" })),
            ("rework_blocks", json!({ "type": "integer" })),
            ("rework_minutes", json!({ "type": "integer" })),
        ]),
        "export csv" => object(&[
            ("from", string()),
            ("to", string()),
            ("days", json!({ "type": "integer" })),
            ("weekdays_note", string()),
            ("files", json!({ "type": ["object", "null"], "description": "written, days, blocks (paths)" })),
            ("csv", json!({ "type": ["string", "null"], "description": "the file's text with --stdout" })),
        ]),
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

/// A block edit returns the day's state plus the blocks it touched.
fn block_edit_schema(name: &str) -> Value {
    let mut fields = vec![
        ("date", string()),
        ("mode", json!({ "type": "object" })),
        ("start", string()),
        ("end", string()),
        ("finalised", json!({ "type": "boolean" })),
        ("block", json!({ "type": ["object", "null"] })),
        ("uncategorised_blocks", json!({ "type": "integer" })),
        ("metrics", json!({ "type": "object" })),
        ("inputs", json!({ "type": "object" })),
        ("saved", json!({ "type": "boolean", "description": "false with --dry-run" })),
        ("changed", json!({ "type": "array", "description": "the blocks addressed, after the change" })),
    ];
    if name == "block bulk" {
        fields.push(("applied", json!({ "type": "integer" })));
        fields.push(("skipped", json!({ "type": "array", "description": "start, code (no_such_block|not_lost_time|not_work), reason" })));
    }
    object(&fields)
}
