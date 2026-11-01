//! The Settings tab (D-44) and turning the saved shortcut into a system hotkey.
//! The settings themselves live in `oee-store`.

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

pub use oee_store::settings::{AppSettings, SettingsForm, Shortcut, BAD_KEY};

/// `Shortcut` is defined in `oee-store`, which has no hotkey library, so the
/// conversion is added here.
pub trait ShortcutExt {
    fn to_hotkey(&self) -> Result<HotKey, String>;
}

impl ShortcutExt for Shortcut {
    fn to_hotkey(&self) -> Result<HotKey, String> {
        self.validate()?;
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
        let c = self.key.to_ascii_uppercase();
        let name = if c.is_ascii_alphabetic() {
            format!("Key{c}")
        } else {
            format!("Digit{c}")
        };
        let code: Code = name.parse().map_err(|_| BAD_KEY.to_string())?;
        Ok(HotKey::new(Some(mods), code))
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
            ui.label("Weekly OEE goal");
            ui.horizontal(|ui| {
                ui.checkbox(&mut form.goal_on, "On");
                ui.add_enabled(
                    form.goal_on,
                    egui::TextEdit::singleline(&mut form.goal).desired_width(40.0),
                );
                ui.label("%");
            });
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

}
