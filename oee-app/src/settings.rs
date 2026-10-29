//! User settings (D-44): the day's defaults and the downtime shortcut, saved to
//! `settings.json`.

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
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

    pub fn to_hotkey(&self) -> Result<HotKey, String> {
        let mut mods = Modifiers::empty();
        for (on, m) in [
            (self.ctrl, Modifiers::CONTROL),
            (self.alt, Modifiers::ALT),
            (self.shift, Modifiers::SHIFT),
            (self.cmd, Modifiers::SUPER),
        ] {
            if on {
                mods |= m;
            }
        }
        if mods.is_empty() {
            return Err(NEEDS_MODIFIER.into());
        }
        let c = self.key.to_ascii_uppercase();
        let name = if c.is_ascii_alphabetic() {
            format!("Key{c}")
        } else if c.is_ascii_digit() {
            format!("Digit{c}")
        } else {
            return Err(BAD_KEY.into());
        };
        let code: Code = name.parse().map_err(|_| BAD_KEY.to_string())?;
        Ok(HotKey::new(Some(mods), code))
    }
}

const NEEDS_MODIFIER: &str =
    "Add at least one modifier (Ctrl, Option, Shift or Cmd) so typing doesn't trigger the shortcut.";
const BAD_KEY: &str = "The shortcut key must be one letter or digit.";

/// Everything in `settings.json`. Missing fields take their defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub core: Settings,
    pub shortcut: Shortcut,
}

/// The text fields on the Settings tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsForm {
    pub start: String,
    pub end: String,
    pub target: String,
    pub key: String,
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
        shortcut.to_hotkey()?;
        Ok(AppSettings {
            core: Settings {
                default_start,
                default_end,
                base_target,
            },
            shortcut,
        })
    }
}

/// Draw the Settings tab. Returns true when Save was pressed.
pub fn show(ui: &mut egui::Ui, form: &mut SettingsForm) -> bool {
    let mut save = false;
    ui.heading("Settings");
    ui.label("New days use these. Days already started keep the settings they began with.");
    ui.add_space(8.0);
    egui::Grid::new("settings_grid")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            ui.label("Default start time");
            ui.add(egui::TextEdit::singleline(&mut form.start).desired_width(60.0));
            ui.end_row();
            ui.label("Default finish time");
            ui.add(egui::TextEdit::singleline(&mut form.end).desired_width(60.0));
            ui.end_row();
            ui.label("Base target (Work points)");
            ui.add(egui::TextEdit::singleline(&mut form.target).desired_width(60.0));
            ui.end_row();
            ui.label("Downtime shortcut");
            ui.horizontal(|ui| {
                ui.checkbox(&mut form.ctrl, "Ctrl");
                ui.checkbox(&mut form.alt, "Option");
                ui.checkbox(&mut form.shift, "Shift");
                ui.checkbox(&mut form.cmd, "Cmd");
                ui.add(egui::TextEdit::singleline(&mut form.key).desired_width(30.0).char_limit(1));
            });
            ui.end_row();
        });
    ui.add_space(8.0);
    if let Some(e) = &form.error {
        crate::theme::error(ui, e);
    } else if form.saved {
        ui.label("Saved.");
    }
    if ui.button("Save").clicked() {
        save = true;
    }
    save
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
    fn shortcuts_turn_into_hotkeys() {
        let d = Shortcut::default().to_hotkey().unwrap();
        assert_eq!(d, HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD));
        let digit = Shortcut {
            cmd: true,
            ctrl: false,
            alt: false,
            shift: false,
            key: '7',
        };
        assert_eq!(
            digit.to_hotkey().unwrap(),
            HotKey::new(Some(Modifiers::SUPER), Code::Digit7)
        );
        assert_eq!(digit.label(), "Cmd+7");
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
