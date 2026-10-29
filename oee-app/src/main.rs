//! Personal OEE desktop app: window over `oee-core`'s live tracker.

mod andon;
mod store;

use chrono::{Local, NaiveDate, Timelike};
use eframe::egui;
use oee_core::{
    day_result, BlockKind, DayStatus, LostCategory, Mode, Notice, PlannedKind, Settings, Tracker,
};

fn now_minutes() -> u32 {
    let t = Local::now();
    t.hour() * 60 + t.minute()
}

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

struct App {
    tracker: Option<Tracker>,
    messages: Vec<String>,
    store: Option<store::Store>,
    /// The day being tracked and the JSON last written for it.
    today: NaiveDate,
    last_saved: String,
}

impl App {
    /// Open today's saved record if there is one (D-13, D-14).
    fn new() -> Self {
        Self::open(
            store::Store::default_location(),
            Local::now().date_naive(),
            now_minutes(),
        )
    }

    fn open(store: Option<store::Store>, today: NaiveDate, now: u32) -> Self {
        let mut app = Self {
            tracker: None,
            messages: Vec::new(),
            store,
            today,
            last_saved: String::new(),
        };
        app.load_day(now);
        app
    }

    /// Resume `self.today` from disk, or leave the Not started screen.
    fn load_day(&mut self, now: u32) {
        self.tracker = None;
        self.last_saved.clear();
        match self.store.as_ref().map(|s| s.load_day(self.today)) {
            Some(Ok(Some(record))) => {
                self.last_saved = serde_json::to_string(&record).unwrap_or_default();
                self.tracker = Some(Tracker::resume(record, Settings::default(), now));
            }
            Some(Err(e)) => self
                .messages
                .push(format!("Couldn't read today's saved day: {e}")),
            _ => {}
        }
    }

    /// D-14: a new date starts a new record automatically. The old day is
    /// saved first.
    fn roll_over_if_new_day(&mut self, date: NaiveDate, now: u32) {
        if date == self.today {
            return;
        }
        self.save_if_changed();
        self.today = date;
        self.load_day(now);
    }

    /// Write the day if anything changed since the last write.
    fn save_if_changed(&mut self) {
        let (Some(store), Some(t)) = (&self.store, &self.tracker) else {
            return;
        };
        let Ok(json) = serde_json::to_string(&t.record) else {
            return;
        };
        if json == self.last_saved {
            return;
        }
        match store.save_day(self.today, &t.record) {
            Ok(()) => self.last_saved = json,
            Err(e) => self.messages.push(format!("Couldn't save: {e}")),
        }
    }

    fn note(&mut self, notices: Vec<Notice>) {
        for n in notices {
            self.messages.push(match n {
                Notice::PlannedStarted { label, .. } => format!("Planned downtime started: {label}"),
                Notice::PlannedEnded => "Planned downtime ended, back to Work".into(),
                Notice::CategoriseReminder { blocks } => {
                    format!("{blocks} lost block(s) need a category")
                }
                Notice::DayEnded => "Day ended".into(),
            });
        }
        let extra = self.messages.len().saturating_sub(5);
        self.messages.drain(..extra);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        let now = now_minutes();
        self.roll_over_if_new_day(Local::now().date_naive(), now);

        if let Some(t) = self.tracker.as_mut() {
            let notices = t.tick(now);
            self.note(notices);
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Personal OEE");
                ui.label(hhmm(now));
                ui.separator();
                match self.tracker.as_mut() {
                    None => {
                        if ui.button("Start day").clicked() {
                            self.tracker =
                                Some(Tracker::start_day(now, Vec::new(), Settings::default()));
                        }
                    }
                    Some(t) => {
                        let mode = t.mode();
                        match mode {
                            Mode::Work => {
                                ui.label("Working");
                                if ui.button("Start downtime").clicked() {
                                    t.toggle_downtime(now);
                                }
                            }
                            Mode::Downtime { since } => {
                                ui.colored_label(
                                    egui::Color32::LIGHT_RED,
                                    format!("Downtime for {} min", now.saturating_sub(since)),
                                );
                                if ui.button("Back to work").clicked() {
                                    t.toggle_downtime(now);
                                }
                            }
                            Mode::Planned { kind, until } => {
                                let k = match kind {
                                    PlannedKind::Break => "Break",
                                    PlannedKind::Meeting => "Meeting",
                                };
                                ui.label(format!("{k} until {}", hhmm(until)));
                            }
                            Mode::Ended => {
                                ui.label("Day ended");
                                if ui.button("Finalise").clicked() {
                                    t.finalise();
                                }
                            }
                        }
                        if ui.button("+20 min").clicked() {
                            let end = t.record.end + 20;
                            t.extend_to(end, now);
                        }
                    }
                }
            });
        });

        egui::SidePanel::left("andon")
            .resizable(false)
            .show(ctx, |ui| {
                andon::stack_light(ui, andon::lamps(self.tracker.as_ref().map(|t| t.mode())));
            });

        egui::TopBottomPanel::top("status").show(ctx, |ui| {
            let word = andon::status_word(self.tracker.as_ref());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(word).size(44.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(hhmm(now)).size(32.0).monospace());
                });
            });
            ui.label(andon::sub_line(self.tracker.as_ref(), now));
            ui.add_space(4.0);
        });

        egui::SidePanel::right("metrics").min_width(200.0).show(ctx, |ui| {
            ui.heading("Today");
            if let Some(t) = &self.tracker {
                let inputs = t.inputs_at(now);
                let r = day_result(Some(&inputs));
                let m = r.metrics;
                ui.label(format!("Day {} – {}", hhmm(t.record.start), hhmm(t.record.end)));
                ui.label(format!("Target: {} points", inputs.target_points));
                ui.label(format!("Work: {} points", inputs.work_points));
                ui.separator();
                ui.label(format!("Availability: {}", pct(m.availability)));
                ui.label(format!("Efficiency: {}", pct(m.efficiency)));
                ui.label(format!("Quality: {}", pct(m.quality)));
                ui.strong(format!("OEE: {}", pct(m.oee)));
                if m.over_capacity {
                    ui.colored_label(egui::Color32::YELLOW, "Over capacity");
                }
                if r.status == DayStatus::ZeroWork {
                    ui.label("No Work points yet");
                }
            } else {
                ui.label("Start the day to begin.");
            }
            ui.separator();
            for msg in &self.messages {
                ui.label(msg);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(t) = self.tracker.as_mut() else {
                ui.label("No day in progress.");
                return;
            };
            let until = if t.mode() == Mode::Ended { t.record.end } else { now };
            let blocks = t.live_record(now).blocks_until(until);
            let mut cat_edit = None;
            let mut rework_edit = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for b in &blocks {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{}–{}", hhmm(b.start), hhmm(b.end)));
                        match b.kind {
                            BlockKind::Work { rework } => {
                                ui.label("Work");
                                let mut r = rework;
                                if ui.checkbox(&mut r, "Rework").changed() {
                                    rework_edit = Some((b.start, r));
                                }
                            }
                            BlockKind::Lost { category } => {
                                let mut sel = category;
                                egui::ComboBox::from_id_salt(b.start)
                                    .selected_text(
                                        sel.map_or("Uncategorised", |c| c.label()),
                                    )
                                    .show_ui(ui, |ui| {
                                        for c in LostCategory::ALL {
                                            ui.selectable_value(&mut sel, Some(c), c.label())
                                                .on_hover_text(c.description());
                                        }
                                    });
                                if sel != category {
                                    if let Some(c) = sel {
                                        cat_edit = Some((b.start, c));
                                    }
                                }
                            }
                            BlockKind::Planned(k) => {
                                ui.label(match k {
                                    PlannedKind::Break => "Planned: break",
                                    PlannedKind::Meeting => "Planned: meeting",
                                });
                            }
                        }
                    });
                }
            });
            if let Some((s, c)) = cat_edit {
                let _ = t.record.categorise(s, c);
            }
            if let Some((s, r)) = rework_edit {
                let _ = t.record.set_rework(s, r);
            }
        });

        self.save_if_changed();
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([820.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Personal OEE",
        options,
        Box::new(|_| Ok(Box::new(App::new()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::hm;

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("oee-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn opens_on_not_started_when_nothing_is_saved() {
        let dir = temp_dir("fresh");
        let app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert!(app.tracker.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_new_date_saves_yesterday_and_starts_fresh() {
        let dir = temp_dir("roll");
        let mut app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        app.tracker = Some(Tracker::start_day(hm(9, 0), vec![], Settings::default()));

        app.roll_over_if_new_day(date(30), hm(8, 50));
        assert_eq!(app.today, date(30));
        assert!(app.tracker.is_none());
        let saved = store::Store::new(&dir).load_day(date(29)).unwrap();
        assert!(saved.is_some());

        // Coming back to the old date's file later reopens it.
        app.roll_over_if_new_day(date(29), hm(10, 0));
        assert!(app.tracker.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn same_date_changes_nothing() {
        let dir = temp_dir("same");
        let mut app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        app.tracker = Some(Tracker::start_day(hm(9, 0), vec![], Settings::default()));
        app.roll_over_if_new_day(date(29), hm(9, 5));
        assert!(app.tracker.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
