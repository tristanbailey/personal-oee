//! The block editor: a modal opened by clicking a block (D-25, D-26).

use eframe::egui::{self, RichText};
use oee_core::{Block, BlockKind, DayRecord, LostCategory, Mode, PlannedKind};

/// FR-025: notes are optional and capped.
pub const NOTE_LIMIT: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// FR-012: pick one of the five categories.
    Lost { category: Option<LostCategory> },
    /// FR-019: Work, optionally Rework.
    Work { rework: bool },
    /// Read-only: names the planned window.
    Planned { name: String },
    /// A stop that hasn't ended yet. It can be categorised once it has.
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    pub block_start: u32,
    pub block_end: u32,
    pub kind: Kind,
    pub note: String,
    pub error: Option<String>,
}

fn planned_name(record: &DayRecord, start: u32, kind: PlannedKind) -> String {
    record
        .planned
        .iter()
        .find(|w| w.start <= start && start < w.end && !w.label.is_empty())
        .map(|w| w.label.clone())
        .unwrap_or_else(|| {
            match kind {
                PlannedKind::Break => "Break",
                PlannedKind::Meeting => "Meeting",
            }
            .into()
        })
}

impl Editor {
    /// Open the editor for the block starting at `block_start`.
    ///
    /// `stored` is the saved record and `live` adds a running stop. A block
    /// that is only lost in `live` can't be edited yet, and neither can the
    /// current block during a stop. Future blocks don't open.
    pub fn open(
        stored: &DayRecord,
        live: &DayRecord,
        block_start: u32,
        now: u32,
        mode: Mode,
    ) -> Option<Editor> {
        let block: Block = live.blocks().into_iter().find(|b| b.start == block_start)?;
        let ended = mode == Mode::Ended;
        if !ended && block.start > now {
            return None;
        }
        let current = !ended && block.start <= now && now < block.end;
        let lost_in_stored = stored
            .blocks()
            .iter()
            .any(|b| b.start == block_start && matches!(b.kind, BlockKind::Lost { .. }));
        let running = (current && matches!(mode, Mode::Downtime { .. }))
            || (matches!(block.kind, BlockKind::Lost { .. }) && !lost_in_stored);
        let kind = if running {
            Kind::Running
        } else {
            match block.kind {
                BlockKind::Lost { category } => Kind::Lost { category },
                BlockKind::Work { rework } => Kind::Work { rework },
                BlockKind::Planned(k) => Kind::Planned {
                    name: planned_name(live, block.start, k),
                },
            }
        };
        Some(Editor {
            block_start,
            block_end: block.end,
            kind,
            note: block.note.unwrap_or_default(),
            error: None,
        })
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            Kind::Lost { .. } => "Lost time",
            Kind::Work { .. } => "Work",
            Kind::Planned { .. } => "Planned downtime",
            Kind::Running => "Stop in progress",
        }
    }

    /// D-26: apply the edits to `record`. Everything is applied to a copy and
    /// only swapped in if every step works, so a failure saves nothing.
    pub fn save(&self, record: &mut DayRecord) -> Result<(), String> {
        if self.note.chars().count() > NOTE_LIMIT {
            return Err(format!("Notes can be at most {NOTE_LIMIT} characters."));
        }
        let mut copy = record.clone();
        let s = self.block_start;
        let result = match &self.kind {
            Kind::Lost {
                category: Some(c), ..
            } => copy.categorise(s, *c),
            Kind::Work { rework } => copy.set_rework(s, *rework),
            _ => Ok(()),
        }
        .and_then(|()| copy.set_note(s, Some(self.note.clone())));
        result.map_err(|e| e.to_string())?;
        *record = copy;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Cancel,
    Save,
}

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Draw the modal. Esc or Cancel closes it without saving.
pub fn show(ctx: &egui::Context, ed: &mut Editor) -> Outcome {
    let mut outcome = Outcome::Open;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Outcome::Cancel;
    }
    egui::Window::new(ed.title())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label(RichText::new(format!("{}–{}", hhmm(ed.block_start), hhmm(ed.block_end))).monospace());
            ui.add_space(6.0);
            match &mut ed.kind {
                Kind::Lost { category } => {
                    for c in LostCategory::ALL {
                        let selected = *category == Some(c);
                        let p = crate::theme::of(ui);
                        // Selected: the status colour as a thick outline on its soft fill.
                        let (fill, edge) = match (selected, c) {
                            (true, LostCategory::ChangeOver) => (p.change_soft, p.change),
                            (true, _) => (p.down_soft, p.down),
                            _ => (p.surface, p.line),
                        };
                        let text = format!("{}\n{}", c.label(), c.description());
                        let button = egui::Button::new(text)
                            .fill(fill)
                            .stroke(egui::Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, edge))
                            .min_size(egui::vec2(ui.available_width(), 44.0));
                        if ui.add(button).clicked() {
                            *category = Some(c);
                        }
                    }
                }
                Kind::Work { rework } => {
                    ui.checkbox(rework, "Mark as Rework (errors or corrections)");
                    ui.label(
                        RichText::new(
                            "Rework still scores 1 Work point. It counts as half a loss in Quality.",
                        )
                        .small(),
                    );
                }
                Kind::Planned { name } => {
                    ui.label(format!("Planned downtime: {name}. This block is set by the calendar."));
                }
                Kind::Running => {
                    ui.label("This stop is still running. You can categorise it once it ends.");
                }
            }
            ui.add_space(6.0);
            ui.label("Note (optional)");
            ui.add(
                egui::TextEdit::multiline(&mut ed.note)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .char_limit(NOTE_LIMIT),
            );
            if let Some(e) = &ed.error {
                crate::theme::error(ui, e);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    outcome = Outcome::Save;
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedWindow, Settings, Tracker};

    fn day() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    fn open(r: &DayRecord, start: u32, now: u32) -> Option<Editor> {
        Editor::open(r, r, start, now, Mode::Work)
    }

    #[test]
    fn titles_and_kinds_follow_the_block() {
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch]);
        r.add_downtime(hm(10, 0), hm(10, 25));
        let now = hm(15, 0);
        let lost = open(&r, hm(10, 0), now).unwrap();
        assert_eq!((lost.title(), lost.block_end), ("Lost time", hm(10, 20)));
        assert_eq!(lost.kind, Kind::Lost { category: None });
        assert_eq!(open(&r, hm(9, 0), now).unwrap().title(), "Work");
        let planned = open(&r, hm(12, 0), now).unwrap();
        assert_eq!(planned.title(), "Planned downtime");
        assert_eq!(planned.kind, Kind::Planned { name: "Lunch".into() });
    }

    #[test]
    fn future_blocks_do_not_open_but_the_current_one_does() {
        let r = day();
        assert!(open(&r, hm(11, 0), hm(10, 0)).is_none());
        assert!(open(&r, hm(10, 0), hm(10, 5)).is_some());
        assert!(open(&r, hm(99, 0), hm(10, 5)).is_none());
    }

    #[test]
    fn saving_a_category_and_note() {
        let mut r = day();
        r.add_downtime(hm(10, 0), hm(10, 25));
        let mut ed = open(&r, hm(10, 0), hm(15, 0)).unwrap();
        ed.kind = Kind::Lost {
            category: Some(LostCategory::Wait),
        };
        ed.note = "waiting on review".into();
        ed.save(&mut r).unwrap();
        let b = r.blocks().into_iter().find(|b| b.start == hm(10, 0)).unwrap();
        assert_eq!(
            b.kind,
            BlockKind::Lost {
                category: Some(LostCategory::Wait)
            }
        );
        assert_eq!(b.note.as_deref(), Some("waiting on review"));
    }

    #[test]
    fn saving_rework_and_a_blank_note_clears_the_note() {
        let mut r = day();
        r.set_note(hm(9, 0), Some("old".into())).unwrap();
        let mut ed = open(&r, hm(9, 0), hm(15, 0)).unwrap();
        assert_eq!(ed.note, "old");
        ed.kind = Kind::Work { rework: true };
        ed.note = "   ".into();
        ed.save(&mut r).unwrap();
        let b = &r.blocks()[0];
        assert_eq!(b.kind, BlockKind::Work { rework: true });
        assert_eq!(b.note, None);
    }

    #[test]
    fn an_edit_error_is_plain_and_saves_nothing() {
        let mut r = day();
        // The editor thinks this block is lost, but it's Work.
        let ed = Editor {
            block_start: hm(9, 0),
            block_end: hm(9, 20),
            kind: Kind::Lost {
                category: Some(LostCategory::Idle),
            },
            note: "should not be saved".into(),
            error: None,
        };
        let before = r.clone();
        assert_eq!(
            ed.save(&mut r),
            Err("only lost-time blocks can be categorised".into())
        );
        assert_eq!(r, before);
    }

    #[test]
    fn notes_over_the_limit_are_refused() {
        let mut r = day();
        let mut ed = open(&r, hm(9, 0), hm(15, 0)).unwrap();
        ed.note = "x".repeat(NOTE_LIMIT + 1);
        assert!(ed.save(&mut r).unwrap_err().contains("300"));
        ed.note = "x".repeat(NOTE_LIMIT);
        assert!(ed.save(&mut r).is_ok());
    }

    #[test]
    fn a_running_stop_cannot_be_edited_yet() {
        let mut t = Tracker::start_day(hm(9, 0), vec![], Settings::default());
        t.toggle_downtime(hm(10, 5));
        let now = hm(10, 30);
        let live = t.live_record(now);
        // The provisional lost block (10:00) and the current block (10:20).
        for start in [hm(10, 0), hm(10, 20)] {
            let ed = Editor::open(&t.record, &live, start, now, t.mode()).unwrap();
            assert_eq!(ed.kind, Kind::Running, "{start}");
        }
        // An earlier Work block still edits normally.
        let ed = Editor::open(&t.record, &live, hm(9, 0), now, t.mode()).unwrap();
        assert_eq!(ed.kind, Kind::Work { rework: false });
    }
}
