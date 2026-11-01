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

use crate::settings::AppSettings;
use crate::tabs::UiState;
use serde::{de::DeserializeOwned, Serialize};

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The folder in `OEE_DATA_DIR` if set, else the app's folder in
    /// Application Support, or `None` if `HOME` isn't set (C-02).
    pub fn default_location() -> Option<Self> {
        let req = crate::location::from_environment(None, false, false);
        crate::location::resolve(&req).ok().map(Self::new)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Delete every file the app keeps: days, calendar, settings and UI
    /// state. The folder stays. Returns how many files were removed.
    pub fn clear(&self) -> io::Result<usize> {
        let mut removed = 0;
        let remove = |path: PathBuf| match fs::remove_file(&path) {
            Ok(()) => Ok(1),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e),
        };
        if let Ok(dir) = fs::read_dir(self.root.join("days")) {
            for entry in dir.flatten() {
                removed += remove(entry.path())?;
            }
        }
        for name in ["calendar.json", "settings.json", "ui.json"] {
            removed += remove(self.root.join(name))?;
        }
        Ok(removed)
    }

    /// True when no day, calendar, settings or UI file is saved.
    pub fn is_empty(&self) -> bool {
        let days = fs::read_dir(self.root.join("days")).map_or(0, |d| d.flatten().count());
        days == 0 && ["calendar.json", "settings.json", "ui.json"].iter().all(|n| !self.root.join(n).exists())
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

    /// Remove a saved day. Returns false if there was none.
    pub fn delete_day(&self, date: NaiveDate) -> io::Result<bool> {
        match fs::remove_file(self.day_path(date)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn load_day(&self, date: NaiveDate) -> io::Result<Option<DayRecord>> {
        read_json(&self.day_path(date))
    }

    /// The seven days from `monday`, each with its record if one is saved.
    /// An unreadable file counts as no record.
    pub fn load_week(&self, monday: NaiveDate) -> Vec<(NaiveDate, Option<DayRecord>)> {
        (0..7)
            .map(|i| {
                let date = monday + chrono::Duration::days(i);
                (date, self.load_day(date).ok().flatten())
            })
            .collect()
    }

    /// Every saved day with its record, oldest first. Unreadable files are skipped.
    pub fn load_all(&self) -> Vec<(NaiveDate, DayRecord)> {
        self.list_days()
            .into_iter()
            .filter_map(|d| Some((d, self.load_day(d).ok().flatten()?)))
            .collect()
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

    /// Last-open tab etc. A missing or unreadable file gives the defaults.
    pub fn load_ui(&self) -> UiState {
        read_json(&self.root.join("ui.json"))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn save_ui(&self, ui: &UiState) -> io::Result<()> {
        write_json(&self.root.join("ui.json"), ui)
    }

    pub fn load_settings(&self) -> io::Result<Option<AppSettings>> {
        read_json(&self.root.join("settings.json"))
    }

    pub fn save_settings(&self, settings: &AppSettings) -> io::Result<()> {
        write_json(&self.root.join("settings.json"), settings)
    }

    pub fn save_calendar(&self, calendar: &WeeklyCalendar) -> io::Result<()> {
        write_json(&self.calendar_path(), calendar)
    }

    pub fn load_calendar(&self) -> io::Result<Option<WeeklyCalendar>> {
        read_json(&self.calendar_path())
    }
}

/// The file format written now. Every saved file carries it as `"version"`.
/// Files saved before it existed have none and are read as they are; a file
/// with a higher number was written by a newer program and is refused rather
/// than guessed at.
pub const FORMAT_VERSION: u32 = 1;

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut doc = serde_json::to_value(value).map_err(io::Error::other)?;
    if let Some(map) = doc.as_object_mut() {
        map.insert("version".into(), FORMAT_VERSION.into());
    }
    let json = serde_json::to_vec_pretty(&doc).map_err(io::Error::other)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

/// `Ok(None)` when the file doesn't exist yet.
fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => {
            let doc: serde_json::Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            if let Some(v) = doc.get("version").and_then(serde_json::Value::as_u64) {
                if v > u64::from(FORMAT_VERSION) {
                    return Err(io::Error::other(format!(
                        "{} was saved by a newer version of Personal OEE (file format {v}, this one reads up to {FORMAT_VERSION})",
                        path.display()
                    )));
                }
            }
            serde_json::from_value(doc).map(Some).map_err(io::Error::other)
        }
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
    fn saved_files_carry_a_version_and_older_files_still_load() {
        let (s, dir) = temp_store("version");
        let record = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(29), &record).unwrap();
        s.save_settings(&Default::default()).unwrap();
        s.save_calendar(&WeeklyCalendar::default()).unwrap();
        for file in ["days/2026-10-29.json", "settings.json", "calendar.json"] {
            let doc: serde_json::Value = serde_json::from_slice(&fs::read(dir.join(file)).unwrap()).unwrap();
            assert_eq!(doc["version"], FORMAT_VERSION, "{file}");
        }
        // The same files without the field, as older versions saved them.
        for file in ["days/2026-10-29.json", "settings.json", "calendar.json"] {
            let mut doc: serde_json::Value = serde_json::from_slice(&fs::read(dir.join(file)).unwrap()).unwrap();
            doc.as_object_mut().unwrap().remove("version");
            fs::write(dir.join(file), serde_json::to_vec(&doc).unwrap()).unwrap();
        }
        assert_eq!(s.load_day(date(29)).unwrap(), Some(record));
        assert!(s.load_settings().unwrap().is_some());
        assert!(s.load_calendar().unwrap().is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_from_a_newer_version_is_refused_not_guessed() {
        let (s, dir) = temp_store("newer");
        s.save_settings(&Default::default()).unwrap();
        let path = dir.join("settings.json");
        let mut doc: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        doc["version"] = (FORMAT_VERSION + 1).into();
        fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        let e = s.load_settings().unwrap_err();
        assert!(e.to_string().contains("newer version"), "{e}");
        let _ = fs::remove_dir_all(dir);
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
    fn delete_removes_a_day_and_reports_whether_there_was_one() {
        let (s, dir) = temp_store("delete");
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(29), &r).unwrap();
        assert_eq!(s.delete_day(date(29)).unwrap(), true);
        assert_eq!(s.load_day(date(29)).unwrap(), None);
        assert_eq!(s.delete_day(date(29)).unwrap(), false);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_week_returns_seven_days_with_records_where_saved() {
        let (s, dir) = temp_store("week");
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(26), &r).unwrap();
        s.save_day(date(28), &r).unwrap();
        s.save_day(date(19), &r).unwrap(); // the week before
        let week = s.load_week(date(26));
        assert_eq!(week.len(), 7);
        assert_eq!(week[0].0, date(26));
        assert_eq!(week[6].0, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        let saved: Vec<_> = week.iter().filter(|(_, r)| r.is_some()).map(|(d, _)| *d).collect();
        assert_eq!(saved, vec![date(26), date(28)]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_all_returns_every_readable_day_oldest_first() {
        let (s, dir) = temp_store("all");
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        s.save_day(date(29), &r).unwrap();
        s.save_day(date(5), &r).unwrap();
        fs::write(dir.join("days/2026-10-12.json"), b"garbage").unwrap();
        let all: Vec<_> = s.load_all().into_iter().map(|(d, _)| d).collect();
        assert_eq!(all, vec![date(5), date(29)]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn settings_round_trip_and_a_missing_file_is_none() {
        let (s, dir) = temp_store("settings");
        assert_eq!(s.load_settings().unwrap(), None);
        let mut custom = AppSettings::default();
        custom.core.base_target = 18;
        custom.shortcut.key = 'J';
        s.save_settings(&custom).unwrap();
        assert_eq!(s.load_settings().unwrap(), Some(custom));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ui_state_round_trips_and_defaults_to_today() {
        use crate::tabs::Tab;
        let (s, dir) = temp_store("ui");
        assert_eq!(s.load_ui().tab, Tab::Today);
        s.save_ui(&UiState { tab: Tab::Week }).unwrap();
        assert_eq!(s.load_ui().tab, Tab::Week);
        fs::write(dir.join("ui.json"), b"garbage").unwrap();
        assert_eq!(s.load_ui().tab, Tab::Today);
        fs::write(dir.join("ui.json"), b"{}").unwrap();
        assert_eq!(s.load_ui().tab, Tab::Today);
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
