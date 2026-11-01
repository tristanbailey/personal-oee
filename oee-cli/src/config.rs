//! `calendar …` and `settings …` commands (docs/CLI_API.md 5.3). They use the
//! same checks as the Calendar and Settings tabs.

use clap::Subcommand;
use oee_core::timeline::snap;
use oee_core::{CalendarEntry, EntryId, PlannedKind, Weekday, WeeklyCalendar};
use oee_store::calendar::{add_entries, DayChoice};
use oee_store::settings::{AppSettings, SettingsForm, Shortcut};
use serde_json::{json, Value};

use crate::day::{data_err, open, Open};
use crate::output::{CliError, Exit, Reply};
use crate::view::{hhmm, planned_name};
use crate::Context;

#[derive(Subcommand, Debug)]
pub enum CalendarCommand {
    /// Entries per weekday, with ids.
    List,
    /// Add a weekly break or meeting.
    Add {
        /// mon … sun, or mon-fri.
        #[arg(long, value_name = "DAY")]
        day: String,
        #[arg(long, value_name = "HH:MM")]
        from: String,
        #[arg(long, value_name = "HH:MM")]
        to: String,
        /// break | meeting
        #[arg(long, value_name = "KIND")]
        kind: String,
        #[arg(long, value_name = "NAME", default_value = "")]
        label: String,
    },
    /// Remove an entry by id.
    Remove {
        #[arg(long)]
        id: u32,
    },
}

#[derive(Subcommand, Debug)]
pub enum SettingsCommand {
    /// Day start, finish, target, shortcut and weekly goal.
    Get,
    /// Change settings, checked like the Settings tab; nothing changes on error.
    Set {
        #[arg(long, value_name = "HH:MM")]
        start: Option<String>,
        #[arg(long, value_name = "HH:MM")]
        end: Option<String>,
        #[arg(long, value_name = "POINTS")]
        target: Option<String>,
        /// A percent from 1 to 100, or off.
        #[arg(long, value_name = "N|off")]
        goal: Option<String>,
        /// For example ctrl+alt+D.
        #[arg(long, value_name = "KEYS")]
        shortcut: Option<String>,
    },
}

impl CalendarCommand {
    pub fn name(&self) -> &'static str {
        match self {
            CalendarCommand::List => "calendar list",
            CalendarCommand::Add { .. } => "calendar add",
            CalendarCommand::Remove { .. } => "calendar remove",
        }
    }
}

impl SettingsCommand {
    pub fn name(&self) -> &'static str {
        match self {
            SettingsCommand::Get => "settings get",
            SettingsCommand::Set { .. } => "settings set",
        }
    }
}

const DAYS: [(Weekday, &str); 7] = [
    (Weekday::Mon, "mon"),
    (Weekday::Tue, "tue"),
    (Weekday::Wed, "wed"),
    (Weekday::Thu, "thu"),
    (Weekday::Fri, "fri"),
    (Weekday::Sat, "sat"),
    (Weekday::Sun, "sun"),
];

fn day_name(d: Weekday) -> &'static str {
    DAYS.iter().find(|(w, _)| *w == d).map_or("", |(_, n)| n)
}

fn entry_json(e: &CalendarEntry, day_start: u32) -> Value {
    let w = &e.window;
    let snapped = snap(w.clone(), day_start);
    let counts_as = (snapped.start != w.start || snapped.end != w.end).then(|| {
        json!({ "from": hhmm(snapped.start), "to": hhmm(snapped.end), "minutes": snapped.end - snapped.start })
    });
    json!({
        "id": e.id.0,
        "day": day_name(e.day),
        "from": hhmm(w.start),
        "to": hhmm(w.end),
        "kind": planned_name(w.kind),
        "label": w.label,
        "counts_as": counts_as,
    })
}

fn calendar_state(calendar: &WeeklyCalendar, day_start: u32) -> Vec<Value> {
    DAYS.iter()
        .flat_map(|(d, _)| calendar.entries_for(*d))
        .map(|e| entry_json(e, day_start))
        .collect()
}

fn save_calendar(ctx: &Context, o: &Open, c: &WeeklyCalendar) -> Result<bool, CliError> {
    if ctx.dry_run {
        return Ok(false);
    }
    o.store.save_calendar(c).map_err(|e| data_err("Couldn't save calendar.json", e))?;
    Ok(true)
}

pub fn run_calendar(ctx: &Context, cmd: &CalendarCommand) -> Result<Reply, CliError> {
    match cmd {
        CalendarCommand::List => {
            let o = open(ctx, false)?;
            Ok(json!({ "entries": calendar_state(&o.calendar, o.settings.core.default_start) }).into())
        }
        CalendarCommand::Add { day, from, to, kind, label } => {
            let choice = DayChoice::parse(day).ok_or_else(|| {
                CliError::usage("bad_day", format!("Unknown day \"{day}\".")).hint("Use mon, tue, wed, thu, fri, sat, sun or mon-fri.")
            })?;
            let kind = match kind.trim().to_ascii_lowercase().as_str() {
                "break" => PlannedKind::Break,
                "meeting" => PlannedKind::Meeting,
                _ => return Err(CliError::usage("bad_kind", format!("Unknown kind \"{kind}\".")).hint("Use break or meeting.")),
            };
            let mut o = open(ctx, true)?;
            let ids = add_entries(&mut o.calendar, choice, from, to, kind, label)
                .map_err(|m| CliError::new(Exit::Rule, "bad_entry", m))?;
            let saved = save_calendar(ctx, &o, &o.calendar)?;
            let day_start = o.settings.core.default_start;
            let added: Vec<Value> = ids
                .iter()
                .filter_map(|id| {
                    DAYS.iter()
                        .flat_map(|(d, _)| o.calendar.entries_for(*d))
                        .find(|e| e.id == *id)
                })
                .map(|e| entry_json(e, day_start))
                .collect();
            Ok(json!({ "added": added, "saved": saved }).into())
        }
        CalendarCommand::Remove { id } => {
            let mut o = open(ctx, true)?;
            let removed = o.calendar.remove(EntryId(*id)).ok_or_else(|| {
                CliError::new(Exit::NotFound, "no_such_entry", format!("No calendar entry has id {id}."))
                    .hint("Run `office-oee calendar list` for the ids.")
            })?;
            let saved = save_calendar(ctx, &o, &o.calendar)?;
            Ok(json!({ "removed": entry_json(&removed, o.settings.core.default_start), "saved": saved }).into())
        }
    }
}

fn shortcut_json(s: &Shortcut) -> Value {
    json!({ "label": s.label(), "ctrl": s.ctrl, "alt": s.alt, "shift": s.shift, "cmd": s.cmd, "key": s.key.to_string() })
}

fn settings_json(s: &AppSettings) -> Value {
    json!({
        "start": hhmm(s.core.default_start),
        "end": hhmm(s.core.default_end),
        "target": s.core.base_target,
        "shortcut": shortcut_json(&s.shortcut),
        "goal": { "enabled": s.goal.enabled, "percent": s.goal.percent },
    })
}

/// "ctrl+alt+D": modifiers then one key.
fn apply_shortcut(form: &mut SettingsForm, text: &str) -> Result<(), CliError> {
    let bad = || CliError::usage("bad_shortcut", format!("Couldn't read the shortcut \"{text}\".")).hint("Use modifiers and one key, for example ctrl+alt+D.");
    let parts: Vec<String> = text.split('+').map(|p| p.trim().to_ascii_lowercase()).collect();
    let (key, mods) = parts.split_last().ok_or_else(bad)?;
    (form.ctrl, form.alt, form.shift, form.cmd) = (false, false, false, false);
    for m in mods {
        match m.as_str() {
            "ctrl" | "control" => form.ctrl = true,
            "alt" | "option" => form.alt = true,
            "shift" => form.shift = true,
            "cmd" | "command" => form.cmd = true,
            _ => return Err(bad()),
        }
    }
    form.key = key.clone();
    Ok(())
}

pub fn run_settings(ctx: &Context, cmd: &SettingsCommand) -> Result<Reply, CliError> {
    match cmd {
        SettingsCommand::Get => {
            let o = open(ctx, false)?;
            Ok(settings_json(&o.settings).into())
        }
        SettingsCommand::Set { start, end, target, goal, shortcut } => {
            let o = open(ctx, true)?;
            let mut form = SettingsForm::from_settings(&o.settings);
            if let Some(v) = start { form.start = v.clone(); }
            if let Some(v) = end { form.end = v.clone(); }
            if let Some(v) = target { form.target = v.clone(); }
            if let Some(g) = goal {
                if g.trim().eq_ignore_ascii_case("off") {
                    form.goal_on = false;
                } else {
                    form.goal_on = true;
                    form.goal = g.clone();
                }
            }
            if let Some(s) = shortcut { apply_shortcut(&mut form, s)?; }
            let new = form.parse().map_err(|m| CliError::new(Exit::Rule, "bad_setting", m))?;
            let saved = if ctx.dry_run {
                false
            } else {
                o.store.save_settings(&new).map_err(|e| data_err("Couldn't save settings.json", e))?;
                true
            };
            let mut v = settings_json(&new);
            v["saved"] = json!(saved);
            Ok(v.into())
        }
    }
}
