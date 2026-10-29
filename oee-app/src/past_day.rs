//! A past day opened in the Today layout, without the andon panel (D-39).

use chrono::NaiveDate;
use eframe::egui;
use oee_core::{day_result, DayRecord, Mode, Settings};

use crate::editor::{self, Editor};
use crate::grid;
use crate::reminder::{self, CategoriseBanner};
use crate::tiles;
use crate::week_screen::Pill;

/// "Tue 20 Oct".
pub fn title(date: NaiveDate) -> String {
    date.format("%a %-d %b").to_string()
}

pub fn missing_text(date: NaiveDate) -> String {
    format!("Nothing logged on {}. It counts as a holiday.", title(date))
}

/// Uncategorised blocks left on a finished day, with Finish day (FR-026, FR-027).
pub fn finish_banner(record: &DayRecord) -> Option<CategoriseBanner> {
    let n = record.uncategorised().len();
    (n > 0 && !record.finalised).then(|| CategoriseBanner {
        text: format!(
            "{} Anything left becomes System Down when you finish.",
            reminder::blocks_need(n)
        ),
        finish_button: true,
    })
}

pub struct PastDay {
    pub date: NaiveDate,
    /// `None` when nothing was logged that day.
    pub record: Option<DayRecord>,
    /// The JSON last written, so edits are saved only when something changed.
    pub last_saved: String,
    pub editor: Option<Editor>,
}

impl PastDay {
    pub fn new(date: NaiveDate, record: Option<DayRecord>) -> Self {
        let last_saved = record
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok())
            .unwrap_or_default();
        Self {
            date,
            record,
            last_saved,
            editor: None,
        }
    }

    /// The JSON to write if the record changed since the last write.
    pub fn pending_save(&self) -> Option<String> {
        let json = serde_json::to_string(self.record.as_ref()?).ok()?;
        (json != self.last_saved).then_some(json)
    }
}

/// Draw the view. Returns true if "← Back to today" was pressed.
pub fn show(ctx: &egui::Context, ui: &mut egui::Ui, day: &mut PastDay, settings: &Settings) -> bool {
    let mut back = false;
    ui.horizontal(|ui| {
        if ui.button("← Back to today").clicked() {
            back = true;
        }
        ui.heading(title(day.date));
        if let Some(record) = &day.record {
            let result = day_result(Some(&record.inputs(settings)));
            Pill::of(&result).show(ui);
        }
    });
    ui.add_space(6.0);

    let Some(record) = day.record.as_mut() else {
        ui.label(missing_text(day.date));
        return back;
    };

    let inputs = record.inputs(settings);
    let result = day_result(Some(&inputs));
    tiles::show(ui, &tiles::tiles(&inputs, &result.metrics, true));
    for b in tiles::banners(&result, true) {
        ui.add_space(4.0);
        tiles::show_banner(ui, &b);
    }
    if let Some(banner) = finish_banner(record) {
        ui.add_space(4.0);
        if reminder::show(ui, &banner) {
            record.finalise();
        }
    }
    ui.add_space(6.0);

    let rows = grid::rows(record, record.end, Mode::Ended);
    let mut clicked = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.strong(grid::header(record));
        grid::show_legend(ui);
        ui.add_space(4.0);
        clicked = grid::show(ui, &rows);
    });
    if let Some(start) = clicked {
        day.editor = Editor::open(record, record, start, record.end, Mode::Ended);
    }

    if let Some(mut ed) = day.editor.take() {
        match editor::show(ctx, &mut ed) {
            editor::Outcome::Open => day.editor = Some(ed),
            editor::Outcome::Cancel => {}
            editor::Outcome::Save => {
                if let Err(e) = ed.save(record) {
                    ed.error = Some(e);
                    day.editor = Some(ed);
                }
            }
        }
    }
    back
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()
    }

    fn day() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    #[test]
    fn title_and_missing_text_match_the_backlog() {
        assert_eq!(title(d()), "Tue 20 Oct");
        assert_eq!(
            missing_text(d()),
            "Nothing logged on Tue 20 Oct. It counts as a holiday."
        );
    }

    #[test]
    fn the_finish_banner_shows_only_while_blocks_need_a_category() {
        let mut r = day();
        assert_eq!(finish_banner(&r), None);
        r.add_downtime(hm(10, 0), hm(10, 25));
        let b = finish_banner(&r).unwrap();
        assert_eq!(
            b.text,
            "1 lost block needs a category. Anything left becomes System Down when you finish."
        );
        assert!(b.finish_button);
        r.finalise();
        assert_eq!(finish_banner(&r), None);
    }

    #[test]
    fn categorising_a_past_block_marks_the_day_for_saving() {
        let mut r = day();
        r.add_downtime(hm(10, 0), hm(10, 25));
        let mut pd = PastDay::new(d(), Some(r));
        assert_eq!(pd.pending_save(), None);

        let record = pd.record.as_mut().unwrap();
        let mut ed = Editor::open(record, record, hm(10, 0), record.end, Mode::Ended).unwrap();
        ed.kind = editor::Kind::Lost {
            category: Some(oee_core::LostCategory::Wait),
        };
        ed.save(record).unwrap();
        let json = pd.pending_save().expect("edit should be pending");
        pd.last_saved = json;
        assert_eq!(pd.pending_save(), None);
    }

    #[test]
    fn a_day_with_no_record_has_nothing_to_save() {
        let pd = PastDay::new(d(), None);
        assert_eq!(pd.pending_save(), None);
    }

    #[test]
    fn every_block_of_a_past_day_can_be_opened() {
        let r = day();
        for b in r.blocks() {
            assert!(Editor::open(&r, &r, b.start, r.end, Mode::Ended).is_some());
        }
    }
}
