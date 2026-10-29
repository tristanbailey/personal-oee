//! Living in the menu bar (D-43): closing the window hides it, and the timer
//! keeps running.

use chrono::{DateTime, Duration, Local};

/// Seconds without a frame before we treat it as the Mac having slept. The
/// window repaints every second, so a gap this long isn't just a slow frame.
pub const SLEEP_GAP_SECONDS: i64 = 120;

pub const HIDDEN_ENV: &str = "PERSONAL_OEE_HIDDEN";

/// Closing the window hides it unless the user chose Quit.
pub fn hide_instead_of_closing(close_requested: bool, quitting: bool) -> bool {
    close_requested && !quitting
}

/// Wall-clock time, so the gap includes time asleep (a monotonic clock
/// doesn't count it on macOS).
pub fn woke_from_sleep(previous: DateTime<Local>, now: DateTime<Local>) -> bool {
    now - previous > Duration::seconds(SLEEP_GAP_SECONDS)
}

/// Start with the window hidden, only in the menu bar, when the environment
/// variable is set to anything but empty or "0".
pub fn start_hidden(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 29, h, m, s).unwrap()
    }

    #[test]
    fn closing_hides_unless_quitting() {
        assert!(hide_instead_of_closing(true, false));
        assert!(!hide_instead_of_closing(true, true));
        assert!(!hide_instead_of_closing(false, false));
    }

    #[test]
    fn a_long_gap_between_frames_means_the_mac_slept() {
        assert!(!woke_from_sleep(at(12, 0, 0), at(12, 0, 1)));
        assert!(!woke_from_sleep(at(12, 0, 0), at(12, 2, 0)));
        assert!(woke_from_sleep(at(12, 0, 0), at(12, 2, 1)));
        assert!(woke_from_sleep(at(12, 0, 0), at(14, 30, 0)));
    }

    #[test]
    fn the_hidden_start_option() {
        assert!(!start_hidden(None));
        assert!(!start_hidden(Some("")));
        assert!(!start_hidden(Some("0")));
        assert!(start_hidden(Some("1")));
    }
}
