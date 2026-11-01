//! The weekly OEE goal on the Week tab (D-56, FR-054): a status line under
//! the tiles. The goal line on the chart is drawn in `trends_screen`.

use chrono::NaiveDate;
use eframe::egui::{self, Color32, RichText};
use oee_core::{goal_status, GoalStatus};

use crate::theme;
use crate::week_screen::week_start;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalLine {
    pub goal_percent: u32,
    /// `None` when the week has no counted days.
    pub status: Option<GoalStatus>,
    pub streak: u32,
}

/// The status of the week starting `monday`, for a goal of `goal_percent`.
/// `streak` is the run of weeks in a row that met it (FR-054).
pub fn line(
    goal_percent: u32,
    oee: f64,
    days_counted: usize,
    monday: NaiveDate,
    today: NaiveDate,
    streak: u32,
) -> GoalLine {
    GoalLine {
        goal_percent,
        status: goal_status(oee, days_counted, goal_percent, week_start(today) == monday),
        streak,
    }
}

/// "3 weeks in a row", or nothing when there is no streak.
pub fn streak_text(streak: u32) -> Option<String> {
    match streak {
        0 => None,
        1 => Some("1 week in a row".into()),
        n => Some(format!("{n} weeks in a row")),
    }
}

fn colours(status: GoalStatus, p: &theme::Palette) -> (Color32, Color32) {
    match status {
        GoalStatus::Met => (p.work_soft, p.work),
        GoalStatus::OnTrack => (p.plan_soft, p.plan),
        GoalStatus::Behind => (p.change_soft, p.change),
        GoalStatus::NotMet => (p.down_soft, p.down),
    }
}

pub fn show(ui: &mut egui::Ui, g: &GoalLine) {
    let p = theme::of(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Goal {}%", g.goal_percent)).strong());
        match g.status {
            Some(s) => {
                let (fill, edge) = colours(s, &p);
                egui::Frame::none()
                    .fill(fill)
                    .stroke(egui::Stroke::new(1.0_f32, edge))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(8.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(s.label()).small().color(p.ink));
                    });
            }
            None => {
                ui.label(RichText::new("No working days yet").small());
            }
        }
        if let Some(t) = streak_text(g.streak) {
            ui.label(RichText::new(t).small());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    #[test]
    fn the_current_week_is_in_progress_and_earlier_ones_are_finished() {
        let today = d(10, 29); // Thursday of the week of 26 Oct
        let now = line(85, 0.80, 3, d(10, 26), today, 0);
        assert_eq!(now.status, Some(GoalStatus::OnTrack));
        let earlier = line(85, 0.80, 5, d(10, 19), today, 0);
        assert_eq!(earlier.status, Some(GoalStatus::NotMet));
    }

    #[test]
    fn a_week_pooled_at_83_meets_an_80_goal() {
        assert_eq!(line(80, 0.83, 3, d(10, 26), d(10, 29), 1).status, Some(GoalStatus::Met));
    }

    #[test]
    fn a_week_with_nothing_counted_has_no_status() {
        assert_eq!(line(85, 0.0, 0, d(11, 2), d(10, 29), 2).status, None);
    }

    #[test]
    fn the_streak_reads_in_weeks() {
        assert_eq!(streak_text(0), None);
        assert_eq!(streak_text(1).as_deref(), Some("1 week in a row"));
        assert_eq!(streak_text(4).as_deref(), Some("4 weeks in a row"));
    }
}
