//! CSV export (D-51): a days file with one row per day, and a blocks file with
//! one row per block. D-60 adds a totals file in the layout of the owner's own
//! tracking spreadsheet.
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

/// D-60: one row per day in the layout of the owner's tracking spreadsheet, so
/// a range can be pasted straight in. Minutes are counted minutes (Change Over
/// at half a cycle, FR-015). Total downtime is the five loss columns added up;
/// Planned Non-Productive (breaks and meetings) is its own column and isn't
/// added in. Cycle time is each day's own (D-61). A holiday is a row of zeros.
pub fn totals_csv(days: &[ExportDay], settings: &Settings) -> String {
    let mut out = format!(
        "Start Date,Week num,Total downtime,Total shift,Planned Non-Productive,System down,\
         Change over,Idle,Manage,Wait,Total points,Rework points,Cycle time{EOL}"
    );
    for d in days {
        let (label, week) = date_label(&d.date).unwrap_or_else(|| (d.date.clone(), 0));
        let i = d
            .record
            .as_ref()
            .map(|r| r.inputs_until(settings, d.until))
            .unwrap_or_default();
        let cycle = d.record.as_ref().map_or(0, |r| r.cycle_minutes);
        out.push_str(&format!(
            "{},{week},{},{},{}",
            text(&label),
            i.losses.total(),
            i.day_minutes,
            i.planned_minutes,
        ));
        for c in LostCategory::ALL {
            out.push_str(&format!(",{}", i.losses.get(c)));
        }
        out.push_str(&format!(",{},{},{cycle}{EOL}", i.work_points, i.rework_points));
    }
    out
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "2025-08-18" → ("Aug 18", 34): the spreadsheet's date and its ISO 8601
/// week number. `None` if the date isn't YYYY-MM-DD.
pub fn date_label(iso: &str) -> Option<(String, u32)> {
    let mut parts = iso.splitn(3, '-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some((format!("{} {d}", MONTHS[m as usize - 1]), iso_week(y, m, d)))
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// 1 for 1 January.
fn day_of_year(y: i32, m: u32, d: u32) -> u32 {
    (1..m).map(|mm| days_in_month(y, mm)).sum::<u32>() + d
}

/// 1 = Monday … 7 = Sunday (Sakamoto's method).
fn weekday(y: i32, m: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let sun0 = (y + y / 4 - y / 100 + y / 400 + T[m as usize - 1] + d as i32).rem_euclid(7);
    if sun0 == 0 { 7 } else { sun0 as u32 }
}

/// Weeks in an ISO year: 53 when it starts on a Thursday, or on a Wednesday in a leap year.
fn iso_weeks_in(y: i32) -> u32 {
    let jan1 = weekday(y, 1, 1);
    if jan1 == 4 || (jan1 == 3 && is_leap(y)) { 53 } else { 52 }
}

fn iso_week(y: i32, m: u32, d: u32) -> u32 {
    let w = (day_of_year(y, m, d) as i32 - weekday(y, m, d) as i32 + 10) / 7;
    if w < 1 {
        iso_weeks_in(y - 1)
    } else if w as u32 > iso_weeks_in(y) {
        1
    } else {
        w as u32
    }
}
