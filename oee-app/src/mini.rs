//! The mini widget (D-57): a small always-on-top window that stays beside
//! other work. It shows the block countdown, the day as a strip of blocks,
//! today's OEE, and two buttons: Downtime (the D-17 toggle) and Rework. When
//! lost blocks wait for a category at Planned Downtime or the end of the day
//! it offers the five categories (FR-026).
//!
//! It draws from the same `Tracker` as the main window; nothing here keeps
//! its own copy of the day.

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use oee_core::{day_result, BlockKind, LostCategory, Mode, Tracker};

use crate::grid::{self, Fill};
use crate::theme;
use crate::{andon, reminder, tiles};

pub const SIZE: [f32; 2] = [264.0, 232.0];

/// What the user asked for in the widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The same toggle as the main window's big button.
    Toggle,
    /// Mark or unmark the current Work block as Rework.
    Rework,
    /// Give every waiting lost block this category.
    Categorise(LostCategory),
    /// Bring the main window forward.
    OpenMain,
    Close,
}

/// Everything the widget shows, worked out from the tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub word: &'static str,
    pub sub: String,
    /// One cell per block of the day, in order.
    pub strip: Vec<(Fill, bool)>,
    pub oee: String,
    pub toggle: andon::ToggleButton,
    /// Lost blocks so far in the stop that is running, if one is.
    pub lost_now: Option<u32>,
    /// The Rework button works on this block start; `None` disables it.
    pub rework_target: Option<u32>,
    pub rework_on: bool,
    /// Lost blocks waiting for a category, when the widget should ask.
    pub categorise: Option<usize>,
}

pub fn view(tracker: Option<&Tracker>, now: u32) -> View {
    let word = andon::status_word(tracker);
    let sub = andon::sub_line(tracker, now);
    let Some(t) = tracker else {
        return View {
            word,
            sub,
            strip: Vec::new(),
            oee: "–".into(),
            toggle: andon::ToggleButton { label: "Not started", tone: andon::Tone::Neutral, action: None },
            lost_now: None,
            rework_target: None,
            rework_on: false,
            categorise: None,
        };
    };
    let live = t.live_record(now);
    let strip = grid::rows(&live, now, t.mode())
        .into_iter()
        .flat_map(|r| r.cells.into_iter().flatten())
        .map(|c| (c.fill, c.current))
        .collect();
    let result = day_result(Some(&t.inputs_at(now)));
    let ended = t.mode() == Mode::Ended;
    let lost_now = matches!(t.mode(), Mode::Downtime { .. }).then(|| lost_blocks(&live, now) - lost_blocks(&t.record, now));
    let rework_target = rework_target(&live, now, ended, lost_now.is_some());
    let rework_on = rework_target.is_some_and(|s| {
        live.blocks().iter().any(|b| b.start == s && matches!(b.kind, BlockKind::Work { rework: true }))
    });
    let waiting = t.record.uncategorised().len();
    View {
        word,
        sub,
        strip,
        oee: tiles::pct(result.metrics.oee),
        toggle: andon::toggle_button(t.mode()),
        lost_now,
        rework_target,
        rework_on,
        categorise: (waiting > 0 && reminder::categorise_banner(t).is_some()).then_some(waiting),
    }
}

fn lost_blocks(record: &oee_core::DayRecord, now: u32) -> u32 {
    record.blocks_until(now).iter().filter(|b| matches!(b.kind, BlockKind::Lost { .. })).count() as u32
}

/// The block the Rework button acts on: the one the clock is in if it is
/// Work, otherwise the latest Work block that has started. During a stop the
/// block the clock is in is part of the stop, so only finished blocks count.
pub fn rework_target(live: &oee_core::DayRecord, now: u32, ended: bool, in_stop: bool) -> Option<u32> {
    live.blocks()
        .iter()
        .filter(|b| (ended || if in_stop { b.end <= now } else { b.start <= now }) && matches!(b.kind, BlockKind::Work { .. }))
        .map(|b| b.start)
        .last()
}

/// Flip Rework on the target block. Returns whether it is now on.
pub fn toggle_rework(tracker: &mut Tracker, now: u32) -> Option<bool> {
    let ended = tracker.mode() == Mode::Ended;
    let in_stop = matches!(tracker.mode(), Mode::Downtime { .. });
    let start = rework_target(&tracker.live_record(now), now, ended, in_stop)?;
    let on = tracker
        .record
        .blocks()
        .iter()
        .any(|b| b.start == start && matches!(b.kind, BlockKind::Work { rework: true }));
    tracker.record.set_rework(start, !on).ok()?;
    Some(!on)
}

/// Give every waiting lost block the category. Returns how many changed.
pub fn categorise_waiting(tracker: &mut Tracker, category: LostCategory) -> u32 {
    let waiting = tracker.record.uncategorised();
    tracker.record.apply_bulk(&waiting, oee_core::BulkChange::Category(category)).applied
}

/// Draw the widget. Returns what was pressed. The caller shows it in its own
/// borderless, always-on-top viewport.
pub fn show(ctx: &egui::Context, v: &View) -> Option<Action> {
    let mut action = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme::palette(ctx.style().visuals.dark_mode).surface).inner_margin(10.0))
        .show(ctx, |ui| {
            let p = theme::of(ui);
            // Title row: also the handle for moving the borderless window.
            ui.horizontal(|ui| {
                let drag = ui.allocate_response(Vec2::new(ui.available_width() - 56.0, 18.0), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.painter().text(
                    drag.rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    "Personal OEE",
                    egui::FontId::proportional(11.0),
                    p.muted,
                );
                if ui.small_button("Open").on_hover_text("Show the main window").clicked() {
                    action = Some(Action::OpenMain);
                }
                if ui.small_button("✕").on_hover_text("Close the widget").clicked() {
                    action = Some(Action::Close);
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(v.word).size(22.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("OEE {}", v.oee)).size(15.0).strong());
                });
            });
            ui.label(RichText::new(&v.sub).size(12.0).color(p.muted));
            ui.add_space(6.0);
            strip(ui, &v.strip);
            ui.add_space(8.0);
            if let Some(n) = v.categorise {
                ui.label(RichText::new(reminder::blocks_need(n)).size(12.0));
                ui.horizontal_wrapped(|ui| {
                    for c in LostCategory::ALL {
                        if ui.small_button(c.label()).clicked() {
                            action = Some(Action::Categorise(c));
                        }
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                let (fill, ink) = match v.toggle.tone {
                    andon::Tone::Red => (p.down, p.on_fill),
                    andon::Tone::Green => (p.work, p.on_fill),
                    andon::Tone::Neutral => (p.line, p.muted),
                };
                let mut label = v.toggle.label.to_string();
                if let Some(n) = v.lost_now {
                    label = format!("{label} · {n} lost");
                }
                let w = (ui.available_width() - 6.0) * 0.64;
                let toggle = egui::Button::new(RichText::new(label).size(14.0).strong().color(ink))
                    .fill(fill)
                    .min_size(Vec2::new(w, 40.0));
                if ui.add_enabled(v.toggle.action.is_some(), toggle).clicked() {
                    action = Some(Action::Toggle);
                }
                let text = if v.rework_on { "Rework ✓" } else { "Rework" };
                let rework = egui::Button::new(RichText::new(text).size(14.0))
                    .min_size(Vec2::new(ui.available_width(), 40.0));
                if ui.add_enabled(v.rework_target.is_some(), rework).clicked() {
                    action = Some(Action::Rework);
                }
            });
        });
    action
}

/// The day as a row of small blocks; the current one has an outline.
fn strip(ui: &mut egui::Ui, cells: &[(Fill, bool)]) {
    if cells.is_empty() {
        return;
    }
    let p = theme::of(ui);
    let gap = 2.0;
    let width = ui.available_width();
    let cell_w = ((width - gap * (cells.len() as f32 - 1.0)) / cells.len() as f32).max(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
    for (i, (fill, current)) in cells.iter().enumerate() {
        let x = rect.left() + i as f32 * (cell_w + gap);
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cell_w, rect.height()));
        let (soft, line) = grid::colours(*fill, &p);
        let solid = match fill {
            Fill::Future => Color32::TRANSPARENT,
            _ => line,
        };
        let _ = soft;
        ui.painter().rect_filled(r, 2.0, solid);
        let stroke = if *current { Stroke::new(2.0_f32, p.ink) } else { Stroke::new(1.0_f32, line.gamma_multiply(0.5)) };
        ui.painter().rect_stroke(r, 2.0, stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedKind, PlannedWindow, Settings};

    fn day() -> Tracker {
        Tracker::start_day(hm(9, 0), vec![PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch")], Settings::default())
    }

    #[test]
    fn with_no_day_the_widget_is_idle() {
        let v = view(None, hm(9, 0));
        assert_eq!((v.word, v.strip.len(), v.toggle.action, v.rework_target), ("NOT STARTED", 0, None, None));
    }

    #[test]
    fn the_strip_has_one_cell_per_block_and_marks_the_current_one() {
        let mut t = day();
        t.tick(hm(10, 10));
        let v = view(Some(&t), hm(10, 10));
        assert_eq!(v.strip.len(), 26);
        assert_eq!(v.strip.iter().filter(|(_, cur)| *cur).count(), 1);
        assert!(v.strip[3].1, "10:00 is the fourth block");
        assert_eq!(v.sub, "Block 4 of 26 · 10 min left");
    }

    #[test]
    fn the_toggle_works_like_the_main_window_and_counts_lost_blocks() {
        let mut t = day();
        t.tick(hm(10, 0));
        assert_eq!(view(Some(&t), hm(10, 0)).toggle.label, "Start downtime");
        t.toggle_downtime_report(hm(10, 5));
        t.tick(hm(10, 50));
        let v = view(Some(&t), hm(10, 50));
        assert_eq!(v.word, "DOWNTIME");
        assert_eq!(v.toggle.label, "Back to work");
        assert_eq!(v.lost_now, Some(2));
    }

    #[test]
    fn rework_flips_the_current_work_block_and_back() {
        let mut t = day();
        t.tick(hm(10, 10));
        assert_eq!(view(Some(&t), hm(10, 10)).rework_target, Some(hm(10, 0)));
        assert_eq!(toggle_rework(&mut t, hm(10, 10)), Some(true));
        assert!(view(Some(&t), hm(10, 10)).rework_on);
        // It counts once the block has finished.
        assert_eq!(t.inputs_at(hm(10, 20)).rework_points, 1);
        assert_eq!(toggle_rework(&mut t, hm(10, 10)), Some(false));
        assert_eq!(t.inputs_at(hm(10, 20)).rework_points, 0);
    }

    #[test]
    fn during_downtime_rework_falls_back_to_the_last_work_block() {
        let mut t = day();
        t.tick(hm(9, 50));
        t.toggle_downtime_report(hm(9, 50));
        t.tick(hm(10, 30));
        // The stop covers two blocks; the latest Work block that started is not one of them.
        let v = view(Some(&t), hm(10, 30));
        assert_eq!(v.lost_now, Some(2));
        assert_eq!(v.rework_target, Some(hm(9, 20)));
    }

    #[test]
    fn it_asks_for_categories_only_at_planned_downtime_or_the_day_end() {
        let mut t = day();
        t.tick(hm(10, 0));
        t.toggle_downtime_report(hm(10, 5));
        t.toggle_downtime_report(hm(10, 30));
        t.tick(hm(11, 0));
        assert_eq!(view(Some(&t), hm(11, 0)).categorise, None, "mid-day: no prompt");
        t.tick(hm(12, 5));
        let v = view(Some(&t), hm(12, 5));
        assert_eq!(v.categorise, Some(1), "at lunch the stop needs a category");
        assert_eq!(categorise_waiting(&mut t, LostCategory::Wait), 1);
        assert_eq!(view(Some(&t), hm(12, 5)).categorise, None);
        assert_eq!(t.record.uncategorised().len(), 0);
    }
}
