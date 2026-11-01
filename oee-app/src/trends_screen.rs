//! The trend chart on the Week tab (D-55): weekly OEE, Availability,
//! Efficiency and Quality over the last 4, 12 or 26 weeks.

use chrono::{Duration, NaiveDate};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, Vec2};
use oee_core::{weekly_series, DayRecord, Settings, WeekPoint};

use crate::theme;
use crate::tiles::pct;
use crate::week_screen::{day_inputs, title, week_start};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Weeks4,
    Weeks12,
    Weeks26,
}

impl Range {
    pub const ALL: [Range; 3] = [Range::Weeks4, Range::Weeks12, Range::Weeks26];

    pub fn weeks(self) -> u32 {
        match self {
            Range::Weeks4 => 4,
            Range::Weeks12 => 12,
            Range::Weeks26 => 26,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Range::Weeks4 => "4 weeks",
            Range::Weeks12 => "12 weeks",
            Range::Weeks26 => "26 weeks",
        }
    }
}

/// The Monday of the oldest week shown, for a chart ending at `current_monday`.
pub fn first_monday(current_monday: NaiveDate, range: Range) -> NaiveDate {
    current_monday - Duration::days(7 * i64::from(range.weeks() - 1))
}

/// FR-051 to FR-053: one pooled point per working week in the range. Today
/// counts up to `now`. Weeks with nothing logged have no point.
pub fn series(
    all_days: &[(NaiveDate, DayRecord)],
    today: NaiveDate,
    now: u32,
    settings: &Settings,
    range: Range,
) -> Vec<WeekPoint<NaiveDate>> {
    let first = first_monday(week_start(today), range);
    weekly_series(
        all_days
            .iter()
            .filter(|(date, _)| week_start(*date) >= first && *date <= today)
            .map(|(date, r)| (week_start(*date), Some(day_inputs(r, *date == today, now, settings)))),
    )
}

/// The top of the chart: 100%, or higher in steps of 25% when Efficiency
/// goes over capacity.
pub fn y_max(points: &[WeekPoint<NaiveDate>]) -> f64 {
    let top = points.iter().map(|p| p.metrics.efficiency).fold(1.0_f64, f64::max);
    (top * 4.0).ceil() / 4.0
}

/// What the hover tooltip says about a week.
pub fn tooltip(p: &WeekPoint<NaiveDate>) -> String {
    let days = if p.days_counted == 1 {
        "1 working day".to_string()
    } else {
        format!("{} working days", p.days_counted)
    };
    let m = &p.metrics;
    let mut text = format!(
        "{} · {days}\nOEE {}\nAvailability {}\nEfficiency {}\nQuality {}",
        title(p.week),
        pct(m.oee),
        pct(m.availability),
        pct(m.efficiency),
        pct(m.quality),
    );
    if m.over_capacity {
        text.push_str("\nOver capacity");
    }
    text
}

/// Which point a pointer at `x` is nearest to, if within half a week's width.
fn nearest(xs: &[f32], x: f32, step: f32) -> Option<usize> {
    xs.iter()
        .enumerate()
        .map(|(i, px)| (i, (px - x).abs()))
        .filter(|(_, d)| *d <= step / 2.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

const EMPTY: &str = "No working weeks in this range yet.";

/// Draw the heading, range buttons and chart.
pub fn show(ui: &mut egui::Ui, range: &mut Range, points: &[WeekPoint<NaiveDate>], today: NaiveDate) {
    ui.horizontal(|ui| {
        ui.heading("Trends");
        for r in Range::ALL {
            if ui.selectable_label(*range == r, r.label()).clicked() {
                *range = r;
            }
        }
    });
    if points.is_empty() {
        ui.label(EMPTY);
        return;
    }
    let pal = theme::of(ui);
    let lines: [(&str, Color32, f32, fn(&oee_core::Metrics) -> f64); 4] = [
        ("OEE", pal.ink, 2.5, |m| m.oee),
        ("Availability", pal.work, 1.5, |m| m.availability),
        ("Efficiency", pal.plan, 1.5, |m| m.efficiency),
        ("Quality", pal.rework, 1.5, |m| m.quality),
    ];

    ui.horizontal_wrapped(|ui| {
        for (name, colour, _, _) in lines {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 10.0), Sense::hover());
            ui.painter().line_segment(
                [rect.left_center(), rect.right_center()],
                Stroke::new(2.5_f32, colour),
            );
            ui.label(RichText::new(name).small());
            ui.add_space(6.0);
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        diamond(ui, rect.center(), 5.0, pal.change);
        ui.label(RichText::new("Over capacity").small());
    });

    let weeks = range.weeks();
    let first = first_monday(week_start(today), *range);
    let top = y_max(points);
    let (left, bottom, height) = (40.0, 20.0, 200.0);
    let size = Vec2::new(ui.available_width().max(200.0), height + bottom);
    let (outer, response) = ui.allocate_exact_size(size, Sense::hover());
    let plot = Rect::from_min_max(
        outer.left_top() + Vec2::new(left, 6.0),
        outer.right_bottom() - Vec2::new(8.0, bottom),
    );
    let step = plot.width() / weeks as f32;
    let x_of = |monday: NaiveDate| {
        let i = (monday - first).num_days() / 7;
        plot.left() + (i as f32 + 0.5) * step
    };
    let y_of = |v: f64| plot.bottom() - (v / top) as f32 * plot.height();

    // Gridlines every 25%, labelled every 50%.
    let steps = (top * 4.0).round() as u32;
    for i in 0..=steps {
        let v = f64::from(i) / 4.0;
        let y = y_of(v);
        let major = i % 2 == 0;
        ui.painter().line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0_f32, if major { pal.line } else { pal.line.gamma_multiply(0.5) }),
        );
        if major {
            ui.painter().text(
                Pos2::new(plot.left() - 6.0, y),
                egui::Align2::RIGHT_CENTER,
                pct(v),
                egui::FontId::monospace(10.0),
                pal.muted,
            );
        }
    }
    // Week labels, thinned to fit.
    let every = i64::from((weeks / 6).max(1));
    for i in (0..i64::from(weeks)).filter(|i| i % every == 0) {
        let monday = first + Duration::days(7 * i);
        ui.painter().text(
            Pos2::new(plot.left() + (i as f32 + 0.5) * step, plot.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            monday.format("%-d %b").to_string(),
            egui::FontId::monospace(10.0),
            pal.muted,
        );
    }

    let xs: Vec<f32> = points.iter().map(|p| x_of(p.week)).collect();
    let hover = response
        .hover_pos()
        .filter(|p| plot.expand(4.0).contains(*p))
        .and_then(|p| nearest(&xs, p.x, step));
    if let Some(i) = hover {
        ui.painter().line_segment(
            [Pos2::new(xs[i], plot.top()), Pos2::new(xs[i], plot.bottom())],
            Stroke::new(1.0_f32, pal.muted.gamma_multiply(0.6)),
        );
    }
    for (_, colour, width, value) in lines {
        let pts: Vec<Pos2> = points
            .iter()
            .zip(&xs)
            .map(|(p, x)| Pos2::new(*x, y_of(value(&p.metrics))))
            .collect();
        ui.painter().add(Shape::line(pts.clone(), Stroke::new(width, colour)));
        for pt in pts {
            ui.painter().circle_filled(pt, width + 1.0, colour);
        }
    }
    for (p, x) in points.iter().zip(&xs) {
        if p.metrics.over_capacity {
            diamond(ui, Pos2::new(*x, y_of(p.metrics.efficiency)), 6.0, pal.change);
        }
    }
    if let Some(i) = hover {
        let text = tooltip(&points[i]);
        response.on_hover_ui_at_pointer(|ui| {
            ui.label(RichText::new(text).monospace());
        });
    }
}

fn diamond(ui: &egui::Ui, c: Pos2, r: f32, colour: Color32) {
    let pts = vec![
        c + Vec2::new(0.0, -r),
        c + Vec2::new(r, 0.0),
        c + Vec2::new(0.0, r),
        c + Vec2::new(-r, 0.0),
    ];
    ui.painter().add(Shape::convex_polygon(
        pts,
        colour,
        Stroke::new(1.5_f32, ui.visuals().extreme_bg_color),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn full() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    #[test]
    fn the_range_starts_at_the_monday_n_weeks_back() {
        let monday = d(10, 26);
        assert_eq!(first_monday(monday, Range::Weeks4), d(10, 5));
        assert_eq!(first_monday(monday, Range::Weeks12), d(8, 10));
        assert_eq!(first_monday(monday, Range::Weeks26), d(5, 4));
    }

    #[test]
    fn saved_days_make_one_point_per_week_inside_the_range() {
        let days = vec![
            (d(9, 28), full()),  // before the 4-week range
            (d(10, 6), full()),  // week of 5 Oct
            (d(10, 7), full()),
            (d(10, 28), full()), // week of 26 Oct
        ];
        let today = d(10, 29);
        let s = series(&days, today, hm(17, 40), &Settings::default(), Range::Weeks4);
        assert_eq!(s.iter().map(|p| p.week).collect::<Vec<_>>(), vec![d(10, 5), d(10, 26)]);
        assert_eq!(s[0].days_counted, 2);
        let all = series(&days, today, hm(17, 40), &Settings::default(), Range::Weeks12);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn future_days_and_empty_ranges_give_no_points() {
        let days = vec![(d(11, 3), full())];
        assert!(series(&days, d(10, 29), hm(12, 0), &Settings::default(), Range::Weeks4).is_empty());
        assert!(series(&[], d(10, 29), hm(12, 0), &Settings::default(), Range::Weeks4).is_empty());
    }

    #[test]
    fn the_chart_grows_past_100_percent_in_quarters() {
        let mut p = series(&[(d(10, 28), full())], d(10, 29), hm(17, 40), &Settings::default(), Range::Weeks4);
        assert_eq!(y_max(&p), 1.5); // 26 work blocks against a target of 20: 130%
        p[0].metrics.efficiency = 0.8;
        assert_eq!(y_max(&p), 1.0);
        p[0].metrics.efficiency = 1.05;
        assert_eq!(y_max(&p), 1.25);
    }

    #[test]
    fn the_tooltip_names_the_week_and_its_values() {
        let p = series(&[(d(10, 28), full())], d(10, 29), hm(17, 40), &Settings::default(), Range::Weeks4);
        let t = tooltip(&p[0]);
        assert!(t.starts_with("26 Oct – 1 Nov · 1 working day\nOEE "), "{t}");
        assert!(t.contains("Efficiency 130.0%"), "{t}");
        assert!(t.ends_with("Over capacity"), "{t}");
    }

    #[test]
    fn the_nearest_point_must_be_within_half_a_week() {
        let xs = [10.0, 30.0, 70.0];
        assert_eq!(nearest(&xs, 12.0, 20.0), Some(0));
        assert_eq!(nearest(&xs, 26.0, 20.0), Some(1));
        assert_eq!(nearest(&xs, 50.0, 20.0), None);
    }
}
