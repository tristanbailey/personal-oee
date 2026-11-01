//! Weekly results over time, for the trend chart (D-55, FR-041).

use std::collections::BTreeMap;

use crate::metrics::{DayInputs, Metrics};
use crate::week::week_report;

/// One working week's pooled result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeekPoint<K> {
    pub week: K,
    /// Days that counted (holidays excluded).
    pub days_counted: usize,
    pub metrics: Metrics,
}

/// One pooled result per week, oldest first.
///
/// `days` is each day's week key (for example its Monday) with its totals, or
/// `None` for a holiday. Weeks pool as in FR-051; a week with no counted days
/// is left out, not given a 0 (FR-052, FR-053). `metrics.over_capacity` marks
/// a week whose pooled Efficiency is over 100%.
pub fn weekly_series<K, I>(days: I) -> Vec<WeekPoint<K>>
where
    K: Ord + Copy,
    I: IntoIterator<Item = (K, Option<DayInputs>)>,
{
    let mut weeks: BTreeMap<K, Vec<Option<DayInputs>>> = BTreeMap::new();
    for (week, inputs) in days {
        weeks.entry(week).or_default().push(inputs);
    }
    weeks
        .into_iter()
        .filter_map(|(week, days)| {
            let r = week_report(days.iter().map(|d| d.as_ref()));
            (r.days_counted > 0).then_some(WeekPoint {
                week,
                days_counted: r.days_counted,
                metrics: r.metrics,
            })
        })
        .collect()
}
