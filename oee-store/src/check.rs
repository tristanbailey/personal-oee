//! `office-oee check`: read every saved file and test the invariants.

use oee_core::{day_result, BlockKind, Settings, BLOCK_MINUTES};

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub files_checked: usize,
    pub days_checked: usize,
    pub problems: Vec<Problem>,
}

pub fn run(store: &Store, fallback: &Settings) -> Report {
    let mut report = Report::default();
    let problem = |report: &mut Report, file: &str, code: &'static str, message: String| {
        report.problems.push(Problem { file: file.to_string(), code, message });
    };
    let settings = match store.load_settings() {
        Ok(s) => {
            report.files_checked += usize::from(s.is_some());
            s.map(|s| s.core).unwrap_or(*fallback)
        }
        Err(e) => {
            report.files_checked += 1;
            problem(&mut report, "settings.json", "unreadable", e.to_string());
            *fallback
        }
    };
    match store.load_calendar() {
        Ok(c) => report.files_checked += usize::from(c.is_some()),
        Err(e) => {
            report.files_checked += 1;
            problem(&mut report, "calendar.json", "unreadable", e.to_string());
        }
    }
    for date in store.list_days() {
        let file = format!("days/{date}.json");
        report.files_checked += 1;
        let record = match store.load_day(date) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(e) => {
                problem(&mut report, &file, "unreadable", e.to_string());
                continue;
            }
        };
        report.days_checked += 1;
        if record.end <= record.start || (record.end - record.start) % BLOCK_MINUTES != 0 {
            problem(&mut report, &file, "bad_day_length", format!("{}–{} is not a whole number of 20-minute blocks", record.start, record.end));
            continue;
        }
        let blocks = record.blocks();
        let tiles = blocks.len() as u32 == record.slot_count()
            && blocks.iter().enumerate().all(|(i, b)| {
                b.start == record.start + i as u32 * BLOCK_MINUTES && b.end == b.start + BLOCK_MINUTES
            });
        if !tiles {
            problem(&mut report, &file, "blocks_do_not_tile", "the blocks don't cover the day exactly".into());
        }
        if record.finalised
            && blocks.iter().any(|b| matches!(b.kind, BlockKind::Lost { category: None }))
        {
            problem(&mut report, &file, "uncategorised_in_finalised_day", "a finalised day still has an uncategorised lost block".into());
        }
        let inputs = record.inputs(&settings);
        let r = day_result(Some(&inputs));
        let m = r.metrics;
        for (name, v) in [("availability", m.availability), ("efficiency", m.efficiency), ("quality", m.quality), ("oee", m.oee)] {
            if !v.is_finite() || v < 0.0 {
                problem(&mut report, &file, "metric_out_of_range", format!("{name} is {v}"));
            }
        }
        for (name, v) in [("availability", m.availability), ("quality", m.quality)] {
            if v > 1.0 {
                problem(&mut report, &file, "metric_out_of_range", format!("{name} is above 100%"));
            }
        }
        if m.efficiency > 1.0 && !m.over_capacity {
            problem(&mut report, &file, "over_capacity_flag", "Efficiency is above 100% but the flag is off".into());
        }
        if inputs.planned_minutes > inputs.day_minutes {
            problem(&mut report, &file, "planned_over_day", "planned time is longer than the day".into());
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed;
    use chrono::NaiveDate;

    fn sandbox(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("oee-store-check-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Store::new(dir)
    }

    #[test]
    fn seeded_data_is_healthy_and_a_broken_file_is_found() {
        let store = sandbox("health");
        let date = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        for (d, r) in seed::build("week4", date).unwrap().days {
            store.save_day(d, &r).unwrap();
        }
        let r = run(&store, &Settings::default());
        assert_eq!((r.days_checked, r.problems.len()), (5, 0));
        std::fs::write(store.root().join("days/2026-11-02.json"), "{ nope").unwrap();
        let r = run(&store, &Settings::default());
        assert_eq!(r.problems.len(), 1);
        assert_eq!((r.problems[0].file.as_str(), r.problems[0].code), ("days/2026-11-02.json", "unreadable"));
        let _ = std::fs::remove_dir_all(store.root());
    }
}
