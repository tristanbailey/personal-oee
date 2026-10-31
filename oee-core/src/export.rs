//! CSV export (D-51): a days file with one row per day, and a blocks file with
//! one row per block.
//!
//! Plain text out, no I/O. The caller says which days to list and how far each
//! one has got, so this module needs no clock or calendar. Numbers are plain
//! (0.8750, not "87.5%"), dates are ISO and times are HH:MM, so a spreadsheet
//! reads them the same in any locale.

use crate::{
    day_result, BlockKind, DayRecord, DayStatus, LostCategory, PlannedKind, Settings,
};

/// One day to export.
#[derive(Debug, Clone)]
pub struct ExportDay {
    /// ISO date, e.g. "2026-10-26".
    pub date: String,
    /// `None` for a holiday: nothing logged.
    pub record: Option<DayRecord>,
    /// Count only time and blocks finished by this minute. Use the record's
    /// end for a finished day, and "now" for today (add a running stop to the
    /// record first).
    pub until: u32,
}

const EOL: &str = "\r\n";

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// RFC 4180 quoting, plus a leading `'` on text a spreadsheet would run as a
/// formula (starts with `=`, `+`, `-`, `@`, tab or carriage return).
fn text(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn snake(c: LostCategory) -> &'static str {
    match c {
        LostCategory::SystemDown => "system_down",
        LostCategory::ChangeOver => "change_over",
        LostCategory::Idle => "idle",
        LostCategory::Management => "management",
        LostCategory::Wait => "wait",
    }
}

pub fn days_csv(days: &[ExportDay], settings: &Settings) -> String {
    let mut out = String::from(
        "date,status,start,end,day_minutes,planned_minutes,lost_minutes,work_points,\
         rework_points,target_points,availability,efficiency,quality,oee,over_capacity",
    );
    for c in LostCategory::ALL {
        out.push_str(&format!(",{}_minutes", snake(c)));
    }
    out.push_str(EOL);

    for d in days {
        let inputs = d.record.as_ref().map(|r| r.inputs_until(settings, d.until));
        let result = day_result(inputs.as_ref());
        let status = match result.status {
            DayStatus::Holiday => "holiday",
            DayStatus::ZeroWork => "zero_work",
            DayStatus::Worked => "worked",
        };
        let (start, end) = d
            .record
            .as_ref()
            .map_or((String::new(), String::new()), |r| (hhmm(r.start), hhmm(r.end)));
        let m = result.metrics;
        let i = inputs.unwrap_or_default();
        out.push_str(&format!(
            "{},{status},{start},{end},{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{}",
            d.date,
            i.day_minutes,
            i.planned_minutes,
            i.losses.total(),
            i.work_points,
            i.rework_points,
            i.target_points,
            m.availability,
            m.efficiency,
            m.quality,
            m.oee,
            m.over_capacity,
        ));
        for c in LostCategory::ALL {
            out.push_str(&format!(",{}", i.losses.get(c)));
        }
        out.push_str(EOL);
    }
    out
}

pub fn blocks_csv(days: &[ExportDay], _settings: &Settings) -> String {
    let mut out = format!("date,block_start,block_end,type,category,rework,note,uncategorised{EOL}");
    for d in days {
        let Some(record) = &d.record else { continue };
        for b in record.blocks_until(d.until) {
            let (ty, category, rework, uncategorised) = match b.kind {
                BlockKind::Work { rework } => ("work", "", rework, false),
                BlockKind::Lost { category } => (
                    "lost",
                    // FR-027: uncategorised counts as System Down.
                    category.unwrap_or(LostCategory::SystemDown).label(),
                    false,
                    category.is_none(),
                ),
                BlockKind::Planned(PlannedKind::Break) => ("break", "", false, false),
                BlockKind::Planned(PlannedKind::Meeting) => ("meeting", "", false, false),
            };
            out.push_str(&format!(
                "{},{},{},{ty},{category},{rework},{},{uncategorised}{EOL}",
                d.date,
                hhmm(b.start),
                hhmm(b.end),
                text(b.note.as_deref().unwrap_or("")),
            ));
        }
    }
    out
}
