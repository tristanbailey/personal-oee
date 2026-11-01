//! D-56: the weekly OEE goal.

use oee_core::*;

fn week(k: u32, oee: f64) -> WeekPoint<u32> {
    WeekPoint {
        week: k,
        days_counted: 3,
        metrics: Metrics { oee, ..Metrics::ZERO },
    }
}

#[test]
fn a_week_pooled_at_83_meets_an_80_goal() {
    assert_eq!(goal_status(0.83, 3, 80, true), Some(GoalStatus::Met));
    assert_eq!(goal_status(0.83, 3, 80, false), Some(GoalStatus::Met));
}

#[test]
fn the_goal_is_compared_at_the_precision_shown() {
    assert!(goal_met(0.7996, 80)); // shows as 80.0%
    assert!(!goal_met(0.7994, 80)); // shows as 79.9%
    assert!(goal_met(0.85, 85));
}

#[test]
fn an_unmet_week_in_progress_is_on_track_within_five_points_else_behind() {
    assert_eq!(goal_status(0.81, 2, 85, true), Some(GoalStatus::OnTrack));
    assert_eq!(goal_status(0.80, 2, 85, true), Some(GoalStatus::OnTrack));
    assert_eq!(goal_status(0.799, 2, 85, true), Some(GoalStatus::Behind));
    assert_eq!(goal_status(0.0, 2, 85, true), Some(GoalStatus::Behind));
}

#[test]
fn a_finished_week_under_the_goal_is_not_met() {
    assert_eq!(goal_status(0.84, 5, 85, false), Some(GoalStatus::NotMet));
}

#[test]
fn a_week_with_no_counted_days_has_no_status() {
    assert_eq!(goal_status(0.0, 0, 85, true), None);
    assert_eq!(goal_status(0.0, 0, 85, false), None);
}

#[test]
fn the_streak_counts_met_weeks_back_from_the_latest() {
    let w = [week(1, 0.5), week(2, 0.9), week(3, 0.86), week(4, 0.85)];
    assert_eq!(goal_streak(&w, 85, 9), 3);
    assert_eq!(goal_streak(&w, 88, 9), 0);
    assert_eq!(goal_streak(&[] as &[WeekPoint<u32>], 85, 9), 0);
}

#[test]
fn an_unfinished_current_week_under_the_goal_does_not_break_the_streak() {
    let w = [week(1, 0.9), week(2, 0.9), week(3, 0.4)];
    assert_eq!(goal_streak(&w, 85, 3), 2);
    // Once that week is finished and missed, the streak is over.
    assert_eq!(goal_streak(&w, 85, 4), 0);
    // A current week that is already met counts.
    let w = [week(1, 0.9), week(2, 0.9), week(3, 0.9)];
    assert_eq!(goal_streak(&w, 85, 3), 3);
}

#[test]
fn weeks_with_nothing_logged_neither_count_nor_break_the_streak() {
    // Weeks 2 and 3 were all holidays, so only 1 and 4 are in the series.
    let w = [week(1, 0.9), week(4, 0.9)];
    assert_eq!(goal_streak(&w, 85, 5), 2);
}
