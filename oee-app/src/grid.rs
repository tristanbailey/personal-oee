//! The shift log: an hour grid of 20-minute blocks (D-22, D-24).

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use oee_core::{BlockKind, DayRecord, LostCategory, Mode, PlannedKind};

/// How a cell is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Work,
    Lost,
    ChangeOver,
    /// Lost time still waiting for a category: striped, dashed outline.
    Uncategorised,
    Planned,
    /// Hasn't happened yet: dashed outline only.
    Future,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub start: u32,
    pub end: u32,
    pub fill: Fill,
    pub label: String,
    pub rework: bool,
    pub has_note: bool,
    /// The block the clock is in right now.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Hour of day, e.g. 9 for the 09:00 row.
    pub hour: u32,
    /// Slots at :00, :20 and :40.
    pub cells: [Option<Cell>; 3],
}

fn planned_label(record: &DayRecord, start: u32, kind: PlannedKind) -> String {
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

/// Lay the day's blocks out by clock hour, three to a row. Pass the live
/// record so a running stop is included.
pub fn rows(record: &DayRecord, now: u32, mode: Mode) -> Vec<Row> {
    let ended = mode == Mode::Ended;
    let in_downtime = matches!(mode, Mode::Downtime { .. });
    let mut rows: Vec<Row> = Vec::new();

    for b in record.blocks() {
        let finished = ended || b.end <= now;
        let current = !ended && b.start <= now && now < b.end;
        let (fill, label) = match b.kind {
            BlockKind::Planned(k) => (Fill::Planned, planned_label(record, b.start, k)),
            _ if current && in_downtime => (Fill::Lost, "Downtime…".into()),
            BlockKind::Work { .. } if current => (Fill::Work, "In progress".into()),
            BlockKind::Work { .. } if !finished => (Fill::Future, String::new()),
            BlockKind::Work { rework } => (
                Fill::Work,
                if rework { "Rework" } else { "Work" }.into(),
            ),
            BlockKind::Lost { category: None } => (Fill::Uncategorised, "Categorise".into()),
            BlockKind::Lost {
                category: Some(LostCategory::ChangeOver),
            } => (Fill::ChangeOver, LostCategory::ChangeOver.label().into()),
            BlockKind::Lost { category: Some(c) } => (Fill::Lost, c.label().into()),
        };
        let cell = Cell {
            start: b.start,
            end: b.end,
            fill,
            label,
            rework: matches!(b.kind, BlockKind::Work { rework: true }),
            has_note: b.note.is_some(),
            current,
        };
        let hour = b.start / 60;
        if rows.last().map(|r| r.hour) != Some(hour) {
            rows.push(Row {
                hour,
                cells: [None, None, None],
            });
        }
        let slot = (b.start % 60 / 20) as usize;
        rows.last_mut().expect("row just pushed").cells[slot] = Some(cell);
    }
    rows
}

fn colours(fill: Fill) -> (Color32, Color32) {
    // (fill, accent) from the prototype's dark-mode tokens.
    match fill {
        Fill::Work => (Color32::from_rgb(0x18, 0x3a, 0x26), Color32::from_rgb(0x46, 0xb8, 0x74)),
        Fill::Lost | Fill::Uncategorised => {
            (Color32::from_rgb(0x4a, 0x1f, 0x1c), Color32::from_rgb(0xef, 0x5b, 0x50))
        }
        Fill::ChangeOver => (Color32::from_rgb(0x3e, 0x2e, 0x10), Color32::from_rgb(0xf0, 0xa6, 0x2a)),
        Fill::Planned => (Color32::from_rgb(0x1e, 0x2b, 0x3e), Color32::from_rgb(0x84, 0xa6, 0xdc)),
        Fill::Future => (Color32::TRANSPARENT, Color32::from_gray(110)),
    }
}

fn dashed_outline(ui: &egui::Ui, rect: Rect, stroke: Stroke) {
    let pts = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    ui.painter()
        .extend(Shape::dashed_line(&pts, stroke, 4.0, 3.0));
}

/// Draw one cell. Returns true if it was clicked (future cells can't be).
fn paint_cell(ui: &mut egui::Ui, size: Vec2, cell: &Cell) -> bool {
    let sense = if cell.fill == Fill::Future {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if sense == Sense::click() {
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    let (fill, accent) = colours(cell.fill);
    let p = ui.painter();
    p.rect_filled(rect, 6.0, fill);

    if cell.fill == Fill::Uncategorised {
        let clipped = p.with_clip_rect(rect);
        let stripe = Stroke::new(1.5_f32, accent.gamma_multiply(0.35));
        let mut x = rect.left() - rect.height();
        while x < rect.right() {
            clipped.line_segment(
                [Pos2::new(x, rect.bottom()), Pos2::new(x + rect.height(), rect.top())],
                stripe,
            );
            x += 9.0;
        }
    }

    let stroke = Stroke::new(1.0_f32, accent.gamma_multiply(0.6));
    match cell.fill {
        Fill::Future | Fill::Uncategorised => dashed_outline(ui, rect, Stroke::new(1.0_f32, accent)),
        _ => {
            ui.painter().rect_stroke(rect, 6.0, stroke);
        }
    }
    if cell.current {
        ui.painter()
            .rect_stroke(rect, 6.0, Stroke::new(2.5_f32, ui.visuals().strong_text_color()));
    }

    // The fills are dark, so filled cells always use light text; "not yet"
    // cells are transparent and follow the theme.
    let ink = if cell.fill == Fill::Future {
        ui.visuals().text_color()
    } else {
        Color32::from_rgb(0xe6, 0xeb, 0xe9)
    };
    ui.painter().text(
        rect.left_top() + Vec2::new(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        format!("{:02}:{:02}", cell.start / 60, cell.start % 60),
        egui::FontId::monospace(11.0),
        ink.gamma_multiply(0.7),
    );
    ui.painter().text(
        rect.left_bottom() + Vec2::new(8.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        &cell.label,
        egui::FontId::proportional(14.0),
        ink,
    );
    // Dots: purple = Rework, grey = note.
    let mut dot_x = rect.right() - 10.0;
    if cell.rework {
        ui.painter()
            .circle_filled(Pos2::new(dot_x, rect.top() + 10.0), 4.0, REWORK_DOT);
        dot_x -= 12.0;
    }
    if cell.has_note {
        ui.painter()
            .circle_filled(Pos2::new(dot_x, rect.top() + 10.0), 4.0, Color32::from_gray(150));
    }
    response.clicked()
}

/// D-23: "09:00–17:40 · 26 blocks", plus "+ 10 min" when the day isn't a whole
/// number of blocks.
pub fn header(record: &DayRecord) -> String {
    let hhmm = |m: u32| format!("{:02}:{:02}", m / 60, m % 60);
    let n = record.slot_count();
    let mut text = format!(
        "{}–{} · {n} block{}",
        hhmm(record.start),
        hhmm(record.end),
        if n == 1 { "" } else { "s" }
    );
    let extra = (record.end - record.start) % oee_core::BLOCK_MINUTES;
    if extra > 0 {
        text.push_str(&format!(" + {extra} min"));
    }
    text
}

/// What each colour and dot means, in display order.
pub const LEGEND: [(Fill, &str); 6] = [
    (Fill::Work, "Work"),
    (Fill::Lost, "Lost"),
    (Fill::ChangeOver, "Change Over"),
    (Fill::Planned, "Planned"),
    (Fill::Uncategorised, "Needs category"),
    (Fill::Future, "Not yet"),
];

const REWORK_DOT: Color32 = Color32::from_rgb(0xb0, 0x8a, 0xe6);

pub fn show_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (fill, name) in LEGEND {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            let (bg, accent) = colours(fill);
            ui.painter().rect_filled(rect, 3.0, bg);
            match fill {
                Fill::Future | Fill::Uncategorised => dashed_outline(ui, rect, Stroke::new(1.0_f32, accent)),
                _ => {
                    ui.painter()
                        .rect_stroke(rect, 3.0, Stroke::new(1.0_f32, accent.gamma_multiply(0.6)));
                }
            }
            ui.label(egui::RichText::new(name).small());
            ui.add_space(6.0);
        }
        for (colour, name) in [(REWORK_DOT, "Rework"), (Color32::from_gray(150), "Note")] {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, colour);
            ui.label(egui::RichText::new(name).small());
            ui.add_space(6.0);
        }
    });
}

/// Draw the grid. Returns the start of the block that was clicked, if any.
pub fn show(ui: &mut egui::Ui, rows: &[Row]) -> Option<u32> {
    let mut clicked = None;
    let label_w = 48.0;
    let gap = ui.spacing().item_spacing.x;
    let cell_w = ((ui.available_width() - label_w - gap * 3.0) / 3.0).max(60.0);
    let size = Vec2::new(cell_w, 46.0);
    for row in rows {
        ui.horizontal(|ui| {
            ui.add_sized(
                [label_w, size.y],
                egui::Label::new(egui::RichText::new(format!("{:02}:00", row.hour)).monospace()),
            );
            for slot in &row.cells {
                match slot {
                    Some(cell) => {
                        if paint_cell(ui, size, cell) {
                            clicked = Some(cell.start);
                        }
                    }
                    None => {
                        ui.allocate_exact_size(size, Sense::hover());
                    }
                }
            }
        });
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedWindow};

    fn day() -> DayRecord {
        DayRecord::new(hm(9, 0), hm(17, 40), vec![])
    }

    fn cell(rows: &[Row], h: u32, m: u32) -> &Cell {
        rows.iter()
            .find(|r| r.hour == h)
            .and_then(|r| r.cells[(m / 20) as usize].as_ref())
            .unwrap_or_else(|| panic!("no cell at {h}:{m:02}"))
    }

    #[test]
    fn header_shows_range_and_block_count() {
        assert_eq!(header(&day()), "09:00–17:40 · 26 blocks");
        let odd = DayRecord::new(hm(9, 0), hm(17, 30), vec![]);
        assert_eq!(header(&odd), "09:00–17:30 · 25 blocks + 10 min");
        let one = DayRecord::new(hm(9, 0), hm(9, 20), vec![]);
        assert_eq!(header(&one), "09:00–09:20 · 1 block");
    }

    #[test]
    fn legend_covers_every_fill() {
        for fill in [
            Fill::Work,
            Fill::Lost,
            Fill::ChangeOver,
            Fill::Uncategorised,
            Fill::Planned,
            Fill::Future,
        ] {
            assert!(LEGEND.iter().any(|(f, _)| *f == fill), "{fill:?}");
        }
    }

    #[test]
    fn a_520_minute_day_is_nine_rows_ending_at_17_20() {
        let rows = rows(&day(), hm(9, 0), Mode::Work);
        assert_eq!(rows.len(), 9);
        assert_eq!(rows[0].hour, 9);
        assert_eq!(rows[8].hour, 17);
        let last = rows[8].cells.iter().flatten().last().unwrap();
        assert_eq!(last.start, hm(17, 20));
        let total: usize = rows.iter().map(|r| r.cells.iter().flatten().count()).sum();
        assert_eq!(total, 26);
    }

    #[test]
    fn an_early_start_leaves_the_first_slots_of_the_row_empty() {
        let r = DayRecord::new(hm(8, 40), hm(17, 40), vec![]);
        let rows = rows(&r, hm(9, 0), Mode::Work);
        assert_eq!(rows[0].hour, 8);
        assert!(rows[0].cells[0].is_none() && rows[0].cells[1].is_none());
        assert_eq!(rows[0].cells[2].as_ref().unwrap().start, hm(8, 40));
    }

    #[test]
    fn past_current_and_future_work_blocks() {
        let rows = rows(&day(), hm(10, 5), Mode::Work);
        let past = cell(&rows, 9, 20);
        assert_eq!((past.fill, past.label.as_str()), (Fill::Work, "Work"));
        let now = cell(&rows, 10, 0);
        assert!(now.current);
        assert_eq!(now.label, "In progress");
        let future = cell(&rows, 10, 20);
        assert_eq!((future.fill, future.label.as_str()), (Fill::Future, ""));
        assert!(!future.current);
    }

    #[test]
    fn lost_blocks_are_coloured_by_category() {
        let mut r = day();
        r.add_downtime(hm(9, 0), hm(9, 25));
        r.add_downtime(hm(9, 40), hm(10, 5));
        r.categorise(hm(9, 40), LostCategory::ChangeOver).unwrap();
        let rows = rows(&r, hm(11, 0), Mode::Work);
        let uncat = cell(&rows, 9, 0);
        assert_eq!(
            (uncat.fill, uncat.label.as_str()),
            (Fill::Uncategorised, "Categorise")
        );
        let co = cell(&rows, 9, 40);
        assert_eq!((co.fill, co.label.as_str()), (Fill::ChangeOver, "Change Over"));
        r.categorise(hm(9, 0), LostCategory::Wait).unwrap();
        let rows = super::rows(&r, hm(11, 0), Mode::Work);
        assert_eq!(cell(&rows, 9, 0).fill, Fill::Lost);
        assert_eq!(cell(&rows, 9, 0).label, "Wait");
    }

    #[test]
    fn planned_blocks_use_the_window_name() {
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch]);
        let rows = rows(&r, hm(9, 0), Mode::Work);
        for m in [0, 20, 40] {
            let c = cell(&rows, 12, m);
            assert_eq!((c.fill, c.label.as_str()), (Fill::Planned, "Lunch"));
        }
    }

    #[test]
    fn the_current_block_says_downtime_during_a_stop() {
        let r = day();
        let rows = rows(&r, hm(10, 10), Mode::Downtime { since: hm(10, 5) });
        let c = cell(&rows, 10, 0);
        assert!(c.current);
        assert_eq!((c.fill, c.label.as_str()), (Fill::Lost, "Downtime…"));
    }

    #[test]
    fn rework_and_notes_are_flagged_and_ended_days_have_no_current_block() {
        let mut r = day();
        r.set_rework(hm(9, 0), true).unwrap();
        r.set_note(hm(9, 20), Some("pairing".into())).unwrap();
        let rows = rows(&r, hm(18, 0), Mode::Ended);
        let a = cell(&rows, 9, 0);
        assert!(a.rework);
        assert_eq!(a.label, "Rework");
        assert!(cell(&rows, 9, 20).has_note);
        assert!(rows.iter().flat_map(|r| r.cells.iter().flatten()).all(|c| !c.current));
        // Every block is finished on an ended day: none is dashed.
        assert!(rows
            .iter()
            .flat_map(|r| r.cells.iter().flatten())
            .all(|c| c.fill != Fill::Future));
    }
}
