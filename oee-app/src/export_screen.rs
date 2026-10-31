//! CSV export from the Week tab (D-52): choose a range, pick a folder, and
//! write the days file and the blocks file (D-51).

use std::io;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use eframe::egui;
use oee_core::export::ExportDay;
use oee_core::DayRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Range {
    /// The week shown on the Week tab.
    #[default]
    ThisWeek,
    Custom,
    All,
}

#[derive(Debug, Default)]
pub struct ExportForm {
    pub range: Range,
    /// YYYY-MM-DD, for `Range::Custom`.
    pub from: String,
    pub to: String,
    pub error: Option<String>,
}

/// The first and last date to export. `saved` is every saved date, oldest first.
pub fn resolve(
    form: &ExportForm,
    week_monday: NaiveDate,
    saved: &[NaiveDate],
) -> Result<(NaiveDate, NaiveDate), String> {
    match form.range {
        Range::ThisWeek => Ok((week_monday, week_monday + Duration::days(6))),
        Range::All => match (saved.first(), saved.last()) {
            (Some(a), Some(b)) => Ok((*a, *b)),
            _ => Err("Nothing has been logged yet.".into()),
        },
        Range::Custom => {
            let parse = |label: &str, s: &str| {
                NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                    .map_err(|_| format!("{label} date should look like 2026-10-26."))
            };
            let (from, to) = (parse("The first", &form.from)?, parse("The last", &form.to)?);
            if to < from {
                return Err("The last date is before the first.".into());
            }
            Ok((from, to))
        }
    }
}

/// One entry per date from `from` to `to`. A date with a record is exported
/// (today only up to `now`, counting a stop still running). A weekday with no
/// record is a holiday row. Weekends without a record and dates after today
/// are left out, as on the Week tab.
pub fn build_days(
    from: NaiveDate,
    to: NaiveDate,
    saved: &[(NaiveDate, DayRecord)],
    today: NaiveDate,
    now: u32,
) -> Vec<ExportDay> {
    let mut out = Vec::new();
    let mut date = from;
    while date <= to {
        let record = saved.iter().find(|(d, _)| *d == date).map(|(_, r)| r);
        match record {
            Some(r) if date == today => {
                let mut live = r.clone();
                if let Some(since) = live.down_since {
                    live.add_downtime(since, now);
                }
                out.push(ExportDay { date: date.to_string(), record: Some(live), until: now });
            }
            Some(r) => out.push(ExportDay {
                date: date.to_string(),
                record: Some(r.clone()),
                until: r.end,
            }),
            None => {
                let weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
                if !weekend && date <= today {
                    out.push(ExportDay { date: date.to_string(), record: None, until: 0 });
                }
            }
        }
        date += Duration::days(1);
    }
    out
}

/// `name.csv`, or `name (2).csv` and so on, so an earlier export isn't overwritten.
fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.csv"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).csv")))
        .find(|p| !p.exists())
        .expect("an unused name")
}

/// Write both files into `dir`. Returns their paths: days first, then blocks.
pub fn write_files(
    dir: &Path,
    from: NaiveDate,
    to: NaiveDate,
    days_csv: &str,
    blocks_csv: &str,
) -> io::Result<(PathBuf, PathBuf)> {
    let days = free_path(dir, &format!("oee-days-{from}-{to}"));
    let blocks = free_path(dir, &format!("oee-blocks-{from}-{to}"));
    std::fs::write(&days, days_csv)?;
    std::fs::write(&blocks, blocks_csv)?;
    Ok((days, blocks))
}

/// Draw the export row. Returns true when "Export CSV…" was pressed.
pub fn show(ui: &mut egui::Ui, form: &mut ExportForm) -> bool {
    let mut pressed = false;
    ui.collapsing("Export CSV", |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut form.range, Range::ThisWeek, "This week");
            ui.radio_value(&mut form.range, Range::Custom, "Custom dates");
            ui.radio_value(&mut form.range, Range::All, "All days");
        });
        if form.range == Range::Custom {
            ui.horizontal(|ui| {
                ui.label("From");
                ui.add(egui::TextEdit::singleline(&mut form.from).hint_text("2026-10-26").desired_width(90.0));
                ui.label("to");
                ui.add(egui::TextEdit::singleline(&mut form.to).hint_text("2026-10-30").desired_width(90.0));
            });
        }
        if ui.button("Export CSV…").clicked() {
            pressed = true;
        }
        if let Some(e) = &form.error {
            ui.colored_label(egui::Color32::from_rgb(0xef, 0x5b, 0x50), e);
        }
    });
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn rec() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    fn form(range: Range, from: &str, to: &str) -> ExportForm {
        ExportForm { range, from: from.into(), to: to.into(), error: None }
    }

    #[test]
    fn this_week_is_monday_to_sunday() {
        let r = resolve(&form(Range::ThisWeek, "", ""), d(26), &[]).unwrap();
        assert_eq!(r, (d(26), d(30) + Duration::days(2)));
    }

    #[test]
    fn all_days_spans_the_first_to_the_last_saved_date() {
        let r = resolve(&form(Range::All, "", ""), d(26), &[d(20), d(22), d(29)]).unwrap();
        assert_eq!(r, (d(20), d(29)));
        assert!(resolve(&form(Range::All, "", ""), d(26), &[]).is_err());
    }

    #[test]
    fn custom_dates_must_parse_and_be_in_order() {
        let ok = resolve(&form(Range::Custom, " 2026-10-20 ", "2026-10-22"), d(26), &[]);
        assert_eq!(ok.unwrap(), (d(20), d(22)));
        assert!(resolve(&form(Range::Custom, "20 Oct", "2026-10-22"), d(26), &[]).is_err());
        assert!(resolve(&form(Range::Custom, "2026-10-22", "2026-10-20"), d(26), &[]).is_err());
    }

    #[test]
    fn weekdays_without_a_record_are_holidays_and_weekends_are_skipped() {
        // Mon 26 Oct to Sun 1 Nov; a record on Mon and on Sat.
        let saved = vec![(d(26), rec()), (d(31), rec())];
        let days = build_days(d(26), d(26) + Duration::days(6), &saved, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), hm(12, 0));
        let dates: Vec<_> = days.iter().map(|x| x.date.as_str()).collect();
        assert_eq!(
            dates,
            ["2026-10-26", "2026-10-27", "2026-10-28", "2026-10-29", "2026-10-30", "2026-10-31"]
        );
        assert!(days[0].record.is_some() && days[1].record.is_none() && days[5].record.is_some());
    }

    #[test]
    fn dates_after_today_are_left_out() {
        let days = build_days(d(26), d(30), &[], d(28), hm(12, 0));
        assert_eq!(days.len(), 3);
    }

    #[test]
    fn today_is_scored_up_to_now_with_a_running_stop() {
        let mut r = rec();
        r.down_since = Some(hm(11, 0));
        let days = build_days(d(28), d(28), &[(d(28), r)], d(28), hm(11, 40));
        assert_eq!(days[0].until, hm(11, 40));
        let live = days[0].record.as_ref().unwrap();
        assert_eq!(live.downtime.len(), 1);
    }

    #[test]
    fn a_finished_day_counts_in_full() {
        let days = build_days(d(26), d(26), &[(d(26), rec())], d(28), hm(12, 0));
        assert_eq!(days[0].until, hm(17, 40));
    }

    #[test]
    fn an_earlier_export_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("oee-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = write_files(&dir, d(26), d(30), "one", "x").unwrap();
        assert!(a.ends_with("oee-days-2026-10-26-2026-10-30.csv"));
        assert!(b.ends_with("oee-blocks-2026-10-26-2026-10-30.csv"));
        let (a2, _) = write_files(&dir, d(26), d(30), "two", "y").unwrap();
        assert!(a2.ends_with("oee-days-2026-10-26-2026-10-30 (2).csv"));
        assert_eq!(std::fs::read_to_string(a).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(a2).unwrap(), "two");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
