//! Personal OEE desktop app: window over `oee-core`'s live tracker.

mod andon;
mod bulk;
mod calendar_screen;
mod editor;
mod export_screen;
mod goal_screen;
mod grid;
mod lifecycle;
mod hotkey;
mod notify;
mod past_day;
mod practice;
mod reminder;
mod start;
mod settings;
mod store;
mod theme;
mod reopen;
mod tray;
mod tabs;
mod week_screen;
mod tiles;
mod trends_screen;

use chrono::{Local, NaiveDate, Timelike};
use eframe::egui;
use oee_core::{
    day_result, Mode, Notice, Tracker,
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
    /// Blocks picked on Today for a bulk change (D-54).
    selection: bulk::Selection,
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
    /// How many weeks the Week tab's trend chart covers (D-55).
    trend_range: trends_screen::Range,
    /// The CSV export options on the Week tab (D-52).
    export_form: export_screen::ExportForm,
    /// Every saved day, for the over-capacity list. Refreshed every few seconds.
    /// A past day being viewed or edited, shown on the Today tab (D-39).
    past_day: Option<past_day::PastDay>,
    /// The system-wide downtime shortcut, if it could be registered (D-40).
    hotkey: Option<hotkey::Hotkey>,
    /// The menu-bar item (D-41).
    tray: Option<tray::Tray>,
    /// False while the window is hidden in the menu bar (D-43, D-47).
    window_visible: bool,
    /// macOS notifications for the engine's notices (D-42).
    notifier: Option<notify::Notifier>,
    /// Set by the menu-bar Quit, so closing the window really closes (D-43).
    quitting: bool,
    /// The practice-mode fake clock; stays at 0 in release builds (D-45).
    practice: practice::Practice,
    /// Day defaults and the shortcut (D-44), saved to `settings.json`.
    settings: settings::AppSettings,
    settings_form: settings::SettingsForm,
    /// When the last frame ran, to notice the Mac waking from sleep.
    last_frame: chrono::DateTime<Local>,
    all_days_cache: Option<(std::time::Instant, Vec<(NaiveDate, oee_core::DayRecord)>)>,
}

/// A week's records, reloaded when the week changes or every couple of seconds.
struct WeekCache {
    monday: NaiveDate,
    loaded_at: std::time::Instant,
    days: Vec<(NaiveDate, Option<oee_core::DayRecord>)>,
}

impl App {
    /// Open today's saved record if there is one (D-13, D-14), and register
    /// the system-wide shortcut (D-40).
    fn new(ctx: egui::Context) -> Self {
        let mut app = Self::open(
            store::Store::default_location(),
            Local::now().date_naive(),
            now_minutes(),
        );
        app.notifier = Some(notify::Notifier::new(ctx.clone()));
        match tray::Tray::new(ctx.clone()) {
            Ok(t) => app.tray = Some(t),
            Err(e) => app.push_message(format!("Couldn't add the menu-bar item: {e}")),
        }
        if let Err(e) = reopen::install(ctx.clone()) {
            app.push_message(format!("Dock-click reopen isn't available: {e}"));
        }
        let shortcut = app.settings.shortcut;
        let registered = shortcut
            .to_hotkey()
            .and_then(|k| hotkey::Hotkey::register(ctx, k));
        match registered {
            Ok(h) => app.hotkey = Some(h),
            Err(e) => app.push_message(format!(
                "Couldn't register the {} shortcut: {e}",
                shortcut.label()
            )),
        }
        app
    }

    /// FR-011: the downtime toggle, from the button, the shortcut or the D key.
    fn toggle_downtime(&mut self, now: u32) {
        let Some(t) = self.tracker.as_mut() else { return };
        if let Some(stop) = t.toggle_downtime_report(now).stopped {
            self.push_message(andon::stop_message(stop));
        }
    }

    fn open(store: Option<store::Store>, today: NaiveDate, now: u32) -> Self {
        let tab = store.as_ref().map(|s| s.load_ui().tab).unwrap_or_default();
        let mut app = Self {
            tracker: None,
            messages: Vec::new(),
            selection: bulk::Selection::default(),
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
            export_form: export_screen::ExportForm::default(),
            all_days_cache: None,
            trend_range: trends_screen::Range::Weeks12,
            past_day: None,
            hotkey: None,
            tray: None,
            window_visible: !lifecycle::start_hidden(
                std::env::var(lifecycle::HIDDEN_ENV).ok().as_deref(),
            ),
            notifier: None,
            quitting: false,
            practice: Default::default(),
            settings: Default::default(),
            settings_form: settings::SettingsForm::from_settings(&Default::default()),
            last_frame: Local::now(),
        };
        app.load_settings();
        app.load_calendar();
        app.load_day(now);
        app
    }

    fn load_settings(&mut self) {
        match self.store.as_ref().map(|s| s.load_settings()) {
            Some(Ok(Some(settings))) => self.settings = settings,
            Some(Err(e)) => self
                .messages
                .push(format!("Couldn't read the settings: {e}")),
            _ => {}
        }
        self.settings_form = settings::SettingsForm::from_settings(&self.settings);
    }

    /// D-44: check the form, save it, and re-register the shortcut if it changed.
    fn apply_settings(&mut self) {
        let new = match self.settings_form.parse() {
            Ok(new) => new,
            Err(e) => {
                self.settings_form.error = Some(e);
                self.settings_form.saved = false;
                return;
            }
        };
        if let Some(store) = &self.store {
            if let Err(e) = store.save_settings(&new) {
                self.settings_form.error = Some(format!("Couldn't save the settings: {e}"));
                return;
            }
        }
        if new.shortcut != self.settings.shortcut {
            let result = new
                .shortcut
                .to_hotkey()
                .and_then(|k| match self.hotkey.as_mut() {
                    Some(h) => h.change(k),
                    None => Ok(()),
                });
            if let Err(e) = result {
                // The old shortcut stays; the rest of the settings are saved.
                self.push_message(format!(
                    "Couldn't switch the shortcut to {}: {e}",
                    new.shortcut.label()
                ));
                let kept = settings::AppSettings {
                    shortcut: self.settings.shortcut,
                    ..new
                };
                self.settings = kept;
                if let Some(store) = &self.store {
                    let _ = store.save_settings(&kept);
                }
                self.settings_form = settings::SettingsForm::from_settings(&kept);
                self.settings_form.error =
                    Some(format!("The shortcut {} couldn't be registered, so the old one stays.", new.shortcut.label()));
                return;
            }
        }
        self.settings = new;
        self.settings_form.error = None;
        self.settings_form.saved = true;
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

    /// D-39: open a date in the Today layout. Today itself just shows Today.
    fn open_day(&mut self, date: NaiveDate) {
        self.tab = tabs::Tab::Today;
        if date == self.today {
            self.past_day = None;
            return;
        }
        let record = match self.store.as_ref().map(|s| s.load_day(date)) {
            Some(Ok(record)) => record,
            Some(Err(e)) => {
                self.push_message(format!("Couldn't read that day: {e}"));
                None
            }
            None => None,
        };
        self.past_day = Some(past_day::PastDay::new(date, record));
    }

    /// Save edits made to the past day being viewed.
    fn save_past_day_if_changed(&mut self) {
        let (Some(store), Some(pd)) = (&self.store, self.past_day.as_mut()) else {
            return;
        };
        let Some(json) = pd.pending_save() else { return };
        let Some(record) = &pd.record else { return };
        match store.save_day(pd.date, record) {
            Ok(()) => pd.last_saved = json,
            Err(e) => {
                let msg = format!("Couldn't save: {e}");
                self.push_message(msg);
            }
        }
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
                self.tracker = Some(Tracker::resume(record, self.settings.core, now));
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

    /// D-52: write the days and blocks CSV for the chosen range into a folder
    /// the user picks.
    fn export_csv(&mut self, now: u32) {
        self.export_form.error = None;
        let saved = self.store.as_ref().map(|s| s.load_all()).unwrap_or_default();
        let dates: Vec<NaiveDate> = saved.iter().map(|(d, _)| *d).collect();
        let (from, to) = match export_screen::resolve(&self.export_form, self.week_monday, &dates) {
            Ok(range) => range,
            Err(e) => {
                self.export_form.error = Some(e);
                return;
            }
        };
        let days = export_screen::build_days(from, to, &saved, self.today, now);
        let core = self.settings.core;
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Choose a folder for the CSV files")
            .pick_folder()
        else {
            return; // cancelled
        };
        let days_csv = oee_core::export::days_csv(&days, &core);
        let blocks_csv = oee_core::export::blocks_csv(&days, &core);
        match export_screen::write_files(&dir, from, to, &days_csv, &blocks_csv) {
            Ok((d, b)) => {
                let name = |p: &std::path::Path| {
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                };
                self.push_message(format!(
                    "Saved {} and {} to {}.",
                    name(&d),
                    name(&b),
                    dir.display()
                ));
            }
            Err(e) => self.export_form.error = Some(format!("Couldn't save the files: {e}")),
        }
    }

    fn push_message(&mut self, message: String) {
        self.messages.push(message);
        let extra = self.messages.len().saturating_sub(5);
        self.messages.drain(..extra);
    }

    fn note(&mut self, notices: Vec<Notice>) {
        for n in notices {
            if let Some(notifier) = &self.notifier {
                notifier.send(&n);
            }
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
        let now = self.practice.apply(now_minutes());
        let core = self.settings.core;

        // D-43: closing the window hides it; the app keeps running in the menu bar.
        if lifecycle::hide_instead_of_closing(
            ctx.input(|i| i.viewport().close_requested()),
            self.quitting,
        ) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_visible = false;
        }
        // After a sleep, the tick below catches the day up straight away.
        let wall = Local::now();
        if lifecycle::woke_from_sleep(self.last_frame, wall) {
            self.push_message(format!("Back from sleep. Caught up to {}.", hhmm(now)));
        }
        self.last_frame = wall;
        self.roll_over_if_new_day(Local::now().date_naive(), now);

        if let Some(t) = self.tracker.as_mut() {
            let notices = t.tick(now);
            self.note(notices);
        }

        // D-40: the system-wide shortcut works on any tab; D works in the window.
        let shortcut = self.hotkey.as_ref().is_some_and(|h| h.take_pressed());
        let d_key = hotkey::d_key_toggles(
            ctx.input(|i| i.key_pressed(egui::Key::D)),
            ctx.input(|i| i.modifiers.any()),
            ctx.wants_keyboard_input(),
            self.editor.is_some()
                || self.past_day.is_some()
                || self.tab != tabs::Tab::Today,
        );
        let menu = self
            .tray
            .as_ref()
            .map(|t| t.take_actions())
            .unwrap_or_default();
        if shortcut || d_key || menu.toggle {
            self.toggle_downtime(now);
        }
        let clicked = self.notifier.as_ref().is_some_and(|n| n.take_open());
        let dock = reopen::should_show(reopen::take_requested(), self.window_visible);
        if menu.open || clicked || dock {
            self.window_visible = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let errors = self
            .notifier
            .as_ref()
            .map(|n| n.take_errors())
            .unwrap_or_default();
        for e in errors {
            self.push_message(e);
        }
        if menu.quit {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let tray_state = tray::tray_state(self.tracker.as_ref(), now);
        if let Some(tray) = self.tray.as_mut() {
            tray.update(tray_state);
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Personal OEE");
                ui.label(hhmm(now));
                ui.separator();
                for tab in tabs::Tab::ALL {
                    if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                        self.tab = tab;
                        if tab == tabs::Tab::Today {
                            // Clicking Today always returns to today (D-39).
                            self.past_day = None;
                        }
                    }
                }
                // D-45: practice buttons, debug builds only.
                #[cfg(debug_assertions)]
                practice::show(ui, &mut self.practice, now);
            });
        });
        if self.tab != self.last_tab {
            self.last_tab = self.tab;
            if let Some(store) = &self.store {
                let _ = store.save_ui(&tabs::UiState { tab: self.tab });
            }
        }

        if self.tab != tabs::Tab::Today {
            let goal = self.settings.goal.active();
            let (week_view, review, trend, streak) = if self.tab == tabs::Tab::Week {
                let (today, now_min) = (self.today, now);
                let days = self.week_days();
                let view = week_screen::WeekView::build(days, today, now_min, &core);
                let range = self.trend_range;
                let all = self.all_days();
                let review = week_screen::over_capacity_rows(all, today, now_min, &core);
                let trend = trends_screen::series(all, today, now_min, &core, range);
                // The streak looks back over every saved week, not just the chart's range.
                let streak = goal.map_or(0, |g| {
                    let every = trends_screen::series_from(all, NaiveDate::MIN, today, now_min, &core);
                    oee_core::goal_streak(&every, g, week_screen::week_start(today))
                });
                (Some(view), review, trend, streak)
            } else {
                (None, Vec::new(), Vec::new(), 0)
            };
            let today = self.today;
            let monday = self.week_monday;
            let mut go_to = None;
            let mut open_date = None;
            let mut save_settings = false;
            let mut export_now = false;
            egui::CentralPanel::default().show(ctx, |ui| match self.tab {
                tabs::Tab::Week => {
                    let working = week_view.as_ref().map_or(0, |v| v.working_days);
                    go_to = week_screen::show_nav(ui, monday, working);
                    export_now = export_screen::show(ui, &mut self.export_form);
                    if let Some(v) = &week_view {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add_space(6.0);
                            tiles::show(ui, &v.tiles);
                            if let Some(g) = goal {
                                ui.add_space(6.0);
                                let line = goal_screen::line(g, v.oee, v.working_days, monday, today, streak);
                                goal_screen::show(ui, &line);
                            }
                            ui.add_space(10.0);
                            open_date = week_screen::show_table(ui, v).or(open_date);
                            ui.add_space(10.0);
                            week_screen::show_loss_bars(ui, &v.losses);
                            ui.add_space(10.0);
                            trends_screen::show(ui, &mut self.trend_range, &trend, today, goal);
                            ui.add_space(10.0);
                            open_date = week_screen::show_review(ui, &review).or(open_date);
                        });
                    }
                }
                tabs::Tab::Calendar => calendar_screen::show(
                    ui,
                    &mut self.calendar,
                    &mut self.add_form,
                    core.default_start,
                ),
                tabs::Tab::Settings => save_settings = settings::show(ui, &mut self.settings_form),
                _ => {}
            });
            if save_settings {
                self.apply_settings();
            }
            if export_now {
                self.export_csv(now);
            }
            if let Some(m) = go_to {
                self.week_monday = m;
            }
            if let Some(date) = open_date {
                self.open_day(date);
            }
            self.save_if_changed();
            self.save_calendar_if_changed();
            return;
        }

        if let Some(day) = self.past_day.as_mut() {
            let mut back = false;
            egui::CentralPanel::default().show(ctx, |ui| {
                back = past_day::show(ctx, ui, day, &core);
            });
            if back {
                self.past_day = None;
            }
            self.save_past_day_if_changed();
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

        match pressed {
            Some(andon::ToggleAction::ToggleDowntime) => self.toggle_downtime(now),
            Some(andon::ToggleAction::Extend) => {
                if let Some(t) = self.tracker.as_mut() {
                    let end = t.record.end + 20;
                    t.extend_to(end, now);
                }
            }
            None => {}
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
                    theme::error(ui, e);
                }
                if ui.button("Start day").clicked() {
                    match start::validate_start(input, now) {
                        Ok(at) => {
                            self.tracker =
                                Some(Tracker::start_day(at, planned, core));
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
            let mut action = None;
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
                action = bulk::show(ui, &self.selection, &t.record);
                ui.add_space(4.0);
                clicked = grid::show(ui, &grid_rows, self.selection.starts());
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
            match action {
                Some(bulk::Action::Apply(change)) => {
                    self.selection.apply(&mut t.record, change);
                }
                Some(bulk::Action::Clear) => self.selection.clear(),
                None => {}
            }
            if let Some(c) = clicked {
                let selectable =
                    editor::Editor::selectable(&t.record, &live, c.start, now, t.mode());
                if self.selection.route(c, selectable) {
                    self.editor =
                        editor::Editor::open(&t.record, &live, c.start, now, t.mode());
                }
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
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 560.0])
            .with_visible(!lifecycle::start_hidden(
                std::env::var(lifecycle::HIDDEN_ENV).ok().as_deref(),
            )),
        ..Default::default()
    };
    eframe::run_native(
        "Personal OEE",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(cc.egui_ctx.clone())))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, Settings};

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
    fn saved_settings_apply_to_new_days_and_survive_a_restart() {
        let dir = temp_dir("settings");
        let mut app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        app.settings_form.target = "25".into();
        app.settings_form.end = "17:00".into();
        app.apply_settings();
        assert_eq!(app.settings_form.error, None);
        assert!(app.settings_form.saved);
        assert_eq!(app.settings.core.base_target, 25);

        let again = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        assert_eq!(again.settings.core.base_target, 25);
        assert_eq!(again.settings.core.default_end, hm(17, 0));
        assert_eq!(again.settings_form.target, "25");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_settings_change_and_save_nothing() {
        let dir = temp_dir("badsettings");
        let mut app = App::open(Some(store::Store::new(&dir)), date(29), hm(9, 0));
        app.settings_form.target = "0".into();
        app.apply_settings();
        assert!(app.settings_form.error.as_deref().unwrap().contains("daily target"));
        assert!(!app.settings_form.saved);
        assert_eq!(app.settings, Default::default());
        assert_eq!(store::Store::new(&dir).load_settings().unwrap(), None);
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
