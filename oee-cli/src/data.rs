//! `data seed`, `data reset` and `check` (docs/CLI_API.md 5.5): known
//! datasets for tests, a clean slate, and a health check.

use clap::Subcommand;
use oee_store::store::Store;
use oee_store::{check, seed};
use serde_json::{json, Value};

use crate::day::data_err;
use crate::output::{CliError, Exit, Reply};
use crate::Context;

#[derive(Subcommand, Debug)]
pub enum DataCommand {
    /// Fill the data folder with a known dataset around --date's week.
    Seed {
        /// empty | week4 | holiday-week | zero-work-day | over-capacity | uncategorised-day | streak-3
        #[arg(long, value_name = "NAME")]
        scenario: String,
    },
    /// Delete every saved day, the calendar, settings and UI state (needs --yes).
    Reset,
}

impl DataCommand {
    pub fn name(&self) -> &'static str {
        match self {
            DataCommand::Seed { .. } => "data seed",
            DataCommand::Reset => "data reset",
        }
    }
}

pub fn run(ctx: &Context, cmd: &DataCommand) -> Result<Reply, CliError> {
    match cmd {
        DataCommand::Seed { scenario } => seed_data(ctx, scenario),
        DataCommand::Reset => reset(ctx),
    }
}

fn seed_data(ctx: &Context, scenario: &str) -> Result<Reply, CliError> {
    let built = seed::build(scenario, ctx.date).ok_or_else(|| {
        let names: Vec<&str> = seed::SCENARIOS.iter().map(|(n, _)| *n).collect();
        CliError::usage("bad_scenario", format!("Unknown scenario \"{scenario}\"."))
            .hint(format!("Use one of: {}.", names.join(", ")))
    })?;
    let store = Store::new(ctx.data_folder(true)?);
    if !store.is_empty() && !ctx.yes {
        return Err(CliError::new(
            Exit::Rule,
            "data_not_empty",
            format!("{} already holds data, and seeding replaces all of it.", store.root().display()),
        )
        .hint("Add --yes to replace it."));
    }
    let dates: Vec<String> = built.days.iter().map(|(d, _)| d.to_string()).collect();
    if !ctx.dry_run {
        store.clear().map_err(|e| data_err("Couldn't clear the data folder", e))?;
        for (date, record) in &built.days {
            store.save_day(*date, record).map_err(|e| data_err(&format!("Couldn't save {date}"), e))?;
        }
        store
            .save_settings(&Default::default())
            .map_err(|e| data_err("Couldn't save settings.json", e))?;
    }
    Ok(json!({ "scenario": scenario, "days": dates, "saved": !ctx.dry_run }).into())
}

fn reset(ctx: &Context) -> Result<Reply, CliError> {
    if !ctx.yes {
        return Err(CliError::usage("confirmation_required", "Resetting deletes every saved day, the calendar and the settings.")
            .hint("Add --yes to confirm."));
    }
    let store = Store::new(ctx.data_folder(true)?);
    let removed = if ctx.dry_run { 0 } else { store.clear().map_err(|e| data_err("Couldn't clear the data folder", e))? };
    Ok(json!({ "removed_files": removed, "saved": !ctx.dry_run }).into())
}

/// `check`: read every file and test the invariants. Exit 5 if anything is wrong.
pub fn check(ctx: &Context) -> Result<Reply, CliError> {
    // Open the folder directly: a bad settings or calendar file is a finding, not a reason to stop.
    let store = Store::new(ctx.data_folder(false)?);
    let report = check::run(&store, &Default::default());
    let problems: Vec<Value> = report
        .problems
        .iter()
        .map(|p| json!({ "file": p.file, "code": p.code, "message": p.message }))
        .collect();
    if !problems.is_empty() {
        let list: Vec<String> = report.problems.iter().map(|p| format!("{}: {} ({})", p.file, p.message, p.code)).collect();
        return Err(CliError::new(Exit::Data, "check_failed", format!("{} problem(s): {}", problems.len(), list.join("; ")))
            .hint("Fix or delete the named files."));
    }
    Ok(json!({ "healthy": true, "files_checked": report.files_checked, "days_checked": report.days_checked, "problems": problems }).into())
}
