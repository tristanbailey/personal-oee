//! `office-oee`: command-line access to Personal OEE for agents and tests.
//! Design and command list: `docs/CLI_API.md`.
//!
//! [`run`] takes the arguments, the environment and the system clock, and
//! returns what to print and the exit code, so every behaviour can be tested
//! without starting a process.

pub mod catalogue;
pub mod day;
pub mod output;
pub mod view;

use std::path::PathBuf;

use chrono::{DateTime, Local, NaiveDate};
use clap::{Parser, Subcommand};
use oee_store::{clock, location};
use serde_json::json;

use output::{CliError, Exit, Reply, Stamp};

#[derive(Parser, Debug)]
#[command(
    name = "office-oee",
    about = "Personal OEE from the command line. JSON on stdout; see `office-oee commands`.",
    disable_version_flag = true
)]
pub struct Cli {
    /// Test data folder (env OEE_DATA_DIR). Required for writes unless --real.
    #[arg(long, global = true, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
    /// Use the app's real data folder.
    #[arg(long, global = true)]
    pub real: bool,
    /// The clock: HH:MM or YYYY-MM-DDTHH:MM (env OEE_NOW).
    #[arg(long, global = true, value_name = "WHEN")]
    pub now: Option<String>,
    /// The day to work on, YYYY-MM-DD. Defaults to the date of --now.
    #[arg(long, global = true, value_name = "DATE")]
    pub date: Option<String>,
    /// Short human summary instead of JSON.
    #[arg(long, global = true)]
    pub text: bool,
    /// Run a write command fully, print the result, save nothing.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Confirm a destructive command.
    #[arg(long, global = true)]
    pub yes: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// CLI and engine versions.
    Version,
    /// List every command, flag and exit code as JSON.
    Commands,
    /// JSON Schema of one command's result.
    Schema {
        /// A name from `commands`.
        command: String,
    },
    /// The day: start, status, downtime, extend, end, finish, show, list, delete.
    #[command(subcommand)]
    Day(day::DayCommand),
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Version => "version",
            Command::Commands => "commands",
            Command::Schema { .. } => "schema",
            Command::Day(d) => d.name(),
        }
    }
}

/// The parts of the environment the CLI reads.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub oee_data_dir: Option<PathBuf>,
    pub oee_now: Option<String>,
    pub home: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Self {
        Self {
            oee_data_dir: std::env::var_os(location::ENV_VAR).map(PathBuf::from),
            oee_now: std::env::var(clock::ENV_VAR).ok(),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }
}

/// What to print and how to exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: String,
    pub exit: i32,
}

/// What a command knows about where and when it is running.
pub struct Context {
    pub now: clock::Now,
    pub date: NaiveDate,
    pub text: bool,
    pub dry_run: bool,
    pub yes: bool,
    data_dir: Option<PathBuf>,
    real: bool,
    env: Env,
}

impl Context {
    /// The data folder for a command. A write must say `--data-dir` or `--real`.
    pub fn data_folder(&self, writes: bool) -> Result<PathBuf, CliError> {
        let req = location::Request {
            data_dir: self.data_dir.clone(),
            real: self.real,
            env: self.env.oee_data_dir.clone(),
            home: self.env.home.clone(),
            writes,
        };
        location::resolve(&req).map_err(|e| {
            let exit = match e {
                location::LocationError::Conflict => Exit::Usage,
                location::LocationError::NeedsChoice => Exit::NoLocation,
                location::LocationError::NoHome => Exit::Other,
            };
            CliError::new(exit, e.code(), e.message())
        })
    }

    fn stamp(&self) -> Stamp {
        Stamp {
            date: self.date.to_string(),
            now: format!("{:02}:{:02}", self.now.minutes / 60, self.now.minutes % 60),
        }
    }
}

/// Run one command line. `args` includes the program name first.
pub fn run<I, S>(args: I, env: &Env, system: DateTime<Local>) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let text_flag = args.iter().any(|a| a == "--text");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => return parse_failure(e, text_flag),
    };
    let name = cli.command.name();
    let ctx = match context(&cli, env, system) {
        Ok(c) => c,
        Err(e) => return failure(name, &e, cli.text),
    };
    match dispatch(&cli.command, &ctx) {
        Ok(reply) => success(name, &ctx, &reply),
        Err(e) => failure(name, &e, ctx.text),
    }
}

fn context(cli: &Cli, env: &Env, system: DateTime<Local>) -> Result<Context, CliError> {
    let now = clock::resolve(cli.now.as_deref(), env.oee_now.as_deref(), system)
        .map_err(|m| CliError::usage("bad_time", m))?;
    let date = match cli.date.as_deref() {
        None => now.date,
        Some(d) => NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").map_err(|_| {
            CliError::usage("bad_date", format!("Couldn't read the date \"{d}\". Use YYYY-MM-DD, for example 2026-11-02."))
        })?,
    };
    Ok(Context {
        now,
        date,
        text: cli.text,
        dry_run: cli.dry_run,
        yes: cli.yes,
        data_dir: cli.data_dir.clone(),
        real: cli.real,
        env: env.clone(),
    })
}

fn dispatch(command: &Command, ctx: &Context) -> Result<Reply, CliError> {
    match command {
        Command::Version => Ok(json!({
            "cli": env!("CARGO_PKG_VERSION"),
            "engine": env!("CARGO_PKG_VERSION"),
        })
        .into()),
        Command::Commands => Ok(catalogue::commands_result().into()),
        Command::Schema { command } => catalogue::schema_for(command)
            .map(Reply::from)
            .ok_or_else(|| {
                CliError::new(Exit::NotFound, "no_such_command", format!("There is no command called \"{command}\"."))
                    .hint("Run `office-oee commands` for the list.")
            }),
        Command::Day(d) => day::run(ctx, d),
    }
}

fn success(name: &str, ctx: &Context, reply: &Reply) -> Outcome {
    let stdout = if ctx.text {
        output::result_text(&reply.result)
    } else {
        format!("{}\n", output::success_json(name, &ctx.stamp(), &reply.result, &reply.notices))
    };
    Outcome { stdout, exit: Exit::Ok.code() }
}

fn failure(name: &str, error: &CliError, text: bool) -> Outcome {
    let stdout = if text {
        output::error_text(error)
    } else {
        format!("{}\n", output::failure_json(name, error))
    };
    Outcome { stdout, exit: error.exit.code() }
}

/// A command line clap rejected, or `--help`.
fn parse_failure(e: clap::Error, text: bool) -> Outcome {
    use clap::error::ErrorKind;
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand) {
        return Outcome { stdout: e.to_string(), exit: Exit::Ok.code() };
    }
    let first = e
        .to_string()
        .lines()
        .next()
        .unwrap_or("bad command line")
        .trim_start_matches("error: ")
        .to_string();
    let error = CliError::usage("usage", first).hint("Run `office-oee commands` for every command and flag.");
    failure("", &error, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ctx(args: &[&str], env: Env) -> Context {
        let cli = Cli::try_parse_from(std::iter::once("office-oee").chain(args.iter().copied()).chain(["version"])).unwrap();
        context(&cli, &env, Local.with_ymd_and_hms(2026, 11, 1, 15, 12, 0).unwrap()).unwrap()
    }

    fn home() -> Env {
        Env { home: Some("/Users/t".into()), ..Env::default() }
    }

    #[test]
    fn reads_default_to_the_real_folder() {
        let c = ctx(&[], home());
        assert_eq!(
            c.data_folder(false).unwrap(),
            PathBuf::from("/Users/t/Library/Application Support/PersonalOEE")
        );
    }

    #[test]
    fn a_write_with_no_location_is_exit_7() {
        let e = ctx(&[], home()).data_folder(true).unwrap_err();
        assert_eq!((e.exit.code(), e.code.as_str()), (7, "data_location_required"));
    }

    #[test]
    fn a_write_with_data_dir_or_real_is_allowed() {
        assert_eq!(ctx(&["--data-dir", "/tmp/x"], home()).data_folder(true).unwrap(), PathBuf::from("/tmp/x"));
        assert!(ctx(&["--real"], home()).data_folder(true).unwrap().ends_with("PersonalOEE"));
        let env = Env { oee_data_dir: Some("/tmp/e".into()), ..home() };
        assert_eq!(ctx(&[], env).data_folder(true).unwrap(), PathBuf::from("/tmp/e"));
    }

    #[test]
    fn real_together_with_data_dir_is_a_usage_error() {
        let e = ctx(&["--real", "--data-dir", "/tmp/x"], home()).data_folder(true).unwrap_err();
        assert_eq!((e.exit.code(), e.code.as_str()), (2, "location_conflict"));
    }
}
