//! Core values as JSON: names, times and numbers an agent can rely on.
//!
//! Times of day are `"HH:MM"`. Fractions (Availability, Efficiency, Quality,
//! OEE) are numbers where 1.0 is 100%, each with a `display` string to one
//! decimal place, like the app.

use oee_core::{
    Block, BlockKind, DayInputs, DayRecord, DayResult, DayStatus, LostCategory, Metrics, Mode,
    Notice, PlannedKind,
};
use serde_json::{json, Value};

pub fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub fn category_name(c: LostCategory) -> &'static str {
    match c {
        LostCategory::SystemDown => "system-down",
        LostCategory::ChangeOver => "change-over",
        LostCategory::Idle => "idle",
        LostCategory::Management => "management",
        LostCategory::Wait => "wait",
    }
}

pub fn planned_name(k: PlannedKind) -> &'static str {
    match k {
        PlannedKind::Break => "break",
        PlannedKind::Meeting => "meeting",
    }
}

fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn frac(x: f64) -> Value {
    // Rounded so the JSON doesn't carry float noise.
    json!((x * 1e6).round() / 1e6)
}

pub fn metrics_json(m: &Metrics) -> Value {
    json!({
        "availability": frac(m.availability),
        "efficiency": frac(m.efficiency),
        "quality": frac(m.quality),
        "oee": frac(m.oee),
        "over_capacity": m.over_capacity,
        "display": {
            "availability": pct(m.availability),
            "efficiency": pct(m.efficiency),
            "quality": pct(m.quality),
            "oee": pct(m.oee),
        },
    })
}

pub fn status_name(s: DayStatus) -> &'static str {
    match s {
        DayStatus::Holiday => "holiday",
        DayStatus::ZeroWork => "zero-work",
        DayStatus::Worked => "worked",
    }
}

pub fn inputs_json(i: &DayInputs) -> Value {
    json!({
        "day_minutes": i.day_minutes,
        "planned_minutes": i.planned_minutes,
        "downtime_minutes": i.downtime_minutes(),
        "work_points": i.work_points,
        "rework_points": i.rework_points,
        "target_points": i.target_points,
        "losses": LostCategory::ALL.iter().map(|c| (category_name(*c).to_string(), json!(i.losses.get(*c)))).collect::<serde_json::Map<_, _>>(),
    })
}

pub fn result_json(r: &DayResult) -> Value {
    let mut v = metrics_json(&r.metrics);
    v["status"] = json!(status_name(r.status));
    v
}

pub fn block_json(b: &Block) -> Value {
    let (kind, rework, category, planned) = match b.kind {
        BlockKind::Work { rework } => ("work", rework, None, None),
        BlockKind::Lost { category } => ("lost", false, category, None),
        BlockKind::Planned(k) => ("planned", false, None, Some(k)),
    };
    json!({
        "start": hhmm(b.start),
        "end": hhmm(b.end),
        "kind": kind,
        "rework": rework,
        "category": category.map(category_name),
        "planned_kind": planned.map(planned_name),
        "uncategorised": kind == "lost" && category.is_none(),
        "note": b.note,
    })
}

pub fn mode_json(m: Mode) -> Value {
    match m {
        Mode::Work => json!({ "kind": "work" }),
        Mode::Downtime { since } => json!({ "kind": "downtime", "since": hhmm(since) }),
        Mode::Planned { kind, until } => {
            json!({ "kind": "planned", "planned_kind": planned_name(kind), "until": hhmm(until) })
        }
        Mode::Ended => json!({ "kind": "ended" }),
    }
}

pub fn notice_json(n: &Notice) -> Value {
    match n {
        Notice::PlannedStarted { kind, label } => {
            json!({ "type": "planned_started", "planned_kind": planned_name(*kind), "label": label })
        }
        Notice::PlannedEnded => json!({ "type": "planned_ended" }),
        Notice::CategoriseReminder { blocks } => json!({ "type": "categorise_reminder", "blocks": blocks }),
        Notice::DayEnded => json!({ "type": "day_ended" }),
    }
}

pub fn record_json(r: &DayRecord, blocks: &[Block]) -> Value {
    json!({
        "start": hhmm(r.start),
        "end": hhmm(r.end),
        "finalised": r.finalised,
        "planned": r.planned.iter().map(|w| json!({
            "start": hhmm(w.start), "end": hhmm(w.end),
            "kind": planned_name(w.kind), "label": w.label,
        })).collect::<Vec<_>>(),
        "downtime": r.downtime.iter().map(|d| json!({ "start": hhmm(d.start), "end": hhmm(d.end) })).collect::<Vec<_>>(),
        "blocks": blocks.iter().map(block_json).collect::<Vec<_>>(),
    })
}
