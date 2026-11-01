//! What every command prints: one JSON envelope, and a stable exit code
//! (docs/CLI_API.md sections 4.3 and 4.4).

use serde::Serialize;
use serde_json::{json, Value};

/// Exit codes. Stable: agents branch on these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    Other = 1,
    /// Bad flag, bad time format, missing argument.
    Usage = 2,
    /// No day record, no such block, no such calendar entry.
    NotFound = 3,
    /// A rule refused the action (rework on a lost block, toggle during Planned…).
    Rule = 4,
    /// An unreadable or invalid data file.
    Data = 5,
    /// The desktop app is open and the write was refused.
    AppRunning = 6,
    /// A write with no `--data-dir` or `--real`.
    NoLocation = 7,
}

impl Exit {
    pub const ALL: [Exit; 8] = [
        Exit::Ok,
        Exit::Other,
        Exit::Usage,
        Exit::NotFound,
        Exit::Rule,
        Exit::Data,
        Exit::AppRunning,
        Exit::NoLocation,
    ];

    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn meaning(self) -> &'static str {
        match self {
            Exit::Ok => "Success",
            Exit::Other => "Anything else",
            Exit::Usage => "Usage error: bad flag, bad time format, missing argument",
            Exit::NotFound => "Not found: no day record, no such block, no such calendar entry",
            Exit::Rule => "Rule refused the action",
            Exit::Data => "Data problem: unreadable or invalid file",
            Exit::AppRunning => "The desktop app is open and the write was refused",
            Exit::NoLocation => "A write with no --data-dir or --real",
        }
    }
}

/// A failure with a stable `code`, plain words, and often a fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip)]
    pub exit: Exit,
}

impl CliError {
    pub fn new(exit: Exit, code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: None,
            exit,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn usage(code: &str, message: impl Into<String>) -> Self {
        Self::new(Exit::Usage, code, message)
    }
}

/// A command's answer: the `result` and any notices the tracker raised.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub result: Value,
    pub notices: Vec<Value>,
}

impl From<Value> for Reply {
    fn from(result: Value) -> Self {
        Self { result, notices: Vec::new() }
    }
}

/// What the envelope says about when and for which day the command ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub date: String,
    pub now: String,
}

#[derive(Serialize)]
struct Success<'a> {
    ok: bool,
    command: &'a str,
    date: &'a str,
    now: &'a str,
    result: &'a Value,
    notices: &'a [Value],
}

#[derive(Serialize)]
struct Failure<'a> {
    ok: bool,
    command: &'a str,
    error: &'a CliError,
}

pub fn success_json(command: &str, stamp: &Stamp, result: &Value, notices: &[Value]) -> String {
    let env = Success {
        ok: true,
        command,
        date: &stamp.date,
        now: &stamp.now,
        result,
        notices,
    };
    serde_json::to_string(&env).expect("envelope serialises")
}

pub fn failure_json(command: &str, error: &CliError) -> String {
    let env = Failure {
        ok: false,
        command,
        error,
    };
    serde_json::to_string(&env).expect("envelope serialises")
}

/// `--text`: a short readable form of a result. Objects become `key: value`
/// lines, arrays become `- item` lines, nested values are indented.
pub fn result_text(value: &Value) -> String {
    let mut out = String::new();
    write_text(value, 0, &mut out);
    out
}

fn write_text(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                match v {
                    Value::Object(_) | Value::Array(_) if !is_empty(v) => {
                        out.push_str(&format!("{pad}{k}:\n"));
                        write_text(v, depth + 1, out);
                    }
                    _ => out.push_str(&format!("{pad}{k}: {}\n", scalar(v))),
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                match v {
                    Value::Object(_) | Value::Array(_) if !is_empty(v) => {
                        out.push_str(&format!("{pad}-\n"));
                        write_text(v, depth + 1, out);
                    }
                    _ => out.push_str(&format!("{pad}- {}\n", scalar(v))),
                }
            }
        }
        other => out.push_str(&format!("{pad}{}\n", scalar(other))),
    }
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "none".into(),
        Value::Object(_) => "{}".into(),
        Value::Array(_) => "[]".into(),
        other => other.to_string(),
    }
}

/// `--text` for a failure.
pub fn error_text(error: &CliError) -> String {
    match &error.hint {
        Some(h) => format!("error: {}\nhint: {h}\n", error.message),
        None => format!("error: {}\n", error.message),
    }
}

pub fn exit_code_list() -> Value {
    Value::Array(
        Exit::ALL
            .iter()
            .map(|e| json!({ "code": e.code(), "meaning": e.meaning() }))
            .collect(),
    )
}
