//! The clock (C-03): the system clock, or a fixed time from `--now` /
//! `OEE_NOW`, so the same command gives the same answer every time.
//!
//! Accepted forms: `HH:MM` (today's date) and `YYYY-MM-DDTHH:MM`, with a space
//! allowed instead of `T`. Seconds, if given, are ignored: the engine works in
//! whole minutes.

use chrono::{DateTime, Local, NaiveDate, Timelike};

use crate::start::parse_hhmm;

pub const ENV_VAR: &str = "OEE_NOW";

/// A date and a time of day in minutes since midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    pub date: NaiveDate,
    pub minutes: u32,
}

impl Now {
    pub fn from_system(t: DateTime<Local>) -> Self {
        Self {
            date: t.date_naive(),
            minutes: t.hour() * 60 + t.minute(),
        }
    }
}

/// The time to use: `flag` beats `env`, which beats the system clock.
/// A time without a date takes the date of `system`. Empty values count as unset.
pub fn resolve(
    flag: Option<&str>,
    env: Option<&str>,
    system: DateTime<Local>,
) -> Result<Now, String> {
    let given = flag
        .filter(|v| !v.trim().is_empty())
        .or(env.filter(|v| !v.trim().is_empty()));
    let today = Now::from_system(system);
    match given {
        None => Ok(today),
        Some(text) => parse(text, today.date),
    }
}

/// [`resolve`] using the real environment and system clock.
pub fn from_environment(flag: Option<&str>) -> Result<Now, String> {
    let env = std::env::var(ENV_VAR).ok();
    resolve(flag, env.as_deref(), Local::now())
}

/// Parse `HH:MM`, or a date and time. `default_date` fills in a missing date.
pub fn parse(text: &str, default_date: NaiveDate) -> Result<Now, String> {
    let bad = || {
        format!(
            "Couldn't read the time \"{}\". Use HH:MM, or YYYY-MM-DDTHH:MM, for example 10:25 or 2026-11-02T10:25.",
            text.trim()
        )
    };
    let text = text.trim();
    let (date, time) = match text.split_once(['T', ' ']) {
        Some((d, t)) => (
            NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").map_err(|_| bad())?,
            t.trim(),
        ),
        None => (default_date, text),
    };
    // Drop seconds: HH:MM:SS -> HH:MM.
    let mut parts = time.splitn(3, ':');
    let (h, m, s) = (parts.next(), parts.next(), parts.next());
    if let Some(s) = s {
        if s.len() != 2 || !s.chars().all(|c| c.is_ascii_digit()) || s.parse::<u32>().map_or(true, |v| v > 59) {
            return Err(bad());
        }
    }
    let hm = format!("{}:{}", h.ok_or_else(bad)?, m.ok_or_else(bad)?);
    let minutes = parse_hhmm(&hm).ok_or_else(bad)?;
    Ok(Now { date, minutes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    fn system() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 1, 15, 12, 40).unwrap()
    }

    #[test]
    fn with_no_flag_or_variable_it_is_the_system_clock_in_whole_minutes() {
        let n = resolve(None, None, system()).unwrap();
        assert_eq!((n.date, n.minutes), (d(1), 15 * 60 + 12));
    }

    #[test]
    fn a_bare_time_takes_the_system_date() {
        let n = resolve(Some("10:25"), None, system()).unwrap();
        assert_eq!((n.date, n.minutes), (d(1), 10 * 60 + 25));
        assert_eq!(resolve(Some("9:05"), None, system()).unwrap().minutes, 9 * 60 + 5);
    }

    #[test]
    fn a_date_and_time_sets_both() {
        for text in ["2026-11-02T10:25", "2026-11-02 10:25", " 2026-11-02T10:25 "] {
            let n = resolve(Some(text), None, system()).unwrap();
            assert_eq!((n.date, n.minutes), (d(2), 10 * 60 + 25), "{text}");
        }
    }

    #[test]
    fn seconds_are_ignored() {
        let n = resolve(Some("10:25:59"), None, system()).unwrap();
        assert_eq!(n.minutes, 10 * 60 + 25);
        let n = resolve(Some("2026-11-02T10:25:30"), None, system()).unwrap();
        assert_eq!((n.date, n.minutes), (d(2), 10 * 60 + 25));
    }

    #[test]
    fn the_flag_beats_the_variable_and_empty_values_are_unset() {
        assert_eq!(resolve(Some("08:00"), Some("09:00"), system()).unwrap().minutes, 8 * 60);
        assert_eq!(resolve(None, Some("09:00"), system()).unwrap().minutes, 9 * 60);
        assert_eq!(resolve(Some(""), Some("09:00"), system()).unwrap().minutes, 9 * 60);
        assert_eq!(resolve(Some(" "), Some(""), system()).unwrap().minutes, 15 * 60 + 12);
    }

    #[test]
    fn the_same_input_always_gives_the_same_time() {
        let later = Local.with_ymd_and_hms(2026, 11, 1, 23, 59, 59).unwrap();
        assert_eq!(
            resolve(Some("2026-11-02T10:25"), None, system()),
            resolve(Some("2026-11-02T10:25"), None, later)
        );
    }

    #[test]
    fn bad_times_are_refused_in_plain_words() {
        for bad in ["25:00", "10:60", "10", "noon", "2026-13-01T10:00", "2026-11-02T", "10:25:7", "10:25:99", "10:25:xx"] {
            let e = resolve(Some(bad), None, system()).unwrap_err();
            assert!(e.contains("Use HH:MM"), "{bad}: {e}");
        }
    }
}
