//! Personal OEE desktop app: window over `oee-core`'s live tracker.

mod andon;
mod calendar_screen;
mod editor;
mod grid;
mod reminder;
mod start;
mod store;
mod tabs;
mod week_screen;
mod tiles;

use chrono::{Local, NaiveDate, Timelike};
use eframe::egui;
use oee_core::{
    day_result, Mode, Notice, Settings, Tracker,
};

fn now_minutes() -> u32 {
    let t = Local::now();
    t.hour() * 60 + t.minute()
}

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

struct App {
    tracker: Option<Tracker>,
    messages: Vec<String>,
    store: Option<store::Store>,
    /// The day being tracked and the JSON last written for it.
    today: NaiveDate,
    last_saved: String,
    /// Text in the start-time field on the Not started screen.
    start_input: Option<String>,
    start_error: Option<String>,
    /// The block editor, when open (D-25).
    editor: Option<editor::Editor>,
    /// The screen showing, remembered between launches (D-29).
    tab: tabs::Tab,
    last_tab: tabs::Tab,
    /// The weekly Planned Downtime calendar (D-30), saved to `calendar.json`.
    calendar: oee_core::WeeklyCalendar,
    last_calendar: String,
    add_form: calendar_screen::AddForm,
    /// The Monday of the week shown on the Week tab (D-34).
    week_monday: NaiveDate,
    week_cache: Option<WeekCache>,
    /// Every saved day, for the over-capacity list. Refreshed every few seconds.
    all_days_cache: Option<(std::time::Instant, Vec<(NaiveDate, oee_core::DayRecord)>)>,
}

/// A week's records, reloaded when the week changes or every couple of seconds.
struct WeekCache {
    monday: NaiveDate,
    loaded_at: std::time::Instant,
    days: Vec<(NaiveDate, Option<oee_core::DayRecord>)>,
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
        let tab = store.as_ref().map(|s| s.load_ui().tab).unwrap_or_default();
        let mut app = Self {
            tracker: None,
            messages: Vec::new(),
            store,
            today,
            last_saved: String::new(),
            start_input: None,
            start_error: None,
            editor: None,
            tab,
            last_tab: tab,
            calendar: Default::default(),
            last_calendar: String::new(),
            add_form: Default::default(),
            week_monday: week_screen::week_start(today),
            week_cache: None,
            all_days_cache: None,
        };
        app.load_calendar();
        app.load_day(now);
        app
    }

    fn load_calendar(&mut self) {
        match self.store.as_ref().map(|s| s.load_calendar()) {
            Some(Ok(Some(calendar))) => self.calendar = calendar,
            Some(Err(e)) => self
                .messages
                .push(format!("Couldn't read the calendar: {e}")),
            _ => {}
        }
        self.last_calendar = serde_json::to_string(&self.calendar).unwrap_or_default();
    }

    /// The saved days for the week being shown, refreshed every 2 seconds.
    fn week_days(&mut self) -> &[(NaiveDate, Option<oee_core::DayRecord>)] {
        let stale = self.week_cache.as_ref().is_none_or(|c| {
            c.monday != self.week_monday || c.loaded_at.elapsed().as_secs() >= 2
        });
        if stale {
            let days = self
                .store
                .as_ref()
                .map(|s| s.load_week(self.week_monday))
                .unwrap_or_default();
            self.week_cache = Some(WeekCache {
                monday: self.week_monday,
                loaded_at: std::time::Instant::now(),
                days,
            });
        }
        &self.week_cache.as_ref().expect("just filled").days
    }

    fn all_days(&mut self) -> &[(NaiveDate, oee_core::DayRecord)] {
        let stale = self
            .all_days_cache
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed().as_secs() >= 5);
        if stale {
            let days = self.store.as_ref().map(|s| s.load_all()).unwrap_or_default();
            self.all_days_cache = Some((std::time::Instant::now(), days));
        }
        &self.all_days_cache.as_ref().expect("just filled").1
    }

    /// Write the calendar if it changed since the last write.
    fn save_calendar_if_changed(&mut self) {
        let Some(store) = &self.store else { return };
        let Ok(json) = serde_json::to_string(&self.calendar) else {
            return;
        };
        if json == self.last_calendar {
            return;
        }
        match store.save_calendar(&self.calendar) {
            Ok(()) => self.last_calendar = json,
            Err(e) => self.messages.push(format!("Couldn't save the calendar: {e}")),
        }
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

    fn push_message(&mut self, message: String) {
        self.messages.push(message);
        let extra = self.messages.len().saturating_sub(5);
        self.messages.drain(..extra);
    }

    fn note(&mut self, notices: Vec<Notice>) {
        for n in notices {
            self.push_message(match n {
                Notice::PlannedStarted { label, .. } => format!("Planned downtime started: {label}"),
                Notice::PlannedEnded => "Planned downtime ended, back to Work".into(),
                Notice::CategoriseReminder { blocks } => {
                    format!("{blocks} lost block(s) need a category")
                }
                Notice::DayEnded => "Day ended".into(),
            });
        }
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
                for tab in tabs::Tab::ALL {
                    if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                        self.tab = tab;
                    }
                }
            });
        });
        if self.tab != self.last_tab {
            self.last_tab = self.tab;
            if let Some(store) = &self.store {
                let _ = store.save_ui(&tabs::UiState { tab: self.tab });
            }
        }

        if self.tab != tabs::Tab::Today {
            let (week_view, review) = if self.tab == tabs::Tab::Week {
                let (today, now_min) = (self.today, now);
                let days = self.week_days();
                let view = week_screen::WeekView::build(days, today, now_min, &Settings::default());
                let review = week_screen::over_capacity_rows(
                    self.all_days(),
                    today,
                    now_min,
                    &Settings::default(),
                );
                (Some(view), review)
            } else {
                (None, Vec::new())
            };
            let monday = self.week_monday;
            let mut go_to = None;
            egui::CentralPanel::default().show(ctx, |ui| match self.tab {
                tabs::Tab::Week => {
                    let working = week_view.as_ref().map_or(0, |v| v.working_days);
                    go_to = week_screen::show_nav(ui, monday, working);
                    if let Some(v) = &week_view {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add_space(6.0);
                            tiles::show(ui, &v.tiles);
                            ui.add_space(10.0);
                            week_screen::show_table(ui, v);
                            ui.add_space(10.0);
                            week_screen::show_loss_bars(ui, &v.losses);
                            ui.add_space(10.0);
                            week_screen::show_review(ui, &review);
                        });
                    }
                }
                tabs::Tab::Calendar => calendar_screen::show(
                    ui,
                    &mut self.calendar,
                    &mut self.add_form,
                    Settings::default().default_start,
                ),
                _ => {}
            });
            if let Some(m) = go_to {
                self.week_monday = m;
            }
            self.save_if_changed();
            self.save_calendar_if_changed();
            return;
        }

        egui::SidePanel::left("andon")
            .resizable(false)
            .show(ctx, |ui| {
                andon::stack_light(ui, andon::lamps(self.tracker.as_ref().map(|t| t.mode())));
            });

        let mut pressed = None;
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
            if let Some(t) = &self.tracker {
                pressed = andon::big_toggle(ui, andon::toggle_button(t.mode()));
            }
            ui.add_space(4.0);
        });

        let mut stop_msg = None;
        if let (Some(action), Some(t)) = (pressed, self.tracker.as_mut()) {
            match action {
                andon::ToggleAction::ToggleDowntime => {
                    if let Some(stop) = t.toggle_downtime_report(now).stopped {
                        stop_msg = Some(andon::stop_message(stop));
                    }
                }
                andon::ToggleAction::Extend => {
                    let end = t.record.end + 20;
                    t.extend_to(end, now);
                }
            }
        }
        if let Some(m) = stop_msg {
            self.push_message(m);
        }

        if let Some(t) = &self.tracker {
            let inputs = t.inputs_at(now);
            let result = day_result(Some(&inputs));
            let ended = t.mode() == Mode::Ended;
            let tiles = tiles::tiles(&inputs, &result.metrics, ended);
            let banners = tiles::banners(&result, ended);
            egui::TopBottomPanel::top("tiles").show(ctx, |ui| {
                ui.add_space(4.0);
                tiles::show(ui, &tiles);
                for b in &banners {
                    ui.add_space(4.0);
                    tiles::show_banner(ui, b);
                }
                ui.add_space(4.0);
            });
        }

        egui::SidePanel::right("messages").min_width(200.0).show(ctx, |ui| {
            ui.heading("Messages");
            for msg in &self.messages {
                ui.label(msg);
            }
        });

        let mut day_end_notices = Vec::new();
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(t) = self.tracker.as_mut() else {
                // D-19: the Not started screen. D-04: today's planned windows come
                // from the weekly calendar.
                let planned = start::planned_for(&self.calendar, self.today);
                let input = self.start_input.get_or_insert_with(|| hhmm(now));
                ui.heading("Start your day");
                ui.horizontal(|ui| {
                    ui.label("Start time");
                    ui.add(egui::TextEdit::singleline(input).desired_width(60.0));
                    ui.label(format!("(now {})", hhmm(now)));
                });
                for line in start::planned_lines(&planned) {
                    ui.label(line);
                }
                if let Some(e) = &self.start_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                if ui.button("Start day").clicked() {
                    match start::validate_start(input, now) {
                        Ok(at) => {
                            self.tracker =
                                Some(Tracker::start_day(at, planned, Settings::default()));
                            self.start_input = None;
                            self.start_error = None;
                        }
                        Err(e) => self.start_error = Some(e),
                    }
                }
                return;
            };
            let live = t.live_record(now);
            let grid_rows = grid::rows(&live, now, t.mode());
            let mut clicked = None;
            let mut extend = false;
            let mut end_now = false;
            let ended = t.mode() == Mode::Ended;
            if let Some(banner) = reminder::categorise_banner(t) {
                if reminder::show(ui, &banner) {
                    t.finalise();
                }
                ui.add_space(4.0);
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.strong(grid::header(&live));
                grid::show_legend(ui);
                ui.add_space(4.0);
                clicked = grid::show(ui, &grid_rows);
                ui.add_space(8.0);
                // D-27: end-of-day controls.
                ui.horizontal(|ui| {
                    extend = ui.button("Extend 20 min").clicked();
                    if !ended {
                        end_now = ui.button("End day now").clicked();
                    }
                });
            });
            if extend {
                let end = t.record.end + 20;
                t.extend_to(end, now);
            }
            if end_now {
                day_end_notices = t.end_now(now);
            }
            if let Some(start) = clicked {
                self.editor =
                    editor::Editor::open(&t.record, &live, start, now, t.mode());
            }
        });

        self.note(day_end_notices);

        if let Some(mut ed) = self.editor.take() {
            match editor::show(ctx, &mut ed) {
                editor::Outcome::Open => self.editor = Some(ed),
                editor::Outcome::Cancel => {}
                editor::Outcome::Save => {
                    let saved = match (&ed.kind, self.tracker.as_mut()) {
                        (editor::Kind::Running, _) | (_, None) => Ok(()),
                        (_, Some(t)) => ed.save(&mut t.record),
                    };
                    if let Err(e) = saved {
                        ed.error = Some(e);
                        self.editor = Some(ed);
                    }
                }
            }
        }

        self.save_if_changed();
        self.save_calendar_if_changed();
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
    fn the_last_tab_is_remembered() {
        let dir = temp_dir("tab");
        let store = store::Store::new(&dir);
        store.save_ui(&tabs::UiState { tab: tabs::Tab::Calendar }).unwrap();
        let app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert_eq!(app.tab, tabs::Tab::Calendar);
        let _ = std::fs::remove_dir_all(dir);
        let app = App::open(None, date(29), hm(9, 0));
        assert_eq!(app.tab, tabs::Tab::Today);
    }

    #[test]
    fn the_calendar_loads_at_launch_and_saves_when_changed() {
        use oee_core::{PlannedKind, PlannedWindow, Weekday};
        let dir = temp_dir("cal");
        let mut app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert_eq!(app.calendar, Default::default());

        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let id = app.calendar.add(Weekday::Thu, lunch.clone());
        app.save_calendar_if_changed();

        let mut again = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert_eq!(again.calendar.windows_for(Weekday::Thu), vec![lunch]);
        again.calendar.remove(id);
        again.save_calendar_if_changed();
        let third = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert!(third.calendar.windows_for(Weekday::Thu).is_empty());
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
