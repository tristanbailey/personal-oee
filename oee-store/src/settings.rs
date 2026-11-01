//! User settings (D-44): the day's defaults and the downtime shortcut, saved to
//! `settings.json`.

use oee_core::{Settings, BLOCK_MINUTES};
use serde::{Deserialize, Serialize};

use crate::start::{hhmm, parse_hhmm};

pub const MAX_TARGET: u32 = 100;

/// The system-wide downtime shortcut: some modifiers plus one letter or digit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub key: char,
}

impl Default for Shortcut {
    /// FR-011a: ⌃⌥D.
    fn default() -> Self {
        Self {
            ctrl: true,
            alt: true,
            shift: false,
            cmd: false,
            key: 'D',
        }
    }
}

impl Shortcut {
    /// "Ctrl+Option+D". Words, because the window's font can't draw ⌃ ⌥ ⇧.
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Option"),
            (self.shift, "Shift"),
            (self.cmd, "Cmd"),
        ] {
            if on {
                parts.push(name.to_string());
            }
        }
        parts.push(self.key.to_ascii_uppercase().to_string());
        parts.join("+")
    }

    /// Check the shortcut can be registered: a modifier and one letter or digit.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.ctrl || self.alt || self.shift || self.cmd) {
            return Err(NEEDS_MODIFIER.into());
        }
        if !self.key.is_ascii_alphanumeric() {
            return Err(BAD_KEY.into());
        }
        Ok(())
    }
}

pub const NEEDS_MODIFIER: &str =
    "Add at least one modifier (Ctrl, Option, Shift or Cmd) so typing doesn't trigger the shortcut.";
pub const BAD_KEY: &str = "The shortcut key must be one letter or digit.";

/// FR-054: the weekly OEE goal, which can be turned off. The percent is kept
/// while it is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalSetting {
    pub enabled: bool,
    pub percent: u32,
}

pub const DEFAULT_GOAL: u32 = 85;

impl Default for GoalSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            percent: DEFAULT_GOAL,
        }
    }
}

impl GoalSetting {
    /// The goal to show, or `None` when it is off.
    pub fn active(&self) -> Option<u32> {
        self.enabled.then_some(self.percent)
    }
}

/// Everything in `settings.json`. Missing fields take their defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub core: Settings,
    pub shortcut: Shortcut,
    pub goal: GoalSetting,
}

/// The text fields on the Settings tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsForm {
    pub start: String,
    pub end: String,
    pub target: String,
    pub key: String,
    pub goal_on: bool,
    pub goal: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub error: Option<String>,
    pub saved: bool,
}

impl SettingsForm {
    pub fn from_settings(s: &AppSettings) -> Self {
        Self {
            start: hhmm(s.core.default_start),
            end: hhmm(s.core.default_end),
            target: s.core.base_target.to_string(),
            key: s.shortcut.key.to_string(),
            goal_on: s.goal.enabled,
            goal: s.goal.percent.to_string(),
            ctrl: s.shortcut.ctrl,
            alt: s.shortcut.alt,
            shift: s.shortcut.shift,
            cmd: s.shortcut.cmd,
            error: None,
            saved: false,
        }
    }

    /// Check every field and build the settings. Nothing changes on error.
    pub fn parse(&self) -> Result<AppSettings, String> {
        let time = "Enter times as HH:MM, for example 09:00.";
        let default_start = parse_hhmm(&self.start).ok_or(time)?;
        let default_end = parse_hhmm(&self.end).ok_or(time)?;
        if default_end < default_start + BLOCK_MINUTES {
            return Err(format!(
                "The finish time must be at least {BLOCK_MINUTES} minutes after the start time."
            ));
        }
        let base_target: u32 = self
            .target
            .trim()
            .parse()
            .ok()
            .filter(|t| (1..=MAX_TARGET).contains(t))
            .ok_or_else(|| format!("The daily target must be a whole number from 1 to {MAX_TARGET}."))?;
        // A goal that is off keeps its last good percent, so a stray entry
        // in the box doesn't block saving.
        let goal_percent = self.goal.trim().parse().ok().filter(|p| (1..=100).contains(p));
        let goal = match (self.goal_on, goal_percent) {
            (true, Some(percent)) => GoalSetting { enabled: true, percent },
            (true, None) => return Err("The weekly goal must be a whole number from 1 to 100.".into()),
            (false, p) => GoalSetting {
                enabled: false,
                percent: p.unwrap_or(DEFAULT_GOAL),
            },
        };
        let mut chars = self.key.trim().chars();
        let key = match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase(),
            _ => return Err(BAD_KEY.into()),
        };
        let shortcut = Shortcut {
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            cmd: self.cmd,
            key,
        };
        shortcut.validate()?;
        Ok(AppSettings {
            core: Settings {
                default_start,
                default_end,
                base_target,
            },
            shortcut,
            goal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn form() -> SettingsForm {
        SettingsForm::from_settings(&AppSettings::default())
    }

    #[test]
    fn the_defaults_match_the_prd() {
        let s = AppSettings::default();
        assert_eq!(
            (s.core.default_start, s.core.default_end, s.core.base_target),
            (hm(9, 0), hm(17, 40), 20)
        );
        assert_eq!(s.shortcut.label(), "Ctrl+Option+D");
        assert_eq!(form().parse(), Ok(s));
    }

    #[test]
    fn valid_edits_are_applied() {
        let mut f = form();
        f.start = "08:30".into();
        f.end = "17:00".into();
        f.target = "22".into();
        f.key = "j".into();
        f.shift = true;
        let s = f.parse().unwrap();
        assert_eq!(s.core.default_start, hm(8, 30));
        assert_eq!(s.core.default_end, hm(17, 0));
        assert_eq!(s.core.base_target, 22);
        assert_eq!(s.shortcut.label(), "Ctrl+Option+Shift+J");
    }

    #[test]
    fn bad_times_and_ranges_are_refused_in_plain_words() {
        let mut f = form();
        f.start = "nine".into();
        assert_eq!(f.parse(), Err("Enter times as HH:MM, for example 09:00.".into()));
        let mut f = form();
        f.end = "09:10".into();
        assert!(f.parse().unwrap_err().contains("at least 20 minutes"));
        f.end = "09:00".into();
        assert!(f.parse().is_err());
    }

    #[test]
    fn the_target_must_be_a_sensible_whole_number() {
        for bad in ["0", "101", "-3", "abc", "", "2.5"] {
            let mut f = form();
            f.target = bad.into();
            assert!(f.parse().unwrap_err().contains("daily target"), "{bad}");
        }
        let mut f = form();
        f.target = "100".into();
        assert!(f.parse().is_ok());
    }

    #[test]
    fn the_weekly_goal_defaults_to_85_and_can_be_changed_or_turned_off() {
        let f = form();
        assert_eq!((f.goal_on, f.goal.as_str()), (true, "85"));
        assert_eq!(f.parse().unwrap().goal.active(), Some(85));
        let mut f = form();
        f.goal = "80".into();
        assert_eq!(f.parse().unwrap().goal.active(), Some(80));
        f.goal_on = false;
        let off = f.parse().unwrap().goal;
        assert_eq!((off.active(), off.percent), (None, 80));
        // Turning it back on later restores the kept percent.
        assert_eq!(SettingsForm::from_settings(&f.parse().unwrap()).goal, "80");
    }

    #[test]
    fn the_goal_must_be_a_whole_percent_while_it_is_on() {
        for bad in ["0", "101", "-5", "x", "", "82.5"] {
            let mut f = form();
            f.goal = bad.into();
            assert!(f.parse().unwrap_err().contains("weekly goal"), "{bad}");
            f.goal_on = false;
            assert_eq!(f.parse().unwrap().goal.percent, 85, "{bad}");
        }
    }

    #[test]
    fn older_settings_files_load_with_the_default_goal() {
        let s: AppSettings =
            serde_json::from_str(r#"{"core":{"default_start":540,"default_end":1060,"base_target":20}}"#).unwrap();
        assert_eq!(s.goal, GoalSetting::default());
        let s: AppSettings = serde_json::from_str(r#"{"goal":{"enabled":false}}"#).unwrap();
        assert_eq!((s.goal.enabled, s.goal.percent), (false, 85));
    }

    #[test]
    fn the_shortcut_needs_a_modifier_and_a_single_letter_or_digit() {
        let mut f = form();
        f.ctrl = false;
        f.alt = false;
        assert!(f.parse().unwrap_err().contains("at least one modifier"));
        let mut f = form();
        f.key = "".into();
        assert!(f.parse().is_err());
        f.key = "ab".into();
        assert!(f.parse().is_err());
        f.key = "!".into();
        assert!(f.parse().is_err());
        f.key = "5".into();
        assert!(f.parse().is_ok());
    }

    #[test]
    fn settings_json_fills_in_missing_fields() {
        let s: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, AppSettings::default());
        let s: AppSettings =
            serde_json::from_str(r#"{"core":{"default_start":510,"default_end":1020,"base_target":18}}"#).unwrap();
        assert_eq!(s.core.base_target, 18);
        assert_eq!(s.shortcut, Shortcut::default());
        let back: AppSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
