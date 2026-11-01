//! `day …` commands: the lifecycle of one day (docs/CLI_API.md 5.1).
//!
//! Each command loads the day, catches the tracker up to `--now`, applies one
//! action, saves (unless `--dry-run`) and prints the new state. No background
//! process: time only moves when `--now` moves.

use chrono::NaiveDate;
use clap::Subcommand;
use oee_core::{day_result, Mode, Notice, Tracker, BLOCK_MINUTES};
use oee_store::settings::AppSettings;
use oee_store::start::{parse_hhmm, planned_for, validate_start};
use oee_store::store::Store;
use serde_json::{json, Value};

use crate::output::{CliError, Exit, Reply};
use crate::view::{self, hhmm};
use crate::Context;

#[derive(Subcommand, Debug)]
pub enum DayCommand {
    /// Start the day, from the weekly calendar's planned windows.
    Start {
        /// Start time HH:MM, not later than now. Default: now.
        #[arg(long, value_name = "HH:MM")]
        at: Option<String>,
    },
    /// Mode, current block, time left and the metrics so far.
    Status,
    /// Work becomes Downtime, Downtime becomes Work.
    Toggle,
    /// Explicit downtime start or stop; refused if already in that state.
    #[command(subcommand)]
    Downtime(DowntimeCommand),
    /// Move the finish later; reopens an ended day.
    Extend {
        #[arg(long, value_name = "HH:MM")]
        to: String,
    },
    /// End the day now.
    End,
    /// Finalise: uncategorised lost blocks become System Down.
    Finish,
    /// The whole record: planned windows, stops, every block, metrics.
    Show,
    /// Saved days with their status and OEE.
    List {
        #[arg(long, value_name = "DATE")]
        from: Option<String>,
        #[arg(long, value_name = "DATE")]
        to: Option<String>,
    },
    /// Remove a saved day (needs --yes).
    Delete,
}

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum DowntimeCommand {
    Start,
    Stop,
}

impl DayCommand {
    pub fn name(&self) -> &'static str {
        match self {
            DayCommand::Start { .. } => "day start",
            DayCommand::Status => "day status",
            DayCommand::Toggle => "day toggle",
            DayCommand::Downtime(DowntimeCommand::Start) => "day downtime start",
            DayCommand::Downtime(DowntimeCommand::Stop) => "day downtime stop",
            DayCommand::Extend { .. } => "day extend",
            DayCommand::End => "day end",
            DayCommand::Finish => "day finish",
            DayCommand::Show => "day show",
            DayCommand::List { .. } => "day list",
            DayCommand::Delete => "day delete",
        }
    }
}

pub fn run(ctx: &Context, cmd: &DayCommand) -> Result<Reply, CliError> {
    match cmd {
        DayCommand::Start { at } => start(ctx, at.as_deref()),
        DayCommand::Status => status(ctx),
        DayCommand::Toggle => toggle(ctx, Want::Either),
        DayCommand::Downtime(DowntimeCommand::Start) => toggle(ctx, Want::Start),
        DayCommand::Downtime(DowntimeCommand::Stop) => toggle(ctx, Want::Stop),
        DayCommand::Extend { to } => extend(ctx, to),
        DayCommand::End => end(ctx),
        DayCommand::Finish => finish(ctx),
        DayCommand::Show => show(ctx),
        DayCommand::List { from, to } => list(ctx, from.as_deref(), to.as_deref()),
        DayCommand::Delete => delete(ctx),
    }
}

// ---- shared helpers -------------------------------------------------------

struct Open {
    store: Store,
    settings: AppSettings,
    calendar: oee_core::WeeklyCalendar,
}

fn data_err(what: &str, e: impl std::fmt::Display) -> CliError {
    CliError::new(Exit::Data, "data_unreadable", format!("{what}: {e}"))
        .hint("Run `office-oee check` to find the bad file.")
}

fn open(ctx: &Context, writes: bool) -> Result<Open, CliError> {
    let store = Store::new(ctx.data_folder(writes)?);
    let settings = store
        .load_settings()
        .map_err(|e| data_err("Couldn't read settings.json", e))?
        .unwrap_or_default();
    let calendar = store
        .load_calendar()
        .map_err(|e| data_err("Couldn't read calendar.json", e))?
        .unwrap_or_default();
    Ok(Open { store, settings, calendar })
}

/// The minute the engine runs at for `date`: now if it is today, the end of
/// the day for a past date, the start of the day for a future one.
fn now_for(ctx: &Context, date: NaiveDate) -> u32 {
    use std::cmp::Ordering::*;
    match date.cmp(&ctx.now.date) {
        Equal => ctx.now.minutes,
        Less => 24 * 60,
        Greater => 0,
    }
}

fn require_today(ctx: &Context) -> Result<(), CliError> {
    if ctx.date != ctx.now.date {
        return Err(CliError::new(
            Exit::Rule,
            "not_today",
            format!("This command works on the current day, but the day is {} and now is {}.", ctx.date, ctx.now.date),
        )
        .hint("Use `--date` equal to today, or edit past days with the `block` commands."));
    }
    Ok(())
}

fn load_tracker(ctx: &Context, o: &Open) -> Result<(Tracker, Vec<Notice>), CliError> {
    let record = o
        .store
        .load_day(ctx.date)
        .map_err(|e| data_err(&format!("Couldn't read the day {}", ctx.date), e))?
        .ok_or_else(|| {
            CliError::new(Exit::NotFound, "no_day", format!("No day has been started for {}.", ctx.date))
                .hint("Run `office-oee day start`.")
        })?;
    Ok(Tracker::resume_with_notices(record, o.settings.core, now_for(ctx, ctx.date)))
}

/// Save unless `--dry-run`. Returns whether anything was written.
fn save(ctx: &Context, o: &Open, t: &Tracker) -> Result<bool, CliError> {
    if ctx.dry_run {
        return Ok(false);
    }
    o.store
        .save_day(ctx.date, &t.record)
        .map_err(|e| data_err(&format!("Couldn't save the day {}", ctx.date), e))?;
    Ok(true)
}

fn notices_json(n: &[Notice]) -> Vec<Value> {
    n.iter().map(view::notice_json).collect()
}

/// The state of the day: what `status` prints and every write returns.
fn state_json(ctx: &Context, t: &Tracker) -> Value {
    let now = now_for(ctx, ctx.date);
    let live = t.live_record(now);
    let inputs = t.inputs_at(now);
    let block = match t.mode() {
        Mode::Ended => Value::Null,
        _ if now < live.start || now >= live.end => Value::Null,
        _ => {
            let into = now - live.start;
            let start = live.start + into / BLOCK_MINUTES * BLOCK_MINUTES;
            json!({
                "number": into / BLOCK_MINUTES + 1,
                "of": live.slot_count(),
                "start": hhmm(start),
                "end": hhmm(start + BLOCK_MINUTES),
                "minutes_left": start + BLOCK_MINUTES - now,
            })
        }
    };
    json!({
        "date": ctx.date.to_string(),
        "mode": view::mode_json(t.mode()),
        "start": hhmm(live.start),
        "end": hhmm(live.end),
        "finalised": live.finalised,
        "block": block,
        "uncategorised_blocks": live.uncategorised().len(),
        "metrics": view::result_json(&day_result(Some(&inputs))),
        "inputs": view::inputs_json(&inputs),
    })
}

fn with_saved(mut state: Value, saved: bool) -> Value {
    state["saved"] = json!(saved);
    state
}

fn parse_time(text: &str, flag: &str) -> Result<u32, CliError> {
    parse_hhmm(text).ok_or_else(|| {
        CliError::usage("bad_time", format!("Couldn't read {flag} \"{text}\". Use HH:MM, for example 17:40."))
    })
}

fn parse_date(text: &str, flag: &str) -> Result<NaiveDate, CliError> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").map_err(|_| {
        CliError::usage("bad_date", format!("Couldn't read {flag} \"{text}\". Use YYYY-MM-DD, for example 2026-11-02."))
    })
}

// ---- commands ---------------------------------------------------------------

fn start(ctx: &Context, at: Option<&str>) -> Result<Reply, CliError> {
    let o = open(ctx, true)?;
    if ctx.date > ctx.now.date {
        return Err(CliError::new(Exit::Rule, "future_date", "A day can't be started in the future."));
    }
    if o.store
        .load_day(ctx.date)
        .map_err(|e| data_err(&format!("Couldn't read the day {}", ctx.date), e))?
        .is_some()
    {
        return Err(CliError::new(Exit::Rule, "day_exists", format!("A day for {} is already started.", ctx.date))
            .hint("Run `office-oee day status`, or `day delete --yes` to start again."));
    }
    let now = now_for(ctx, ctx.date);
    let start = match at {
        Some(text) if ctx.date == ctx.now.date => validate_start(text, now)
            .map_err(|m| CliError::usage("bad_start", m))?,
        Some(text) => parse_time(text, "--at")?,
        None if ctx.date == ctx.now.date => now,
        None => o.settings.core.default_start,
    };
    let planned = planned_for(&o.calendar, ctx.date);
    let mut t = Tracker::start_day(start, planned, o.settings.core);
    let notices = t.tick(now);
    let saved = save(ctx, &o, &t)?;
    Ok(Reply { result: with_saved(state_json(ctx, &t), saved), notices: notices_json(&notices) })
}

fn status(ctx: &Context) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let (t, notices) = load_tracker(ctx, &o)?;
    Ok(Reply { result: state_json(ctx, &t), notices: notices_json(&notices) })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    Either,
    Start,
    Stop,
}

fn toggle(ctx: &Context, want: Want) -> Result<Reply, CliError> {
    require_today(ctx)?;
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    let refuse = |code: &str, msg: String| Err(CliError::new(Exit::Rule, code, msg));
    match (t.mode(), want) {
        (Mode::Planned { until, .. }, _) => {
            return refuse(
                "in_planned_downtime",
                format!("Planned downtime is running until {}, so the toggle is ignored (FR-036).", hhmm(until)),
            )
        }
        (Mode::Ended, _) => {
            return Err(CliError::new(Exit::Rule, "day_ended", "The day has ended.")
                .hint("Run `office-oee day extend --to HH:MM` to reopen it."))
        }
        (Mode::Downtime { since }, Want::Start) => {
            return refuse("already_in_downtime", format!("Downtime has been running since {}.", hhmm(since)))
        }
        (Mode::Work, Want::Stop) => return refuse("not_in_downtime", "No downtime is running.".into()),
        _ => {}
    }
    let toggled = t.toggle_downtime_report(ctx.now.minutes);
    let saved = save(ctx, &o, &t)?;
    let stopped = toggled.stopped.map(|s| {
        let message = match s.lost_blocks {
            0 => format!("{} min stop ignored. It counts as Work.", s.minutes),
            1 => format!("{} min stop logged as 1 lost block. Categorise it later.", s.minutes),
            n => format!("{} min stop logged as {n} lost blocks. Categorise them later.", s.minutes),
        };
        json!({ "minutes": s.minutes, "lost_blocks": s.lost_blocks, "message": message })
    });
    let mut state = with_saved(state_json(ctx, &t), saved);
    state["stopped"] = stopped.unwrap_or(Value::Null);
    Ok(Reply { result: state, notices: notices_json(&notices) })
}

fn extend(ctx: &Context, to: &str) -> Result<Reply, CliError> {
    require_today(ctx)?;
    let end = parse_time(to, "--to")?;
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    if end <= t.record.end {
        return Err(CliError::new(
            Exit::Rule,
            "not_later",
            format!("The day already finishes at {}; --to must be later.", hhmm(t.record.end)),
        ));
    }
    t.extend_to(end, ctx.now.minutes);
    let saved = save(ctx, &o, &t)?;
    Ok(Reply { result: with_saved(state_json(ctx, &t), saved), notices: notices_json(&notices) })
}

fn end(ctx: &Context) -> Result<Reply, CliError> {
    require_today(ctx)?;
    let o = open(ctx, true)?;
    let (mut t, mut notices) = load_tracker(ctx, &o)?;
    if t.mode() == Mode::Ended {
        return Err(CliError::new(Exit::Rule, "already_ended", "The day has already ended.")
            .hint("Run `office-oee day extend --to HH:MM` to reopen it."));
    }
    notices.extend(t.end_now(ctx.now.minutes));
    let saved = save(ctx, &o, &t)?;
    Ok(Reply { result: with_saved(state_json(ctx, &t), saved), notices: notices_json(&notices) })
}

fn finish(ctx: &Context) -> Result<Reply, CliError> {
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    let defaulted = t.record.uncategorised().len();
    t.finalise();
    let saved = save(ctx, &o, &t)?;
    let mut state = with_saved(state_json(ctx, &t), saved);
    state["defaulted_to_system_down"] = json!(defaulted);
    Ok(Reply { result: state, notices: notices_json(&notices) })
}

fn show(ctx: &Context) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let (t, notices) = load_tracker(ctx, &o)?;
    let now = now_for(ctx, ctx.date);
    let live = t.live_record(now);
    let blocks = live.blocks();
    let mut state = state_json(ctx, &t);
    state["record"] = view::record_json(&live, &blocks);
    Ok(Reply { result: state, notices: notices_json(&notices) })
}

fn list(ctx: &Context, from: Option<&str>, to: Option<&str>) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let from = from.map(|d| parse_date(d, "--from")).transpose()?;
    let to = to.map(|d| parse_date(d, "--to")).transpose()?;
    let mut days = Vec::new();
    for date in o.store.list_days() {
        if from.is_some_and(|f| date < f) || to.is_some_and(|t| date > t) {
            continue;
        }
        let record = match o.store.load_day(date) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(e) => {
                days.push(json!({ "date": date.to_string(), "error": e.to_string() }));
                continue;
            }
        };
        let t = Tracker::resume(record, o.settings.core, now_for(ctx, date));
        let now = now_for(ctx, date);
        let r = day_result(Some(&t.inputs_at(now)));
        days.push(json!({
            "date": date.to_string(),
            "start": hhmm(t.record.start),
            "end": hhmm(t.record.end),
            "finalised": t.record.finalised,
            "mode": view::mode_json(t.mode())["kind"],
            "status": view::status_name(r.status),
            "oee": view::metrics_json(&r.metrics)["oee"],
            "oee_display": view::metrics_json(&r.metrics)["display"]["oee"],
        }));
    }
    Ok(Reply { result: json!({ "days": days }), notices: vec![] })
}

fn delete(ctx: &Context) -> Result<Reply, CliError> {
    if !ctx.yes {
        return Err(CliError::usage(
            "confirmation_required",
            format!("Deleting the day {} can't be undone.", ctx.date),
        )
        .hint("Add --yes to confirm."));
    }
    let o = open(ctx, true)?;
    let exists = o
        .store
        .load_day(ctx.date)
        .map_err(|e| data_err(&format!("Couldn't read the day {}", ctx.date), e))?
        .is_some();
    if !exists {
        return Err(CliError::new(Exit::NotFound, "no_day", format!("No day has been started for {}.", ctx.date)));
    }
    if ctx.dry_run {
        return Ok(Reply { result: json!({ "date": ctx.date.to_string(), "deleted": false, "saved": false }), notices: vec![] });
    }
    o.store
        .delete_day(ctx.date)
        .map_err(|e| data_err(&format!("Couldn't delete the day {}", ctx.date), e))?;
    Ok(Reply { result: json!({ "date": ctx.date.to_string(), "deleted": true, "saved": true }), notices: vec![] })
}
