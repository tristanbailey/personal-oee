//! Saving to `~/Library/Application Support/PersonalOEE/` (D-13).
//!
//! One JSON file per day under `days/`, plus `calendar.json`. Every write goes
//! to a temp file first and is then renamed over the target, so a crash can't
//! leave a half-written file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use oee_core::{DayRecord, WeeklyCalendar};
use serde::{de::DeserializeOwned, Serialize};

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The app's folder in Application Support, or `None` if `HOME` isn't set.
    pub fn default_location() -> Option<Self> {
        let home = std::env::var_os("HOME")?;
        Some(Self::new(
            Path::new(&home).join("Library/Application Support/PersonalOEE"),
        ))
    }

    fn day_path(&self, date: NaiveDate) -> PathBuf {
        self.root.join("days").join(format!("{date}.json"))
    }

    fn calendar_path(&self) -> PathBuf {
        self.root.join("calendar.json")
    }

    pub fn save_day(&self, date: NaiveDate, record: &DayRecord) -> io::Result<()> {
        write_json(&self.day_path(date), record)
    }

    pub fn load_day(&self, date: NaiveDate) -> io::Result<Option<DayRecord>> {
        read_json(&self.day_path(date))
    }

    /// Every saved day, oldest first. Unreadable files are skipped.
    pub fn list_days(&self) -> Vec<NaiveDate> {
        let Ok(dir) = fs::read_dir(self.root.join("days")) else {
            return Vec::new();
        };
        let mut dates: Vec<NaiveDate> = dir
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                name.strip_suffix(".json")?.parse().ok()
            })
            .collect();
        dates.sort();
        dates
    }

    pub fn save_calendar(&self, calendar: &WeeklyCalendar) -> io::Result<()> {
        write_json(&self.calendar_path(), calendar)
    }

    pub fn load_calendar(&self) -> io::Result<Option<WeeklyCalendar>> {
        read_json(&self.calendar_path())
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

/// `Ok(None)` when the file doesn't exist yet.
fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, LostCategory, PlannedKind, PlannedWindow, Weekday};

    fn temp_store(name: &str) -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("oee-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Store::new(&dir), dir)
    }

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn missing_files_load_as_none() {
        let (s, dir) = temp_store("missing");
        assert_eq!(s.load_day(date(29)).unwrap(), None);
        assert_eq!(s.load_calendar().unwrap(), None);
        assert!(s.list_days().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn day_round_trips_through_disk() {
        let (s, dir) = temp_store("day");
        let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        r.add_downtime(hm(10, 0), hm(10, 25));
        r.categorise(hm(10, 0), LostCategory::Idle).unwrap();
        r.down_since = Some(hm(11, 0));
        s.save_day(date(29), &r).unwrap();
        assert_eq!(s.load_day(date(29)).unwrap(), Some(r));
        assert!(dir.join("days/2026-10-29.json").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn overwrite_leaves_no_temp_file() {
        let (s, dir) = temp_store("tmp");
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(29), &r).unwrap();
        s.save_day(date(29), &r).unwrap();
        let names: Vec<_> = fs::read_dir(dir.join("days"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec!["2026-10-29.json"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn calendar_round_trips_and_days_list_in_order() {
        let (s, dir) = temp_store("cal");
        let mut c = WeeklyCalendar::default();
        c.add(
            Weekday::Mon,
            PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"),
        );
        s.save_calendar(&c).unwrap();
        assert_eq!(s.load_calendar().unwrap(), Some(c));
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(29), &r).unwrap();
        s.save_day(date(20), &r).unwrap();
        assert_eq!(s.list_days(), vec![date(20), date(29)]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_file_is_an_error_not_a_panic() {
        let (s, dir) = temp_store("bad");
        fs::create_dir_all(dir.join("days")).unwrap();
        fs::write(dir.join("days/2026-10-29.json"), b"{not json").unwrap();
        assert!(s.load_day(date(29)).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
