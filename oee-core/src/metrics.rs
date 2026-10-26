//! Daily OEE maths.

use std::ops::AddAssign;

use crate::category::LossMinutes;
use crate::rules::REWORK_LOSS;

/// Totals for one day (or a pooled week), in minutes and points.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayInputs {
    /// Every minute from start to finish, including early start and extension (FR-006a).
    pub day_minutes: u32,
    /// Breaks and meetings (FR-032).
    pub planned_minutes: u32,
    /// Unplanned downtime per category, Change Over already at 10 min per block.
    pub losses: LossMinutes,
    /// Completed Work blocks (FR-010).
    pub work_points: u32,
    /// Work blocks marked as Rework (FR-019, FR-020).
    pub rework_points: u32,
    /// Daily target, 20 by default (FR-046, FR-047).
    pub target_points: u32,
}

impl DayInputs {
    pub fn downtime_minutes(&self) -> u32 {
        self.losses.total()
    }
}

impl AddAssign for DayInputs {
    fn add_assign(&mut self, o: Self) {
        self.day_minutes += o.day_minutes;
        self.planned_minutes += o.planned_minutes;
        self.losses += o.losses;
        self.work_points += o.work_points;
        self.rework_points += o.rework_points;
        self.target_points += o.target_points;
    }
}

/// The four OEE values, each as a fraction (1.0 = 100%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub availability: f64,
    pub efficiency: f64,
    pub quality: f64,
    pub oee: f64,
    /// FR-048: Efficiency over 100%.
    pub over_capacity: bool,
}

impl Metrics {
    pub const ZERO: Metrics = Metrics {
        availability: 0.0,
        efficiency: 0.0,
        quality: 0.0,
        oee: 0.0,
        over_capacity: false,
    };

    /// Apply the formulas to a set of totals. Zero Work points gives all zeros.
    ///
    /// - FR-044: `Availability = (day − planned − downtime) / (day − planned)`
    /// - FR-048: `Efficiency = work / target`
    /// - FR-049: `Quality = (work − 0.5 × rework) / work`
    /// - FR-050: `OEE = Availability × Efficiency × Quality`
    pub fn from_inputs(i: &DayInputs) -> Metrics {
        if i.work_points == 0 {
            return Metrics::ZERO;
        }
        let planned_time = f64::from(i.day_minutes.saturating_sub(i.planned_minutes));
        let downtime = f64::from(i.downtime_minutes());
        let availability = if planned_time > 0.0 {
            ((planned_time - downtime) / planned_time).max(0.0)
        } else {
            0.0
        };

        let work = f64::from(i.work_points);
        let efficiency = if i.target_points > 0 {
            work / f64::from(i.target_points)
        } else {
            0.0
        };

        let rework = f64::from(i.rework_points.min(i.work_points));
        let quality = (work - REWORK_LOSS * rework) / work;

        Metrics {
            availability,
            efficiency,
            quality,
            oee: availability * efficiency * quality,
            over_capacity: efficiency > 1.0,
        }
    }
}

/// How a day is treated in reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayStatus {
    /// FR-045: nothing logged. Shows 0 and is left out of the week.
    Holiday,
    /// FR-045a: time logged but 0 Work points. Shows 0 and counts in the week.
    ZeroWork,
    /// A normal day.
    Worked,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayResult {
    pub status: DayStatus,
    pub metrics: Metrics,
}

/// Daily result. Pass `None` for a day with nothing logged.
pub fn day_result(inputs: Option<&DayInputs>) -> DayResult {
    match inputs {
        None => DayResult {
            status: DayStatus::Holiday,
            metrics: Metrics::ZERO,
        },
        Some(i) if i.work_points == 0 => DayResult {
            status: DayStatus::ZeroWork,
            metrics: Metrics::ZERO,
        },
        Some(i) => DayResult {
            status: DayStatus::Worked,
            metrics: Metrics::from_inputs(i),
        },
    }
}
