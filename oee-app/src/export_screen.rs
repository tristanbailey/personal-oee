//! The export panel on the Week tab (D-52, D-60). The range, rows and file
//! writing are in `oee-store`.

use eframe::egui;

pub use oee_store::export_range::{build_days, resolve, write_files, write_totals, ExportForm, Range};

/// Which button on the export panel was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// The days and blocks files (D-52).
    DaysAndBlocks,
    /// The spreadsheet totals file (D-60).
    Totals,
}

/// Draw the export row. Returns which export was asked for, if any.
pub fn show(ui: &mut egui::Ui, form: &mut ExportForm) -> Option<ExportKind> {
    let mut pressed = None;
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
        ui.horizontal(|ui| {
            if ui.button("Export CSV…").clicked() {
                pressed = Some(ExportKind::DaysAndBlocks);
            }
            if ui
                .button("Spreadsheet totals…")
                .on_hover_text("One row per day, in the columns of the tracking spreadsheet")
                .clicked()
            {
                pressed = Some(ExportKind::Totals);
            }
        });
        if let Some(e) = &form.error {
            ui.colored_label(egui::Color32::from_rgb(0xef, 0x5b, 0x50), e);
        }
    });
    pressed
}

