//! `report …` and `export csv` (docs/CLI_API.md 5.4). Read-only: they use the
//! same builders as the Week tab (`oee_store::weekly`) so the numbers match.

use std::path::PathBuf;

use chrono::{Duration, NaiveDate};
use clap::{Subcommand, ValueEnum};
use oee_core::{
    day_result, goal_status, goal_streak, DayInputs, DayStatus, GoalStatus, LostCategory,
};
use oee_store::export_range::{build_days, write_files, write_totals};
use oee_store::weekly::{self, day_inputs, over_capacity, report as pooled, series_from, week_start};
use serde_json::{json, Value};

use crate::day::{data_err, open, parse_date, Open};
use crate::output::{CliError, Reply};
use crate::view::{self, category_name};
use crate::Context;

#[derive(Subcommand, Debug)]
pub enum ReportCommand {
    /// One day's Availability, Efficiency, Quality and OEE. A date with no record is a holiday.
    Day,
    /// The Week tab: per-day rows, pooled week, losses, over-capacity days, goal.
    Week {
        /// Any date in the week; default is the week of --date.
        #[arg(long, value_name = "DATE")]
        week_of: Option<String>,
    },
    /// One pooled point per working week.
    Trends {
        /// 4, 12 or 26 weeks ending with the current week.
        #[arg(long, default_value_t = 12)]
        weeks: u32,
    },
    /// The weekly goal's status and streak.
    Goal {
        #[arg(long, value_name = "DATE")]
        week_of: Option<String>,
    },
    /// Minutes and blocks lost per category, pooled over a range.
    Losses {
        #[arg(long, value_name = "DATE")]
        from: Option<String>,
        #[arg(long, value_name = "DATE")]
        to: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ExportCommand {
    /// The days and blocks CSV files, as the Week tab's Export panel writes them.
    Csv {
        /// First date. Default: Monday of the week of --date.
        #[arg(long, value_name = "DATE")]
        from: Option<String>,
        /// Last date. Default: Sunday of that week.
        #[arg(long, value_name = "DATE")]
        to: Option<String>,
        /// Every saved day.
        #[arg(long, conflicts_with_all = ["from", "to"])]
        all: bool,
        /// Write the days, blocks and totals files into this folder (never overwrites an earlier export).
        #[arg(long, value_name = "DIR", conflicts_with = "stdout")]
        out: Option<PathBuf>,
        /// Return one file's text in the result instead of writing files.
        #[arg(long, value_enum)]
        stdout: Option<Which>,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Days,
    Blocks,
    /// D-60: the spreadsheet totals file.
    Totals,
}

impl ReportCommand {
    pub fn name(&self) -> &'static str {
        match self {
            ReportCommand::Day => "report day",
            ReportCommand::Week { .. } => "report week",
            ReportCommand::Trends { .. } => "report trends",
            ReportCommand::Goal { .. } => "report goal",
            ReportCommand::Losses { .. } => "report losses",
        }
    }
}

impl ExportCommand {
    pub fn name(&self) -> &'static str {
        "export csv"
    }
}

pub fn run(ctx: &Context, cmd: &ReportCommand) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let result = match cmd {
        ReportCommand::Day => day(ctx, &o)?,
        ReportCommand::Week { week_of } => week(ctx, &o, week_of.as_deref())?,
        ReportCommand::Trends { weeks } => trends(ctx, &o, *weeks)?,
        ReportCommand::Goal { week_of } => goal(ctx, &o, week_of.as_deref())?,
        ReportCommand::Losses { from, to } => losses(ctx, &o, from.as_deref(), to.as_deref())?,
    };
    Ok(result.into())
}

fn all_days(o: &Open) -> Vec<(NaiveDate, oee_core::DayRecord)> {
    o.store.load_all()
}

fn monday_of(ctx: &Context, week_of: Option<&str>) -> Result<NaiveDate, CliError> {
    let date = match week_of {
        Some(d) => parse_date(d, "--week-of")?,
        None => ctx.date,
    };
    Ok(week_start(date))
}

fn day(ctx: &Context, o: &Open) -> Result<Value, CliError> {
    let record = o
        .store
        .load_day(ctx.date)
        .map_err(|e| data_err(&format!("Couldn't read the day {}", ctx.date), e))?;
    let inputs = record
        .as_ref()
        .map(|r| day_inputs(r, ctx.date == ctx.now.date, ctx.now.minutes, &o.settings.core));
    let result = day_result(inputs.as_ref());
    let mut v = json!({
        "date": ctx.date.to_string(),
        "metrics": view::result_json(&result),
        "inputs": inputs.as_ref().map(view::inputs_json),
    });
    v["metrics"]["status"] = json!(view::status_name(result.status));
    Ok(v)
}

fn goal_json(o: &Open, oee: f64, days_counted: usize, monday: NaiveDate, today: NaiveDate, all: &[(NaiveDate, oee_core::DayRecord)], now: u32) -> Value {
    let Some(percent) = o.settings.goal.active() else { return Value::Null };
    let status = goal_status(oee, days_counted, percent, week_start(today) == monday);
    let every = series_from(all, NaiveDate::MIN, today, now, &o.settings.core);
    let streak = goal_streak(&every, percent, week_start(today));
    json!({
        "percent": percent,
        "status": status.map(goal_name),
        "label": status.map(GoalStatus::label),
        "streak_weeks": streak,
    })
}

fn goal_name(s: GoalStatus) -> &'static str {
    match s {
        GoalStatus::Met => "met",
        GoalStatus::OnTrack => "on-track",
        GoalStatus::Behind => "behind",
        GoalStatus::NotMet => "not-met",
    }
}

fn week(ctx: &Context, o: &Open, week_of: Option<&str>) -> Result<Value, CliError> {
    let monday = monday_of(ctx, week_of)?;
    let (today, now, core) = (ctx.now.date, ctx.now.minutes, &o.settings.core);
    let days = o.store.load_week(monday);
    let rep = pooled(&days, today, now, core);
    let rows: Vec<Value> = days
        .iter()
        .enumerate()
        .filter(|(i, (_, r))| *i < 5 || r.is_some())
        .map(|(_, (date, record))| {
            let inputs = record.as_ref().map(|r| day_inputs(r, *date == today, now, core));
            let result = day_result(inputs.as_ref());
            json!({
                "date": date.to_string(),
                "weekday": date.format("%a").to_string(),
                "status": view::status_name(result.status),
                "points": inputs.as_ref().map(|i| format!("{}/{}", i.work_points, i.target_points)),
                "metrics": view::metrics_json(&result.metrics),
            })
        })
        .collect();
    let all = all_days(o);
    let flagged: Vec<Value> = over_capacity(&all, today, now, core)
        .into_iter()
        .filter(|d| week_start(d.date) == monday)
        .map(|d| json!({
            "date": d.date.to_string(),
            "points": format!("{}/{}", d.inputs.work_points, d.inputs.target_points),
            "efficiency_display": view::metrics_json(&d.result.metrics)["display"]["efficiency"],
        }))
        .collect();
    Ok(json!({
        "week_of": monday.to_string(),
        "week_end": (monday + Duration::days(6)).to_string(),
        "working_days": weekly::working_days(&days),
        "days_counted": rep.days_counted,
        "pooled": {
            "metrics": view::metrics_json(&rep.metrics),
            "inputs": view::inputs_json(&rep.totals),
        },
        "days": rows,
        "over_capacity_days": flagged,
        "goal": goal_json(o, rep.metrics.oee, rep.days_counted, monday, today, &all, now),
    }))
}

fn trends(ctx: &Context, o: &Open, weeks: u32) -> Result<Value, CliError> {
    if ![4, 12, 26].contains(&weeks) {
        return Err(CliError::usage("bad_weeks", format!("--weeks must be 4, 12 or 26, not {weeks}.")));
    }
    let (today, now, core) = (ctx.now.date, ctx.now.minutes, &o.settings.core);
    let first = weekly::first_monday(week_start(today), weeks);
    let points = series_from(&all_days(o), first, today, now, core);
    Ok(json!({
        "weeks": weeks,
        "from": first.to_string(),
        "goal_percent": o.settings.goal.active(),
        "points": points.iter().map(|p| json!({
            "week_of": p.week.to_string(),
            "days_counted": p.days_counted,
            "metrics": view::metrics_json(&p.metrics),
        })).collect::<Vec<_>>(),
    }))
}

fn goal(ctx: &Context, o: &Open, week_of: Option<&str>) -> Result<Value, CliError> {
    let monday = monday_of(ctx, week_of)?;
    let (today, now, core) = (ctx.now.date, ctx.now.minutes, &o.settings.core);
    let rep = pooled(&o.store.load_week(monday), today, now, core);
    let all = all_days(o);
    Ok(json!({
        "week_of": monday.to_string(),
        "oee": view::metrics_json(&rep.metrics)["oee"],
        "oee_display": view::metrics_json(&rep.metrics)["display"]["oee"],
        "days_counted": rep.days_counted,
        "goal": goal_json(o, rep.metrics.oee, rep.days_counted, monday, today, &all, now),
    }))
}

fn losses(ctx: &Context, o: &Open, from: Option<&str>, to: Option<&str>) -> Result<Value, CliError> {
    let from = from.map(|d| parse_date(d, "--from")).transpose()?;
    let to = to.map(|d| parse_date(d, "--to")).transpose()?;
    let (today, now, core) = (ctx.now.date, ctx.now.minutes, &o.settings.core);
    let mut totals = DayInputs::default();
    let mut blocks = [0u32; 5];
    let mut rework_minutes = 0;
    let mut days = 0;
    for (date, record) in all_days(o) {
        if from.is_some_and(|f| date < f) || to.is_some_and(|t| date > t) {
            continue;
        }
        let inputs = day_inputs(&record, date == today, now, core);
        if day_result(Some(&inputs)).status == DayStatus::Holiday {
            continue;
        }
        for (i, c) in LostCategory::ALL.iter().enumerate() {
            blocks[i] += inputs.losses.get(*c) / c.downtime_minutes(record.cycle_minutes);
        }
        rework_minutes += inputs.rework_points * record.cycle_minutes;
        totals += inputs;
        days += 1;
    }
    let by_category: Vec<Value> = LostCategory::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let minutes = totals.losses.get(*c);
            json!({
                "category": category_name(*c),
                "label": c.label(),
                "minutes": minutes,
                "blocks": blocks[i],
            })
        })
        .collect();
    Ok(json!({
        "from": from.map(|d| d.to_string()),
        "to": to.map(|d| d.to_string()),
        "days": days,
        "total_minutes": totals.downtime_minutes(),
        "categories": by_category,
        "rework_blocks": totals.rework_points,
        "rework_minutes": rework_minutes,
    }))
}

pub fn export(ctx: &Context, cmd: &ExportCommand) -> Result<Reply, CliError> {
    let ExportCommand::Csv { from, to, all, out, stdout } = cmd;
    let o = open(ctx, false)?;
    let saved = all_days(&o);
    let (first, last) = if *all {
        match (saved.first(), saved.last()) {
            (Some(a), Some(b)) => (a.0, b.0),
            _ => {
                return Err(CliError::new(crate::output::Exit::NotFound, "no_days", "Nothing has been logged yet."))
            }
        }
    } else {
        let monday = week_start(ctx.date);
        let first = from.as_deref().map(|d| parse_date(d, "--from")).transpose()?.unwrap_or(monday);
        let last = to
            .as_deref()
            .map(|d| parse_date(d, "--to"))
            .transpose()?
            .unwrap_or(monday + Duration::days(6));
        if last < first {
            return Err(CliError::usage("bad_range", "The last date is before the first."));
        }
        (first, last)
    };
    let rows = build_days(first, last, &saved, ctx.now.date, ctx.now.minutes);
    let days_csv = oee_core::export::days_csv(&rows, &o.settings.core);
    let blocks_csv = oee_core::export::blocks_csv(&rows, &o.settings.core);
    let totals_csv = oee_core::export::totals_csv(&rows, &o.settings.core);
    let mut result = json!({
        "from": first.to_string(),
        "to": last.to_string(),
        "days": rows.len(),
        "weekdays_note": "weekdays with no record are holiday rows; weekends only if they have a record",
        "files": Value::Null,
        "csv": Value::Null,
    });
    if let Some(which) = stdout {
        result["csv"] = json!(match which {
            Which::Days => &days_csv,
            Which::Blocks => &blocks_csv,
            Which::Totals => &totals_csv,
        });
    } else if let Some(dir) = out {
        if ctx.dry_run {
            result["files"] = json!({ "written": false });
        } else {
            std::fs::create_dir_all(dir).map_err(|e| data_err("Couldn't create the output folder", e))?;
            let (d, b) = write_files(dir, first, last, &days_csv, &blocks_csv)
                .map_err(|e| data_err("Couldn't write the CSV files", e))?;
            let t = write_totals(dir, first, last, &totals_csv)
                .map_err(|e| data_err("Couldn't write the CSV files", e))?;
            result["files"] = json!({
                "written": true,
                "days": d.display().to_string(),
                "blocks": b.display().to_string(),
                "totals": t.display().to_string(),
            });
        }
    } else {
        return Err(CliError::usage("output_required", "Say where the CSV goes.")
            .hint("Use --out <folder> to write the files, or --stdout days|blocks|totals to get one in the result."));
    }
    Ok(result.into())
}
