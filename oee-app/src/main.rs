//! Personal OEE desktop app: window over `oee-core`'s live tracker.

use chrono::{Local, Timelike};
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

#[derive(Default)]
struct App {
    tracker: Option<Tracker>,
    messages: Vec<String>,
}

impl App {
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
                        if ui.button("New day").clicked() {
                            self.tracker = None;
                        }
                    }
                }
            });
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
        Box::new(|_| Ok(Box::<App>::default())),
    )
}
