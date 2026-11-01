//! The weekly OEE goal: status and streak (D-56, FR-054).

use crate::trends::WeekPoint;

/// FR-054: how far under the goal an in-progress week can be and still be
/// "on track", in percentage points.
pub const ON_TRACK_MARGIN_POINTS: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Met,
    OnTrack,
    Behind,
    /// A finished week under the goal.
    NotMet,
}

impl GoalStatus {
    pub fn label(self) -> &'static str {
        match self {
            GoalStatus::Met => "Met",
            GoalStatus::OnTrack => "On track",
            GoalStatus::Behind => "Behind",
            GoalStatus::NotMet => "Not met",
        }
    }
}

/// OEE as tenths of a percent, the precision the app shows.
fn tenths(oee: f64) -> i64 {
    (oee * 1000.0).round() as i64
}

/// FR-054: is `oee` (a fraction) at or above the goal, compared as displayed?
pub fn goal_met(oee: f64, goal_percent: u32) -> bool {
    tenths(oee) >= i64::from(goal_percent) * 10
}

/// FR-054: the week's status, or `None` when it has no counted days.
/// `in_progress` is true for the week that contains today.
pub fn goal_status(
    oee: f64,
    days_counted: usize,
    goal_percent: u32,
    in_progress: bool,
) -> Option<GoalStatus> {
    if days_counted == 0 {
        return None;
    }
    Some(if goal_met(oee, goal_percent) {
        GoalStatus::Met
    } else if !in_progress {
        GoalStatus::NotMet
    } else if tenths(oee) >= i64::from(goal_percent.saturating_sub(ON_TRACK_MARGIN_POINTS)) * 10 {
        GoalStatus::OnTrack
    } else {
        GoalStatus::Behind
    })
}

/// FR-054: weeks in a row that met the goal, counting back from the latest.
/// `points` are the working weeks, oldest first; weeks with no counted days
/// aren't in it, so they neither count nor break the streak. The
/// `current_week`, if not met yet, is unfinished and ignored.
pub fn goal_streak<K: Copy + PartialEq>(
    points: &[WeekPoint<K>],
    goal_percent: u32,
    current_week: K,
) -> u32 {
    let mut streak = 0;
    for (i, p) in points.iter().rev().enumerate() {
        let met = goal_met(p.metrics.oee, goal_percent);
        if met {
            streak += 1;
        } else if i == 0 && p.week == current_week {
            continue;
        } else {
            break;
        }
    }
    streak
}
