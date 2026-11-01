//! The Week tab (D-34 onwards): navigation, then pooled figures and the table.

use chrono::{Datelike, Duration, NaiveDate};
use eframe::egui::{self, Color32, RichText};
use oee_core::{
    day_result, over_capacity_days, week_report, DayInputs, DayRecord, DayStatus, LostCategory, Metrics, Settings,
    WeekReport, BLOCK_MINUTES,
};

use crate::theme;
use crate::tiles::{pct, week_tiles, Tile};

/// The Monday of the week containing `date`.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
}

pub fn prev_week(monday: NaiveDate) -> NaiveDate {
    monday - Duration::days(7)
}

pub fn next_week(monday: NaiveDate) -> NaiveDate {
    monday + Duration::days(7)
}

/// "19 Oct – 25 Oct" for the week starting on `monday`.
pub fn title(monday: NaiveDate) -> String {
    let sunday = monday + Duration::days(6);
    format!("{} – {}", monday.format("%-d %b"), sunday.format("%-d %b"))
}

/// FR-051, FR-052: how many days are pooled. Holidays (nothing logged) are
/// left out.
pub fn subline(working_days: usize) -> String {
    match working_days {
        0 => "No working days this week".into(),
        1 => "Pooled from 1 working day, holidays left out".into(),
        n => format!("Pooled from {n} working days, holidays left out"),
    }
}

/// A day with a saved record is a working day.
pub fn working_days(days: &[(NaiveDate, Option<DayRecord>)]) -> usize {
    days.iter().filter(|(_, r)| r.is_some()).count()
}

/// One day's totals. Today's unfinished day counts only what has happened by
/// `now`, including a stop that is still running; other days count in full.
pub(crate) fn day_inputs(record: &DayRecord, is_today: bool, now: u32, settings: &Settings) -> DayInputs {
    if !is_today {
        return record.inputs(settings);
    }
    let mut live = record.clone();
    if let Some(since) = live.down_since {
        live.add_downtime(since, now);
    }
    live.inputs_until(settings, now)
}

/// FR-051 to FR-053: the pooled report for the week's saved days.
pub fn report(
    days: &[(NaiveDate, Option<DayRecord>)],
    today: NaiveDate,
    now: u32,
    settings: &Settings,
) -> WeekReport {
    let inputs: Vec<Option<DayInputs>> = days
        .iter()
        .map(|(date, r)| r.as_ref().map(|r| day_inputs(r, *date == today, now, settings)))
        .collect();
    week_report(inputs.iter().map(|i| i.as_ref()))
}

/// The status pill on a table row (D-36).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pill {
    Worked,
    OverCapacity,
    ZeroDay,
    Holiday,
}

impl Pill {
    pub fn label(self) -> &'static str {
        match self {
            Pill::Worked => "Worked",
            Pill::OverCapacity => "Over capacity",
            Pill::ZeroDay => "0% day",
            Pill::Holiday => "Holiday",
        }
    }

    /// The pill for a day's result (FR-045, FR-045a, FR-048).
    pub fn of(result: &oee_core::DayResult) -> Pill {
        match result.status {
            DayStatus::Holiday => Pill::Holiday,
            DayStatus::ZeroWork => Pill::ZeroDay,
            DayStatus::Worked if result.metrics.over_capacity => Pill::OverCapacity,
            DayStatus::Worked => Pill::Worked,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) {
        let p = theme::of(ui);
        let (fill, edge) = self.colours(&p);
        egui::Frame::none()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0_f32, edge))
            .rounding(10.0)
            .inner_margin(egui::Margin::symmetric(8.0, 1.0))
            .show(ui, |ui| {
                ui.label(RichText::new(self.label()).small().color(p.ink));
            });
    }

    /// (fill, outline) from the prototype's status colours.
    fn colours(self, p: &theme::Palette) -> (Color32, Color32) {
        match self {
            Pill::Worked => (p.work_soft, p.work),
            Pill::OverCapacity => (p.plan_soft, p.plan),
            Pill::ZeroDay => (p.down_soft, p.down),
            Pill::Holiday => (p.surface, p.muted),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub date: Option<NaiveDate>,
    pub label: String,
    pub pill: Option<Pill>,
    pub availability: String,
    pub efficiency: String,
    pub quality: String,
    pub oee: String,
    /// "18/20": Work points over target.
    pub points: String,
}

fn row_values(m: &Metrics) -> [String; 4] {
    [pct(m.availability), pct(m.efficiency), pct(m.quality), pct(m.oee)]
}

/// Mon to Fri always; Sat and Sun only when they have a record.
fn shown(i: usize, record: &Option<DayRecord>) -> bool {
    i < 5 || record.is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarKind {
    Lost,
    ChangeOver,
    Rework,
}

/// One bar in the loss breakdown (D-37).
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub label: &'static str,
    /// Shown on the right: minutes for a category, blocks for Rework.
    pub value: String,
    /// 0.0 to 1.0, scaled to the largest bar.
    pub fraction: f32,
    pub kind: BarKind,
}

/// FR-042: minutes lost per category, then the Rework block count. Category bars
/// are scaled to the largest; the Rework bar counts each block as 20 minutes so
/// it sits on the same scale.
pub fn loss_bars(totals: &DayInputs) -> Vec<Bar> {
    let largest = LostCategory::ALL
        .iter()
        .map(|c| totals.losses.get(*c))
        .max()
        .unwrap_or(0)
        .max(totals.rework_points * BLOCK_MINUTES);
    let scale = |minutes: u32| {
        if largest == 0 {
            0.0
        } else {
            minutes as f32 / largest as f32
        }
    };
    let mut bars: Vec<Bar> = LostCategory::ALL
        .iter()
        .map(|c| {
            let minutes = totals.losses.get(*c);
            Bar {
                label: c.label(),
                value: format!("{minutes} min"),
                fraction: scale(minutes),
                kind: if *c == LostCategory::ChangeOver {
                    BarKind::ChangeOver
                } else {
                    BarKind::Lost
                },
            }
        })
        .collect();
    let rework = totals.rework_points;
    bars.push(Bar {
        label: "Rework",
        value: format!("{rework} block{}", if rework == 1 { "" } else { "s" }),
        fraction: scale(rework * BLOCK_MINUTES),
        kind: BarKind::Rework,
    });
    bars
}

pub fn show_loss_bars(ui: &mut egui::Ui, bars: &[Bar]) {
    ui.label(RichText::new("Where time was lost").strong());
    ui.add_space(4.0);
    for bar in bars {
        let p = theme::of(ui);
        let colour = match bar.kind {
            BarKind::Lost => p.down,
            BarKind::ChangeOver => p.change,
            BarKind::Rework => p.rework,
        };
        ui.horizontal(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(bar.label));
            let track = (ui.available_width() - 90.0).max(40.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(track, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, p.line);
            if bar.fraction > 0.0 {
                let mut filled = rect;
                filled.set_width((track * bar.fraction).max(3.0));
                ui.painter().rect_filled(filled, 4.0, colour);
            }
            ui.monospace(&bar.value);
        });
    }
}

/// One flagged day in the review list (D-38).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRow {
    pub date: NaiveDate,
    /// "Tue 20 Oct".
    pub label: String,
    /// "26/20".
    pub points: String,
    /// "130.0%".
    pub efficiency: String,
}

pub const NO_OVER_CAPACITY: &str = "No over-capacity days yet.";

/// FR-043: every saved day with Efficiency over 100%, newest first.
pub fn over_capacity_rows(
    all_days: &[(NaiveDate, DayRecord)],
    today: NaiveDate,
    now: u32,
    settings: &Settings,
) -> Vec<ReviewRow> {
    let scored: Vec<(&(NaiveDate, DayRecord), _, _)> = all_days
        .iter()
        .map(|d| {
            let inputs = day_inputs(&d.1, d.0 == today, now, settings);
            (d, day_result(Some(&inputs)), inputs)
        })
        .collect();
    let pairs: Vec<(usize, oee_core::DayResult)> = scored
        .iter()
        .enumerate()
        .map(|(i, (_, r, _))| (i, *r))
        .collect();
    let mut rows: Vec<ReviewRow> = over_capacity_days(&pairs)
        .into_iter()
        .map(|&i| {
            let ((date, _), result, inputs) = &scored[i];
            ReviewRow {
                date: *date,
                label: date.format("%a %-d %b").to_string(),
                points: format!("{}/{}", inputs.work_points, inputs.target_points),
                efficiency: pct(result.metrics.efficiency),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.date.cmp(&a.date));
    rows
}

/// Draw the review list. Returns the date of a Review button that was pressed.
pub fn show_review(ui: &mut egui::Ui, rows: &[ReviewRow]) -> Option<NaiveDate> {
    let mut clicked = None;
    ui.label(RichText::new("Over-capacity days to review").strong());
    ui.add_space(4.0);
    if rows.is_empty() {
        ui.label(RichText::new(NO_OVER_CAPACITY).weak());
        return None;
    }
    egui::Grid::new("over_capacity")
        .striped(true)
        .num_columns(4)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            for h in ["Day", "Points", "Efficiency", ""] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for row in rows {
                ui.label(RichText::new(&row.label).monospace());
                ui.label(RichText::new(&row.points).monospace());
                ui.label(RichText::new(&row.efficiency).monospace());
                if ui.button("Review →").clicked() {
                    clicked = Some(row.date);
                }
                ui.end_row();
            }
        });
    clicked
}

/// Everything the Week tab shows below the navigation.
#[derive(Debug, Clone, PartialEq)]
pub struct WeekView {
    pub working_days: usize,
    pub tiles: [Tile; 4],
    pub rows: Vec<TableRow>,
    pub total: TableRow,
    pub losses: Vec<Bar>,
}

impl WeekView {
    pub fn build(
        days: &[(NaiveDate, Option<DayRecord>)],
        today: NaiveDate,
        now: u32,
        settings: &Settings,
    ) -> WeekView {
        let rep = report(days, today, now, settings);
        let rows = days
            .iter()
            .enumerate()
            .filter(|(i, (_, r))| shown(*i, r))
            .map(|(_, (date, record))| {
                let inputs = record
                    .as_ref()
                    .map(|r| day_inputs(r, *date == today, now, settings));
                let result = day_result(inputs.as_ref());
                let pill = Pill::of(&result);
                let [a, e, q, o] = row_values(&result.metrics);
                TableRow {
                    date: Some(*date),
                    label: date.format("%a %-d").to_string(),
                    pill: Some(pill),
                    availability: a,
                    efficiency: e,
                    quality: q,
                    oee: o,
                    points: match &inputs {
                        Some(i) => format!("{}/{}", i.work_points, i.target_points),
                        None => "–".into(),
                    },
                }
            })
            .collect();
        let [a, e, q, o] = row_values(&rep.metrics);
        WeekView {
            working_days: working_days(days),
            losses: loss_bars(&rep.totals),
            tiles: week_tiles(&rep),
            rows,
            total: TableRow {
                date: None,
                label: "Week total".into(),
                pill: None,
                availability: a,
                efficiency: e,
                quality: q,
                oee: o,
                points: format!("{}/{}", rep.totals.work_points, rep.totals.target_points),
            },
        }
    }
}

/// Draw the table. Returns the date of a row that was clicked (D-36, D-39).
pub fn show_table(ui: &mut egui::Ui, view: &WeekView) -> Option<NaiveDate> {
    let mut clicked = None;
    egui::Grid::new("week_table")
        .striped(true)
        .num_columns(7)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            for h in ["Day", "Status", "Avail.", "Effic.", "Quality", "OEE", "Points"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for row in view.rows.iter().chain(std::iter::once(&view.total)) {
                let total = row.date.is_none();
                let text = |s: &str| {
                    let t = RichText::new(s).monospace();
                    if total { t.strong() } else { t }
                };
                let label = ui
                    .add(egui::Label::new(text(&row.label)).sense(egui::Sense::click()))
                    .on_hover_cursor(if total {
                        egui::CursorIcon::Default
                    } else {
                        egui::CursorIcon::PointingHand
                    });
                if label.clicked() && row.date.is_some() {
                    clicked = row.date;
                }
                match row.pill {
                    Some(p) => p.show(ui),
                    None => {
                        ui.label("");
                    }
                }
                for v in [&row.availability, &row.efficiency, &row.quality, &row.oee, &row.points] {
                    ui.label(text(v));
                }
                ui.end_row();
            }
        });
    clicked
}

/// Draw the navigation. Returns the week to show if a button was pressed.
pub fn show_nav(ui: &mut egui::Ui, monday: NaiveDate, working: usize) -> Option<NaiveDate> {
    let mut go = None;
    ui.horizontal(|ui| {
        if ui.button("‹").clicked() {
            go = Some(prev_week(monday));
        }
        ui.heading(title(monday));
        if ui.button("›").clicked() {
            go = Some(next_week(monday));
        }
    });
    ui.label(subline(working));
    go
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn full_day() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    #[test]
    fn the_report_pools_minutes_and_points_across_days() {
        let mut slow = full_day();
        slow.add_downtime(hm(10, 0), hm(10, 25));
        let days = vec![
            (d(10, 26), Some(full_day())),
            (d(10, 27), None),
            (d(10, 28), Some(slow)),
        ];
        let r = report(&days, d(11, 5), hm(12, 0), &Settings::default());
        assert_eq!(r.days_counted, 2);
        assert_eq!(r.totals.day_minutes, 1040);
        assert_eq!(r.totals.work_points, 26 + 25);
        assert_eq!(r.totals.target_points, 40);
        assert_eq!(r.totals.downtime_minutes(), 20);
    }

    #[test]
    fn a_week_of_holidays_is_all_zeros() {
        let days = vec![(d(10, 26), None), (d(10, 27), None)];
        let r = report(&days, d(10, 26), hm(12, 0), &Settings::default());
        assert_eq!(r.days_counted, 0);
        assert_eq!(r.metrics.oee, 0.0);
        let tiles = crate::tiles::week_tiles(&r);
        assert_eq!(tiles[3].value, "0.0%");
        assert_eq!(tiles[3].caption, "Week, pooled");
    }

    #[test]
    fn todays_unfinished_day_only_counts_what_has_happened() {
        let today = d(10, 29);
        let mut rec = full_day();
        rec.down_since = Some(hm(10, 5));
        let days = vec![(today, Some(rec.clone()))];
        // At 10:30 four blocks have finished (09:00 to 10:20), and a 25-minute stop
        // that is still running has taken one of them: 3 Work points, 1 lost block.
        let r = report(&days, today, hm(10, 30), &Settings::default());
        assert_eq!(r.totals.day_minutes, 90);
        assert_eq!(r.totals.work_points, 3);
        assert_eq!(r.totals.downtime_minutes(), 20);
        // The same record on another day counts in full.
        let r = report(&days, d(11, 5), hm(10, 30), &Settings::default());
        assert_eq!(r.totals.day_minutes, 520);
        assert_eq!(r.totals.work_points, 26);
    }

    fn build(days: &[(NaiveDate, Option<DayRecord>)]) -> WeekView {
        WeekView::build(days, d(12, 31), hm(12, 0), &Settings::default())
    }

    /// A full Mon..Sun week starting 26 Oct with the given records.
    fn week(records: [Option<DayRecord>; 7]) -> Vec<(NaiveDate, Option<DayRecord>)> {
        records
            .into_iter()
            .enumerate()
            .map(|(i, r)| (d(10, 26) + Duration::days(i as i64), r))
            .collect()
    }

    #[test]
    fn mon_to_fri_always_show_and_weekends_only_with_a_record() {
        let none = [None, None, None, None, None, None, None];
        let v = build(&week(none));
        assert_eq!(v.rows.len(), 5);
        assert!(v.rows.iter().all(|r| r.pill == Some(Pill::Holiday)));
        assert_eq!(v.rows[0].label, "Mon 26");

        let v = build(&week([None, None, None, None, None, Some(full_day()), None]));
        assert_eq!(v.rows.len(), 6);
        assert_eq!(v.rows[5].label, "Sat 31");
    }

    #[test]
    fn pills_follow_the_day_status() {
        let mut zero = full_day();
        zero.add_downtime(hm(9, 0), hm(17, 40)); // every block lost
        let v = build(&week([
            Some(full_day()), // 26 of 20 points: over capacity
            Some(zero),
            None,
            None,
            None,
            None,
            None,
        ]));
        assert_eq!(v.rows[0].pill, Some(Pill::OverCapacity));
        assert_eq!(v.rows[1].pill, Some(Pill::ZeroDay));
        assert_eq!(v.rows[2].pill, Some(Pill::Holiday));
        assert_eq!(v.rows[0].points, "26/20");
        assert_eq!(v.rows[1].points, "0/20");
        assert_eq!(v.rows[2].points, "–");
        assert_eq!(v.rows[2].oee, "0.0%");
    }

    #[test]
    fn a_normal_day_is_worked_and_the_total_row_pools() {
        let mut ok = full_day();
        // Lose 6 blocks so 20 of 20 points remain.
        for h in [(10, 0), (11, 0)] {
            ok.add_downtime(hm(h.0, h.1), hm(h.0, h.1 + 40) + 20);
        }
        let v = build(&week([Some(ok), None, None, None, None, None, None]));
        assert_eq!(v.rows[0].pill, Some(Pill::Worked));
        assert_eq!(v.rows[0].points, v.total.points);
        assert_eq!(v.total.label, "Week total");
        assert_eq!(v.total.date, None);
        assert_eq!(v.working_days, 1);
        assert_eq!(v.tiles[3].caption, "Week, pooled");
    }

    #[test]
    fn loss_bars_are_scaled_to_the_largest_and_change_over_is_amber() {
        let mut r = full_day();
        r.add_downtime(hm(10, 0), hm(10, 25));
        r.categorise(hm(10, 0), LostCategory::Idle).unwrap();
        r.add_downtime(hm(11, 0), hm(11, 25));
        r.categorise(hm(11, 0), LostCategory::ChangeOver).unwrap();
        r.set_rework(hm(9, 0), true).unwrap();
        let v = build(&week([Some(r), None, None, None, None, None, None]));
        let by = |label| v.losses.iter().find(|b| b.label == label).unwrap();
        let idle = by("Idle");
        assert_eq!((idle.value.as_str(), idle.fraction, idle.kind), ("20 min", 1.0, BarKind::Lost));
        let co = by("Change Over");
        // A Change Over block counts as 10 minutes (FR-015): half of Idle's 20.
        assert_eq!((co.value.as_str(), co.fraction, co.kind), ("10 min", 0.5, BarKind::ChangeOver));
        assert_eq!(by("Wait").fraction, 0.0);
        let rework = by("Rework");
        assert_eq!((rework.value.as_str(), rework.fraction), ("1 block", 1.0));
        // Five categories, then Rework last.
        assert_eq!(v.losses.len(), 6);
        assert_eq!(v.losses[5].label, "Rework");
    }

    #[test]
    fn loss_bars_for_an_empty_week_are_all_zero() {
        let v = build(&week([None, None, None, None, None, None, None]));
        assert!(v.losses.iter().all(|b| b.fraction == 0.0));
        assert_eq!(v.losses[0].value, "0 min");
        assert_eq!(v.losses[5].value, "0 blocks");
    }

    #[test]
    fn the_review_list_has_only_flagged_days_newest_first() {
        let mut ok = full_day();
        for h in [(10, 0), (11, 0)] {
            ok.add_downtime(hm(h.0, h.1), hm(h.0 + 1, h.1)); // 3 lost blocks each
        }
        let all = vec![
            (d(10, 20), full_day()), // 26/20: flagged
            (d(10, 21), ok),         // 20/20: not flagged
            (d(10, 27), full_day()), // flagged
        ];
        let rows = over_capacity_rows(&all, d(11, 5), hm(12, 0), &Settings::default());
        let got: Vec<_> = rows.iter().map(|r| (r.label.as_str(), r.points.as_str(), r.efficiency.as_str())).collect();
        assert_eq!(
            got,
            vec![("Tue 27 Oct", "26/20", "130.0%"), ("Tue 20 Oct", "26/20", "130.0%")]
        );
        assert_eq!(rows[0].date, d(10, 27));
    }

    #[test]
    fn nothing_flagged_gives_an_empty_list() {
        let all = vec![(d(10, 21), DayRecord::new(hm(9, 0), hm(9, 20), vec![]))];
        // 1 of 20 points is far from over capacity.
        assert!(over_capacity_rows(&all, d(11, 5), hm(12, 0), &Settings::default()).is_empty());
        assert!(over_capacity_rows(&[], d(11, 5), hm(12, 0), &Settings::default()).is_empty());
        assert_eq!(NO_OVER_CAPACITY, "No over-capacity days yet.");
    }

    #[test]
    fn weeks_start_on_monday() {
        assert_eq!(week_start(d(10, 29)), d(10, 26)); // Thursday
        assert_eq!(week_start(d(10, 26)), d(10, 26)); // Monday
        assert_eq!(week_start(d(11, 1)), d(10, 26)); // Sunday
    }

    #[test]
    fn titles_match_the_example_and_cross_months() {
        assert_eq!(title(d(10, 19)), "19 Oct – 25 Oct");
        assert_eq!(title(d(10, 26)), "26 Oct – 1 Nov");
    }

    #[test]
    fn arrows_move_by_a_week() {
        assert_eq!(prev_week(d(10, 26)), d(10, 19));
        assert_eq!(next_week(d(10, 26)), d(11, 2));
        assert_eq!(week_start(next_week(d(10, 26))), next_week(d(10, 26)));
    }

    #[test]
    fn the_subline_counts_working_days() {
        assert_eq!(subline(0), "No working days this week");
        assert_eq!(subline(1), "Pooled from 1 working day, holidays left out");
        assert_eq!(subline(4), "Pooled from 4 working days, holidays left out");
    }

    #[test]
    fn only_days_with_a_record_are_working_days() {
        let rec = || Some(DayRecord::new(hm(9, 0), hm(17, 40), vec![]));
        let days = vec![(d(10, 26), rec()), (d(10, 27), None), (d(10, 28), rec())];
        assert_eq!(working_days(&days), 2);
        assert_eq!(working_days(&[]), 0);
    }
}
