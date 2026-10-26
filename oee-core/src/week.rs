//! Weekly figures and report helpers.

use crate::metrics::{DayInputs, DayResult, Metrics};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeekReport {
    /// Days that counted (holidays excluded).
    pub days_counted: usize,
    /// Pooled minutes and points; `totals.losses` is the weekly loss breakdown.
    pub totals: DayInputs,
    pub metrics: Metrics,
}

/// Weekly report from each day of the week (`None` = nothing logged).
///
/// - FR-051: minutes and points are summed, then the formulas applied once.
/// - FR-052: holidays are left out.
/// - FR-053: a week of only holidays is all zeros.
pub fn week_report<'a, I>(days: I) -> WeekReport
where
    I: IntoIterator<Item = Option<&'a DayInputs>>,
{
    let mut totals = DayInputs::default();
    let mut days_counted = 0;
    for day in days.into_iter().flatten() {
        totals += *day;
        days_counted += 1;
    }
    let metrics = if days_counted == 0 {
        Metrics::ZERO
    } else {
        Metrics::from_inputs(&totals)
    };
    WeekReport {
        days_counted,
        totals,
        metrics,
    }
}

/// FR-043: the days flagged as over capacity, for review and correction.
pub fn over_capacity_days<T>(days: &[(T, DayResult)]) -> Vec<&T> {
    days.iter()
        .filter(|(_, r)| r.metrics.over_capacity)
        .map(|(key, _)| key)
        .collect()
}
