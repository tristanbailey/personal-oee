//! Bulk edit in the shift log (D-54): pick several blocks, change them at once.

use std::collections::BTreeSet;

use eframe::egui::{self, RichText};
use oee_core::{
    BlockKind, BlockType, BulkChange, BulkOutcome, DayRecord, LostCategory, PlannedKind,
};

use crate::grid::Clicked;

#[derive(Debug, Default)]
pub struct Selection {
    starts: BTreeSet<u32>,
    /// The result of the last change, shown until the next pick.
    message: Option<String>,
}

pub enum Action {
    Apply(BulkChange),
    Clear,
}

impl Selection {
    pub fn starts(&self) -> &BTreeSet<u32> {
        &self.starts
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    pub fn clear(&mut self) {
        self.starts.clear();
    }

    /// Route a click on a block. Shift or Cmd, or any block already picked,
    /// makes it a pick; otherwise the block editor opens (returns true).
    /// `selectable` is false for blocks that can't be edited yet.
    pub fn route(&mut self, click: Clicked, selectable: bool) -> bool {
        if !click.multi && self.starts.is_empty() {
            return true;
        }
        if selectable && !self.starts.remove(&click.start) {
            self.starts.insert(click.start);
        }
        self.message = None;
        false
    }

    /// Apply a change to the picked blocks in `record`, then clear the pick.
    pub fn apply(&mut self, record: &mut DayRecord, change: BulkChange) -> BulkOutcome {
        let starts: Vec<u32> = self.starts.iter().copied().collect();
        let outcome = record.apply_bulk(&starts, change);
        self.starts.clear();
        self.message = Some(describe(outcome));
        outcome
    }
}

/// "Set 4 of 5 blocks", or "Set 3 blocks" when none were skipped.
pub fn describe(o: BulkOutcome) -> String {
    let total = o.applied + o.skipped;
    let noun = |n: u32| if n == 1 { "block" } else { "blocks" };
    if o.skipped == 0 {
        format!("Set {} {}", o.applied, noun(o.applied))
    } else {
        format!("Set {} of {total} {}", o.applied, noun(total))
    }
}

/// Which kinds of block are picked, so only changes that fit something are offered.
fn kinds(record: &DayRecord, sel: &Selection) -> (bool, bool) {
    let mut lost = false;
    let mut work = false;
    for b in record.blocks().iter().filter(|b| sel.starts.contains(&b.start)) {
        match b.kind {
            BlockKind::Lost { .. } => lost = true,
            BlockKind::Work { .. } => work = true,
            BlockKind::Planned(_) => {}
        }
    }
    (lost, work)
}

/// The bar above the grid. Shows the last result, or the controls when
/// blocks are picked. Esc clears the pick.
pub fn show(ui: &mut egui::Ui, sel: &Selection, record: &DayRecord) -> Option<Action> {
    if sel.is_empty() {
        match &sel.message {
            Some(m) => {
                ui.label(RichText::new(m).small());
            }
            None => {
                ui.label(RichText::new("Shift-click blocks to change several at once.").small());
            }
        }
        return None;
    }
    let mut action = None;
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = Some(Action::Clear);
    }
    let (lost, work) = kinds(record, sel);
    let mut change = |ui: &mut egui::Ui, label: &str, c: BulkChange| {
        if ui.button(label).clicked() {
            action = Some(Action::Apply(c));
        }
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{} selected", sel.starts.len()));
        ui.label("Type");
        change(ui, "Work", BulkChange::Type(BlockType::Work));
        change(ui, "Lost time", BulkChange::Type(BlockType::Lost));
        change(ui, "Break", BulkChange::Type(BlockType::Planned(PlannedKind::Break)));
        change(ui, "Meeting", BulkChange::Type(BlockType::Planned(PlannedKind::Meeting)));
        if work {
            ui.label("Rework");
            change(ui, "Mark", BulkChange::Rework(true));
            change(ui, "Clear", BulkChange::Rework(false));
        }
    });
    if lost {
        ui.horizontal_wrapped(|ui| {
            ui.label("Category");
            for c in LostCategory::ALL {
                change(ui, c.label(), BulkChange::Category(c));
            }
        });
    }
    if ui.small_button("Clear selection").clicked() {
        action = Some(Action::Clear);
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn click(start: u32, multi: bool) -> Clicked {
        Clicked { start, multi }
    }

    #[test]
    fn a_plain_click_opens_the_editor_until_something_is_picked() {
        let mut s = Selection::default();
        assert!(s.route(click(hm(9, 0), false), true));
        assert!(s.is_empty());
        assert!(!s.route(click(hm(9, 0), true), true));
        // With a pick in place, plain clicks keep picking and unpick.
        assert!(!s.route(click(hm(9, 20), false), true));
        assert_eq!(s.starts().len(), 2);
        assert!(!s.route(click(hm(9, 0), false), true));
        assert_eq!(s.starts().iter().copied().collect::<Vec<_>>(), vec![hm(9, 20)]);
    }

    #[test]
    fn blocks_that_cannot_be_edited_are_not_picked() {
        let mut s = Selection::default();
        assert!(!s.route(click(hm(9, 0), true), false));
        assert!(s.is_empty());
    }

    #[test]
    fn describes_the_result() {
        let o = |applied, skipped| BulkOutcome { applied, skipped };
        assert_eq!(describe(o(4, 1)), "Set 4 of 5 blocks");
        assert_eq!(describe(o(3, 0)), "Set 3 blocks");
        assert_eq!(describe(o(1, 0)), "Set 1 block");
        assert_eq!(describe(o(1, 1)), "Set 1 of 2 blocks");
    }

    #[test]
    fn applying_changes_the_record_once_and_clears_the_pick() {
        let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![]);
        for m in [0, 20, 40] {
            r.add_downtime(hm(9, m), hm(9, m + 20));
        }
        let mut s = Selection::default();
        for m in [0, 20, 40] {
            s.route(click(hm(9, m), true), true);
        }
        s.route(click(hm(10, 0), true), true); // a Work block
        s.apply(&mut r, BulkChange::Category(LostCategory::Wait));
        assert!(s.is_empty());
        assert!(r.uncategorised().is_empty());
        assert_eq!(s.message.as_deref(), Some("Set 3 of 4 blocks"));
    }
}
